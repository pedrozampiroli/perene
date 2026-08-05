//! Teste automatizado do protocolo do daemon — SEM UI (aceite do M1).
//!
//! Cobre o cenário do critério de aceite: fechar a janela e reabrir → mesmo
//! shell, mesmo processo vivo, scrollback preservado. Modelamos "fechar/reabrir"
//! como desconectar um cliente e conectar outro; o PTY vive no daemon entre os dois.
//!
//! Roda nas TRÊS plataformas: no unix o transporte é o socket, no Windows é o
//! named pipe. O módulo [`platform`] concentra tudo que difere (endpoint, shell,
//! comandos) — o corpo dos testes é o mesmo. Isso é de propósito: o IPC do
//! Windows já ficou quebrado sem ninguém perceber porque os testes eram
//! `#![cfg(unix)]`.
//!
//! Lição #1: NADA toca `~/.perene2`. Tudo em diretório temporário injetado.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use base64::Engine;
use perene_protocol::{
    decode_line, encode_line, ClientMessage, DaemonMessage, PaneState, SpawnRequest,
};

use platform::Stream;

const MARKER: &str = "PERENE_M1_MARK_42";
const ALIVE: &str = "PERENE_ALIVE_99";

/// Tudo que muda entre unix e Windows.
mod platform {
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    pub type Stream = std::os::unix::net::UnixStream;
    #[cfg(windows)]
    pub type Stream = perene_daemon::winpipe::PipeStream;

    /// Endpoint IPC do daemon deste teste. Único por teste: os testes rodam em
    /// paralelo na mesma máquina (e no Windows o pipe é global à sessão).
    #[cfg(unix)]
    pub fn endpoint(dir: &Path) -> PathBuf {
        dir.join("daemon.sock")
    }

    #[cfg(windows)]
    pub fn endpoint(_dir: &Path) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        PathBuf::from(format!(r"\\.\pipe\perene2-test-{}-{n}", std::process::id()))
    }

    #[cfg(unix)]
    pub fn connect(endpoint: &Path) -> std::io::Result<Stream> {
        Stream::connect(endpoint)
    }

    #[cfg(windows)]
    pub fn connect(endpoint: &Path) -> std::io::Result<Stream> {
        perene_daemon::winpipe::connect(endpoint)
    }

    pub fn split(stream: &Stream) -> std::io::Result<Stream> {
        stream.try_clone()
    }

    /// Fecha a conexão de verdade (o daemon precisa ver EOF), destravando quem
    /// estiver bloqueado lendo.
    #[cfg(unix)]
    pub fn shutdown(stream: &Stream) {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }

    #[cfg(windows)]
    pub fn shutdown(stream: &Stream) {
        stream.shutdown();
    }

    /// Shell hermético e rápido para os testes (evita `.zshrc`/profile pesado).
    #[cfg(unix)]
    pub fn test_shell() -> Option<String> {
        Some("/bin/sh".to_string())
    }

    #[cfg(windows)]
    pub fn test_shell() -> Option<String> {
        Some(std::env::var("COMSPEC").unwrap_or_else(|_| "cmd.exe".to_string()))
    }

    /// Comando que imprime `text` e devolve o prompt.
    #[cfg(unix)]
    pub fn echo_command(text: &str) -> String {
        format!("printf '{text}\\n'")
    }

    #[cfg(windows)]
    pub fn echo_command(text: &str) -> String {
        format!("echo {text}")
    }

    /// Linha digitada no terminal já vivo (com o Enter da plataforma).
    #[cfg(unix)]
    pub fn typed_line(text: &str) -> String {
        format!("echo {text}\n")
    }

    #[cfg(windows)]
    pub fn typed_line(text: &str) -> String {
        format!("echo {text}\r")
    }

    /// Shell + comando que imprimem `ENVCHECK=[a][b][c]` com as três vars de
    /// harness. Precisa de um shell que expanda variável indefinida como vazio —
    /// o `cmd.exe` deixaria `%VAR%` literal, então no Windows usamos PowerShell.
    #[cfg(unix)]
    pub fn env_probe() -> (Option<String>, String) {
        (
            Some("/bin/sh".to_string()),
            "printf 'ENVCHECK=[%s][%s][%s]\\n' \"$CLAUDE_CODE_CHILD_SESSION\" \"$CLAUDE_CODE_SESSION_ID\" \"$CLAUDECODE\"".to_string(),
        )
    }

    #[cfg(windows)]
    pub fn env_probe() -> (Option<String>, String) {
        (
            Some("powershell.exe".to_string()),
            "Write-Host ENVCHECK=[$env:CLAUDE_CODE_CHILD_SESSION][$env:CLAUDE_CODE_SESSION_ID][$env:CLAUDECODE]".to_string(),
        )
    }
}

fn b64_decode(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .unwrap_or_default()
}

/// Cliente de linha com timeout, para os testes não travarem.
///
/// A leitura roda numa thread própria alimentando um canal: assim o timeout é do
/// `recv_timeout` e não depende de o transporte suportar `set_read_timeout` (o
/// named pipe não suporta).
struct LineClient {
    stream: Stream,
    rx: mpsc::Receiver<Vec<u8>>,
    buf: Vec<u8>,
}

impl LineClient {
    fn connect(endpoint: &Path) -> std::io::Result<Self> {
        let stream = platform::connect(endpoint)?;
        let mut read_side = platform::split(&stream)?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut tmp = [0u8; 8192];
            loop {
                match read_side.read(&mut tmp) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(tmp[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });
        Ok(Self {
            stream,
            rx,
            buf: Vec::new(),
        })
    }

    fn send(&mut self, msg: &ClientMessage) {
        let line = encode_line(msg).unwrap();
        self.stream.write_all(line.as_bytes()).unwrap();
        self.stream.flush().unwrap();
    }

    fn next_msg(&mut self, deadline: Instant) -> Option<DaemonMessage> {
        loop {
            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buf.drain(0..=pos).collect();
                let text = String::from_utf8_lossy(&line[..line.len() - 1]).to_string();
                if text.trim().is_empty() {
                    continue;
                }
                return decode_line(&text).ok();
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(chunk) => self.buf.extend_from_slice(&chunk),
                Err(_) => return None, // timeout ou conexão encerrada
            }
        }
    }

    /// Lê mensagens até que os bytes de Output/Scrollback acumulados contenham
    /// `needle`. Retorna true se achou dentro do prazo.
    fn wait_for_output(&mut self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut acc: Vec<u8> = Vec::new();
        while Instant::now() < deadline {
            match self.next_msg(deadline) {
                Some(DaemonMessage::Output(o)) | Some(DaemonMessage::Scrollback(o)) => {
                    acc.extend_from_slice(&b64_decode(&o.data_b64));
                    if String::from_utf8_lossy(&acc).contains(needle) {
                        return true;
                    }
                }
                Some(_) => {}
                None => {}
            }
        }
        String::from_utf8_lossy(&acc).contains(needle)
    }

    /// Espera um `Status` com o estado desejado (indicador da UI).
    fn wait_for_status(&mut self, want: PaneState, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if let Some(DaemonMessage::Status(st)) = self.next_msg(deadline) {
                if st.state == want {
                    return true;
                }
            }
        }
        false
    }

    /// Igual, mas só considera mensagens de Scrollback (replay do reattach).
    fn wait_for_scrollback(&mut self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut acc: Vec<u8> = Vec::new();
        while Instant::now() < deadline {
            match self.next_msg(deadline) {
                Some(DaemonMessage::Scrollback(o)) => {
                    acc.extend_from_slice(&b64_decode(&o.data_b64));
                    if String::from_utf8_lossy(&acc).contains(needle) {
                        return true;
                    }
                }
                Some(DaemonMessage::AttachDone { .. }) => {
                    // Replay terminou; decide com o que já veio.
                    return String::from_utf8_lossy(&acc).contains(needle);
                }
                Some(_) => {}
                None => {}
            }
        }
        String::from_utf8_lossy(&acc).contains(needle)
    }
}

impl Drop for LineClient {
    fn drop(&mut self) {
        // Sem isto a thread leitora seguraria a conexão aberta e o daemon nunca
        // veria o cliente sair — justo o que o teste de reattach quer exercitar.
        platform::shutdown(&self.stream);
    }
}

fn wait_for_daemon(endpoint: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if platform::connect(endpoint).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("daemon não subiu em {}", endpoint.display());
}

/// Sobe um daemon isolado (endpoint/lock/scrollback em tempdir) e espera ficar
/// pronto.
fn start_daemon(dir: &Path) -> std::path::PathBuf {
    let endpoint = platform::endpoint(dir);
    let config = perene_daemon::Config {
        socket_path: endpoint.clone(),
        lock_path: dir.join("daemon.lock"),
        scrollback_dir: dir.join("scrollback"),
        scrollback_cap: 1024 * 1024,
    };
    // Thread (morre com o processo de teste no fim).
    std::thread::spawn(move || {
        let _ = perene_daemon::run(config);
    });
    wait_for_daemon(&endpoint, Duration::from_secs(5));
    endpoint
}

fn hello(client: &mut LineClient) {
    client.send(&ClientMessage::Hello {
        protocol_version: perene_daemon::PROTOCOL_VERSION,
    });
}

#[test]
fn spawn_attach_reattach_preserves_scrollback_and_process() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());

    // ── Cliente 1: cria o pane e vê o marcador ────────────────────────────
    let mut c1 = LineClient::connect(&endpoint).unwrap();
    hello(&mut c1);
    assert!(
        matches!(
            c1.next_msg(Instant::now() + Duration::from_secs(2)),
            Some(DaemonMessage::Welcome { .. })
        ),
        "esperava Welcome"
    );

    let pane_id = "pane_test".to_string();
    c1.send(&ClientMessage::Spawn(SpawnRequest {
        pane_id: pane_id.clone(),
        cols: 80,
        rows: 24,
        cwd: Some(dir.path().to_string_lossy().to_string()),
        command: Some(platform::echo_command(MARKER)),
        shell: platform::test_shell(),
    }));
    c1.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });
    assert!(
        c1.wait_for_output(MARKER, Duration::from_secs(20)),
        "cliente 1 devia ver o marcador do comando inicial"
    );

    // ── "Fecha a janela": desconecta o cliente 1. O PTY segue vivo. ────────
    drop(c1);
    std::thread::sleep(Duration::from_millis(200));

    // ── "Reabre a janela": cliente 2 atacha e recebe o scrollback. ────────
    let mut c2 = LineClient::connect(&endpoint).unwrap();
    hello(&mut c2);
    let _ = c2.next_msg(Instant::now() + Duration::from_secs(2)); // Welcome
    c2.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });
    assert!(
        c2.wait_for_scrollback(MARKER, Duration::from_secs(5)),
        "reattach devia reproduzir o scrollback com o marcador"
    );

    // ── Mesmo processo vivo: mandamos um comando e vemos a resposta. ───────
    c2.send(&ClientMessage::Write {
        pane_id: pane_id.clone(),
        data_b64: base64::engine::general_purpose::STANDARD.encode(platform::typed_line(ALIVE)),
    });
    assert!(
        c2.wait_for_output(ALIVE, Duration::from_secs(20)),
        "o shell original devia continuar vivo e responder após o reattach"
    );

    // Lista de panes reporta o pane vivo.
    c2.send(&ClientMessage::ListPanes);
    let mut saw_pane = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(DaemonMessage::Panes { panes }) = c2.next_msg(deadline) {
            saw_pane = panes.iter().any(|p| p.pane_id == pane_id && p.alive);
            break;
        }
    }
    assert!(saw_pane, "ListPanes devia reportar o pane vivo");
}

/// Regressão: se o Perene for aberto de dentro de uma sessão do Claude Code, os
/// marcadores herdados (`CLAUDE_CODE_CHILD_SESSION`, `CLAUDE_CODE_SESSION_ID`…)
/// desligam o salvamento do transcript do `claude` que roda no PTY — e aí o
/// `--resume` falha depois com "No conversation found". Os terminais têm que
/// nascer com o ambiente limpo.
#[test]
fn spawned_terminals_do_not_inherit_harness_session_env() {
    std::env::set_var("CLAUDE_CODE_CHILD_SESSION", "1");
    std::env::set_var("CLAUDE_CODE_SESSION_ID", "deadbeef");
    std::env::set_var("CLAUDECODE", "1");

    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    let _ = c.next_msg(Instant::now() + Duration::from_secs(2));

    let (shell, command) = platform::env_probe();
    let pane_id = "pane_env".to_string();
    c.send(&ClientMessage::Spawn(SpawnRequest {
        pane_id: pane_id.clone(),
        cols: 80,
        rows: 24,
        cwd: Some(dir.path().to_string_lossy().to_string()),
        // Imprime as vars: têm que sair VAZIAS dentro do PTY.
        command: Some(command),
        shell,
    }));
    c.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });

    assert!(
        c.wait_for_output("ENVCHECK=[][][]", Duration::from_secs(30)),
        "o terminal herdou vars de sessão do harness (transcript saving quebraria)"
    );
}

#[test]
fn single_instance_lock_blocks_second_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("daemon.lock");

    let g1 = perene_daemon::acquire_single_instance(&lock).expect("primeiro lock deve pegar");
    let g2 = perene_daemon::acquire_single_instance(&lock);
    assert!(
        g2.is_err(),
        "segundo daemon NUNCA pode adquirir o lock (lição #2)"
    );

    drop(g1);
    let g3 = perene_daemon::acquire_single_instance(&lock);
    assert!(g3.is_ok(), "liberado o lock, um novo daemon pode assumir");
}


/// O indicador da UI vive de mensagens `Status` do daemon: saída chegando marca
/// `Running` e, depois do silêncio, `Done`. Sem isso o dot não acende.
#[test]
fn reports_session_status_running_then_done() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);

    let pane_id = "pane_status".to_string();
    c.send(&ClientMessage::Spawn(SpawnRequest {
        pane_id: pane_id.clone(),
        cols: 80,
        rows: 24,
        cwd: Some(dir.path().to_string_lossy().to_string()),
        command: Some(platform::echo_command("trabalhando")),
        shell: platform::test_shell(),
    }));
    c.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });

    assert!(
        c.wait_for_status(PaneState::Running, Duration::from_secs(10)),
        "saída do PTY devia marcar a sessão como Running"
    );
    assert!(
        c.wait_for_status(PaneState::Done, Duration::from_secs(10)),
        "depois do silêncio, a sessão devia virar Done"
    );
}
