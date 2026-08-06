//! O lado **cliente** do ACP: o que o agente pede que NÓS façamos.
//!
//! É aqui que o modo ACP se paga. No terminal, a CLI lê e escreve arquivos e
//! roda comandos por conta própria, e o Perene só vê pixels. No ACP a inversão é
//! real: o agente pede, e quem executa somos nós — então dá para pôr limite e
//! deixar tudo visível.
//!
//! Duas garantias, e elas são o motivo deste módulo existir:
//!
//!  - **Escopo:** todo caminho (ler, escrever, `cwd` de comando) é resolvido
//!    contra o diretório da sessão. Sair dele é erro, inclusive por `..` ou por
//!    symlink que aponte para fora.
//!  - **Ambiente limpo:** comandos nascem sem as variáveis de sessão de harness,
//!    igual aos PTYs ([`perene_core::harness_env`]).
//!
//! O que este módulo NÃO faz é decidir se a ação é permitida: isso é do
//! `session/request_permission`, que vai para o usuário (ver [`crate::acp`]). O
//! escopo é a rede de segurança de baixo, não a política.

use std::collections::HashMap;
use std::io::Read;
use std::path::{Component, Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;

use parking_lot::{Condvar, Mutex};
use serde_json::{json, Value};

use perene_acp::RpcError;

/// Teto padrão de saída guardada por comando (a spec deixa o agente escolher).
const DEFAULT_OUTPUT_LIMIT: usize = 1024 * 1024;
/// Teto de leitura de arquivo, para um `fs/read` não estourar a memória.
const MAX_READ_BYTES: u64 = 8 * 1024 * 1024;

fn invalid(msg: impl Into<String>) -> RpcError {
    RpcError {
        code: -32602,
        message: msg.into(),
    }
}

/// Normaliza `.` e `..` sem tocar no disco.
///
/// Não dá para confiar em `canonicalize` sozinho: o alvo pode não existir ainda
/// (um `fs/write` de arquivo novo).
fn lexical_normalize(path: &Path) -> PathBuf {
    let mut out = PathBuf::new();
    for comp in path.components() {
        match comp {
            Component::CurDir => {}
            Component::ParentDir => {
                out.pop();
            }
            other => out.push(other.as_os_str()),
        }
    }
    out
}

/// Caminho real do ancestral mais profundo que existe, com o resto reanexado.
///
/// É o que fecha a porta do symlink: se `<root>/link` aponta para `/etc`, o
/// ancestral existente canoniza para `/etc` e a comparação com a raiz falha.
fn resolve_existing_prefix(path: &Path) -> PathBuf {
    let mut rest: Vec<&std::ffi::OsStr> = Vec::new();
    let mut cur = path;
    loop {
        if let Ok(real) = cur.canonicalize() {
            let mut out = real;
            for part in rest.iter().rev() {
                out.push(part);
            }
            return out;
        }
        match (cur.file_name(), cur.parent()) {
            (Some(name), Some(parent)) => {
                rest.push(name);
                cur = parent;
            }
            _ => return path.to_path_buf(),
        }
    }
}

/// Um comando rodando a pedido do agente.
struct Terminal {
    child: Mutex<Option<Child>>,
    output: Arc<Mutex<Output>>,
    /// `None` enquanto roda; `Some(status)` quando termina.
    exit: Arc<(Mutex<Option<ExitInfo>>, Condvar)>,
}

#[derive(Clone, Copy)]
struct ExitInfo {
    code: Option<i32>,
    signal: Option<i32>,
}

impl ExitInfo {
    fn to_json(self) -> Value {
        json!({ "exitCode": self.code, "signal": self.signal })
    }
}

struct Output {
    bytes: Vec<u8>,
    truncated: bool,
    limit: usize,
}

impl Output {
    /// Guarda o **fim** da saída: quando estoura o limite, o que importa para
    /// quem lê depois é o que aconteceu por último.
    fn push(&mut self, chunk: &[u8]) {
        self.bytes.extend_from_slice(chunk);
        if self.bytes.len() > self.limit {
            let overflow = self.bytes.len() - self.limit;
            self.bytes.drain(0..overflow);
            self.truncated = true;
        }
    }
}

/// Executor das requisições `fs/*` e `terminal/*` de uma sessão.
pub struct ClientTools {
    /// Diretório da sessão, já canonizado. Nada acontece fora daqui.
    root: PathBuf,
    /// `false` = o agente não pode rodar comandos (capacidade não declarada).
    allow_terminal: bool,
    terminals: Mutex<HashMap<String, Arc<Terminal>>>,
    next_terminal: AtomicU64,
}

impl ClientTools {
    pub fn new(root: &str, allow_terminal: bool) -> Self {
        let root = Path::new(root);
        Self {
            root: root.canonicalize().unwrap_or_else(|_| root.to_path_buf()),
            allow_terminal,
            terminals: Mutex::new(HashMap::new()),
            next_terminal: AtomicU64::new(1),
        }
    }

    /// Resolve um caminho pedido pelo agente dentro do escopo da sessão.
    fn resolve(&self, raw: &str) -> Result<PathBuf, RpcError> {
        if raw.is_empty() {
            return Err(invalid("caminho vazio"));
        }
        let asked = Path::new(raw);
        let joined = if asked.is_absolute() {
            asked.to_path_buf()
        } else {
            self.root.join(asked)
        };
        let real = resolve_existing_prefix(&lexical_normalize(&joined));
        if !real.starts_with(&self.root) {
            return Err(invalid(format!(
                "fora do diretório da sessão: {raw} (sessão: {})",
                self.root.display()
            )));
        }
        Ok(real)
    }

    /// Ponto de entrada: o método veio do agente pelo JSON-RPC.
    pub fn handle(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        match method {
            "fs/read_text_file" => self.read_text_file(&params),
            "fs/write_text_file" => self.write_text_file(&params),
            "terminal/create" => self.terminal_create(&params),
            "terminal/output" => self.terminal_output(&params),
            "terminal/wait_for_exit" => self.terminal_wait(&params),
            "terminal/kill" => self.terminal_kill(&params),
            "terminal/release" => self.terminal_release(&params),
            other => Err(RpcError::method_not_found(other)),
        }
    }

    // ── fs ──────────────────────────────────────────────────────────────────

    fn read_text_file(&self, params: &Value) -> Result<Value, RpcError> {
        let path = self.resolve(params["path"].as_str().unwrap_or(""))?;
        let size = std::fs::metadata(&path)
            .map_err(|e| invalid(format!("não consegui ler {}: {e}", path.display())))?
            .len();
        if size > MAX_READ_BYTES {
            return Err(invalid(format!(
                "arquivo grande demais ({size} bytes; teto {MAX_READ_BYTES})"
            )));
        }
        let content = std::fs::read_to_string(&path)
            .map_err(|e| invalid(format!("não consegui ler {}: {e}", path.display())))?;

        // `line` é 1-based e `limit` conta linhas — os dois são opcionais.
        let start = params["line"].as_u64().unwrap_or(1).max(1) as usize - 1;
        let limit = params["limit"].as_u64().map(|n| n as usize);
        let sliced = match (start, limit) {
            (0, None) => content,
            _ => {
                let mut lines: Vec<&str> = content.lines().skip(start).collect();
                if let Some(limit) = limit {
                    lines.truncate(limit);
                }
                lines.join("\n")
            }
        };
        Ok(json!({ "content": sliced }))
    }

    fn write_text_file(&self, params: &Value) -> Result<Value, RpcError> {
        let path = self.resolve(params["path"].as_str().unwrap_or(""))?;
        let content = params["content"].as_str().unwrap_or("");
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .map_err(|e| invalid(format!("não consegui criar {}: {e}", parent.display())))?;
        }
        std::fs::write(&path, content)
            .map_err(|e| invalid(format!("não consegui escrever {}: {e}", path.display())))?;
        Ok(json!({}))
    }

    // ── terminal ────────────────────────────────────────────────────────────

    fn terminal(&self, params: &Value) -> Result<Arc<Terminal>, RpcError> {
        let id = params["terminalId"].as_str().unwrap_or("");
        self.terminals
            .lock()
            .get(id)
            .cloned()
            .ok_or_else(|| invalid(format!("terminal desconhecido: {id}")))
    }

    fn terminal_create(&self, params: &Value) -> Result<Value, RpcError> {
        if !self.allow_terminal {
            return Err(invalid(
                "esta sessão não permite que o agente rode comandos",
            ));
        }
        let program = params["command"]
            .as_str()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| invalid("comando vazio"))?;
        let args: Vec<String> = params["args"]
            .as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|v| v.as_str().map(String::from))
                    .collect()
            })
            .unwrap_or_default();
        let cwd = match params["cwd"].as_str() {
            Some(dir) if !dir.is_empty() => self.resolve(dir)?,
            _ => self.root.clone(),
        };
        let limit = params["outputByteLimit"]
            .as_u64()
            .map(|n| n as usize)
            .filter(|n| *n > 0)
            .unwrap_or(DEFAULT_OUTPUT_LIMIT);

        let mut cmd = Command::new(program);
        cmd.args(&args)
            .current_dir(&cwd)
            .stdin(Stdio::null())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        // Mesma limpeza dos PTYs: a ferramenta não pode se achar aninhada.
        for key in perene_core::harness_env::inherited_session_vars() {
            cmd.env_remove(&key);
        }
        if let Some(env) = params["env"].as_array() {
            for item in env {
                if let (Some(name), Some(value)) = (item["name"].as_str(), item["value"].as_str()) {
                    cmd.env(name, value);
                }
            }
        }
        // Grupo próprio: `terminal/kill` precisa alcançar netos (shell → filho).
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }

        let mut child = cmd
            .spawn()
            .map_err(|e| invalid(format!("não consegui rodar `{program}`: {e}")))?;

        let output = Arc::new(Mutex::new(Output {
            bytes: Vec::new(),
            truncated: false,
            limit,
        }));
        // stdout e stderr entram no MESMO buffer, na ordem em que chegam: é o
        // que o agente veria num terminal de verdade.
        if let Some(out) = child.stdout.take() {
            drain(out, Arc::clone(&output));
        }
        if let Some(err) = child.stderr.take() {
            drain(err, Arc::clone(&output));
        }

        let exit = Arc::new((Mutex::new(None::<ExitInfo>), Condvar::new()));
        let terminal = Arc::new(Terminal {
            child: Mutex::new(Some(child)),
            output,
            exit: Arc::clone(&exit),
        });

        // Colhe o processo numa thread: `terminal/wait_for_exit` só espera o
        // Condvar, então vários pedidos podem esperar o mesmo comando.
        {
            let terminal = Arc::clone(&terminal);
            std::thread::spawn(move || {
                let status = terminal.child.lock().as_mut().map(|c| c.wait());
                let info = match status {
                    Some(Ok(st)) => ExitInfo {
                        code: st.code(),
                        signal: exit_signal(&st),
                    },
                    _ => ExitInfo {
                        code: None,
                        signal: None,
                    },
                };
                let (lock, cvar) = &*exit;
                *lock.lock() = Some(info);
                cvar.notify_all();
            });
        }

        let id = format!(
            "term_{}",
            self.next_terminal.fetch_add(1, Ordering::Relaxed)
        );
        self.terminals.lock().insert(id.clone(), terminal);
        Ok(json!({ "terminalId": id }))
    }

    fn terminal_output(&self, params: &Value) -> Result<Value, RpcError> {
        let terminal = self.terminal(params)?;
        let out = terminal.output.lock();
        let exit = *terminal.exit.0.lock();
        Ok(json!({
            "output": String::from_utf8_lossy(&out.bytes),
            "truncated": out.truncated,
            "exitStatus": exit.map(|e| e.to_json()),
        }))
    }

    fn terminal_wait(&self, params: &Value) -> Result<Value, RpcError> {
        let terminal = self.terminal(params)?;
        let (lock, cvar) = &*terminal.exit;
        let mut guard = lock.lock();
        while guard.is_none() {
            cvar.wait(&mut guard);
        }
        Ok(json!({ "exitStatus": guard.expect("saiu do laço com Some").to_json() }))
    }

    fn terminal_kill(&self, params: &Value) -> Result<Value, RpcError> {
        let terminal = self.terminal(params)?;
        kill_child(&terminal);
        Ok(json!({}))
    }

    fn terminal_release(&self, params: &Value) -> Result<Value, RpcError> {
        let id = params["terminalId"].as_str().unwrap_or("").to_string();
        if let Some(terminal) = self.terminals.lock().remove(&id) {
            kill_child(&terminal);
        }
        Ok(json!({}))
    }

    /// Encerra tudo que ficou rodando (sessão fechada).
    pub fn shutdown(&self) {
        for (_, terminal) in self.terminals.lock().drain() {
            kill_child(&terminal);
        }
    }
}

impl Drop for ClientTools {
    fn drop(&mut self) {
        self.shutdown();
    }
}

/// Lê um stream até o fim, acumulando no buffer de saída.
fn drain(mut stream: impl Read + Send + 'static, output: Arc<Mutex<Output>>) {
    std::thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match stream.read(&mut buf) {
                Ok(0) | Err(_) => break,
                Ok(n) => output.lock().push(&buf[..n]),
            }
        }
    });
}

/// Mata o comando e seus descendentes (um `sh -c` vira filho de novo).
fn kill_child(terminal: &Terminal) {
    let mut guard = terminal.child.lock();
    let Some(child) = guard.as_mut() else { return };
    #[cfg(unix)]
    unsafe {
        libc::killpg(child.id() as i32, libc::SIGKILL);
    }
    #[cfg(windows)]
    {
        let _ = Command::new("taskkill")
            .args(["/T", "/F", "/PID", &child.id().to_string()])
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .status();
    }
    let _ = child.kill();
}

#[cfg(unix)]
fn exit_signal(status: &std::process::ExitStatus) -> Option<i32> {
    use std::os::unix::process::ExitStatusExt;
    status.signal()
}

#[cfg(not(unix))]
fn exit_signal(_status: &std::process::ExitStatus) -> Option<i32> {
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tools(root: &Path) -> ClientTools {
        ClientTools::new(&root.to_string_lossy(), true)
    }

    #[test]
    fn reads_and_writes_inside_the_session_directory() {
        let dir = tempfile::tempdir().unwrap();
        let t = tools(dir.path());

        t.handle(
            "fs/write_text_file",
            json!({"path": "sub/nota.txt", "content": "oi\nmundo\n"}),
        )
        .expect("escrever dentro do escopo é permitido");

        let v = t
            .handle("fs/read_text_file", json!({"path": "sub/nota.txt"}))
            .unwrap();
        assert_eq!(v["content"], "oi\nmundo\n");

        // `line`/`limit` recortam por linha (1-based).
        let v = t
            .handle(
                "fs/read_text_file",
                json!({"path": "sub/nota.txt", "line": 2, "limit": 1}),
            )
            .unwrap();
        assert_eq!(v["content"], "mundo");
    }

    #[test]
    fn refuses_to_escape_the_session_directory() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("projeto");
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(dir.path().join("segredo.txt"), "não é da sua conta").unwrap();
        let t = tools(&root);

        for path in ["../segredo.txt", "sub/../../segredo.txt"] {
            let err = t
                .handle("fs/read_text_file", json!({ "path": path }))
                .expect_err("subir de diretório tem que ser recusado");
            assert!(err.message.contains("fora do diretório"), "{err:?}");
        }

        let absolute = dir.path().join("segredo.txt").to_string_lossy().to_string();
        let err = t
            .handle("fs/read_text_file", json!({ "path": absolute }))
            .expect_err("caminho absoluto fora do escopo também");
        assert!(err.message.contains("fora do diretório"), "{err:?}");

        // Escrever fora não pode nem criar o arquivo.
        let alvo = dir.path().join("invadido.txt");
        let _ = t.handle(
            "fs/write_text_file",
            json!({"path": alvo.to_string_lossy(), "content": "x"}),
        );
        assert!(!alvo.exists(), "não pode ter escrito fora do escopo");
    }

    #[cfg(unix)]
    #[test]
    fn refuses_symlinks_pointing_outside() {
        // O caso que uma checagem só lexical deixaria passar.
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("projeto");
        std::fs::create_dir_all(&root).unwrap();
        let secret = dir.path().join("segredo.txt");
        std::fs::write(&secret, "não é da sua conta").unwrap();
        std::os::unix::fs::symlink(&secret, root.join("atalho.txt")).unwrap();

        let t = tools(&root);
        let err = t
            .handle("fs/read_text_file", json!({"path": "atalho.txt"}))
            .expect_err("symlink para fora tem que ser recusado");
        assert!(err.message.contains("fora do diretório"), "{err:?}");
    }

    #[cfg(unix)]
    #[test]
    fn runs_a_command_and_reports_output_and_exit() {
        let dir = tempfile::tempdir().unwrap();
        let t = tools(dir.path());

        let created = t
            .handle(
                "terminal/create",
                json!({"command": "/bin/sh", "args": ["-c", "echo ola; echo erro 1>&2; exit 3"]}),
            )
            .unwrap();
        let id = created["terminalId"].as_str().unwrap().to_string();

        let waited = t
            .handle("terminal/wait_for_exit", json!({ "terminalId": id }))
            .unwrap();
        assert_eq!(waited["exitStatus"]["exitCode"], 3);

        let out = t
            .handle("terminal/output", json!({ "terminalId": id }))
            .unwrap();
        let text = out["output"].as_str().unwrap();
        assert!(text.contains("ola"), "faltou o stdout: {text:?}");
        assert!(
            text.contains("erro"),
            "stderr entra no mesmo buffer: {text:?}"
        );
        assert_eq!(out["exitStatus"]["exitCode"], 3);

        t.handle("terminal/release", json!({ "terminalId": id }))
            .unwrap();
        assert!(
            t.handle("terminal/output", json!({ "terminalId": id }))
                .is_err(),
            "depois do release o terminal não existe mais"
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_runs_inside_the_session_directory_by_default() {
        let dir = tempfile::tempdir().unwrap();
        let t = tools(dir.path());
        let created = t
            .handle(
                "terminal/create",
                json!({"command": "/bin/sh", "args": ["-c", "pwd"]}),
            )
            .unwrap();
        let id = created["terminalId"].as_str().unwrap().to_string();
        t.handle("terminal/wait_for_exit", json!({ "terminalId": id }))
            .unwrap();
        let out = t
            .handle("terminal/output", json!({ "terminalId": id }))
            .unwrap();
        let pwd = out["output"].as_str().unwrap().trim().to_string();
        assert_eq!(
            std::path::Path::new(&pwd).canonicalize().unwrap(),
            dir.path().canonicalize().unwrap()
        );
    }

    #[cfg(unix)]
    #[test]
    fn command_cwd_cannot_escape_the_session() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path().join("projeto");
        std::fs::create_dir_all(&root).unwrap();
        let t = tools(&root);
        let err = t
            .handle(
                "terminal/create",
                json!({"command": "/bin/sh", "args": ["-c", "pwd"], "cwd": "/"}),
            )
            .expect_err("cwd fora do escopo tem que ser recusado");
        assert!(err.message.contains("fora do diretório"), "{err:?}");
    }

    #[cfg(unix)]
    #[test]
    fn output_keeps_the_tail_and_flags_truncation() {
        let dir = tempfile::tempdir().unwrap();
        let t = tools(dir.path());
        let created = t
            .handle(
                "terminal/create",
                json!({"command": "/bin/sh",
                       "args": ["-c", "for i in 1 2 3 4 5 6 7 8 9; do echo linha$i; done"],
                       "outputByteLimit": 16}),
            )
            .unwrap();
        let id = created["terminalId"].as_str().unwrap().to_string();
        t.handle("terminal/wait_for_exit", json!({ "terminalId": id }))
            .unwrap();
        let out = t
            .handle("terminal/output", json!({ "terminalId": id }))
            .unwrap();
        assert_eq!(out["truncated"], true);
        let text = out["output"].as_str().unwrap();
        assert!(text.len() <= 16, "estourou o teto: {text:?}");
        assert!(text.contains("linha9"), "o fim é o que importa: {text:?}");
    }

    #[test]
    fn terminal_is_refused_when_the_capability_is_off() {
        let dir = tempfile::tempdir().unwrap();
        let t = ClientTools::new(&dir.path().to_string_lossy(), false);
        let err = t
            .handle("terminal/create", json!({"command": "/bin/sh"}))
            .expect_err("sem a capacidade, rodar comando é erro");
        assert!(err.message.contains("não permite"), "{err:?}");
    }

    #[test]
    fn unknown_methods_are_reported_as_such() {
        let dir = tempfile::tempdir().unwrap();
        let t = tools(dir.path());
        let err = t.handle("fs/algo_novo", json!({})).unwrap_err();
        assert_eq!(err.code, -32601);
    }
}
