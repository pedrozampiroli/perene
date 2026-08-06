//! Transporte IPC + single-instance.
//!
//! Unix: `UnixListener`/`UnixStream` (mac/linux).
//! Windows: named pipes ([`crate::winpipe`]) — mesmo protocolo, mesmo loop.
//!
//! A lógica de atendimento ([`handle_client`]/[`dispatch`]) é genérica sobre
//! [`Transport`], então as duas plataformas rodam exatamente o mesmo código: só
//! o "como conectar" muda.
//!
//! Single-instance (lição #2 — dois daemons NUNCA): lock exclusivo no lockfile
//! (`flock` no unix, abertura sem compartilhamento no Windows). Se outro daemon
//! já segura o lock, [`run`] falha e o processo sai (a UI então adota o daemon
//! existente conectando no endpoint).

use std::io::{BufRead, BufReader, BufWriter, Read, Write};
use std::path::{Path, PathBuf};
use std::sync::Arc;

use perene_core::paths;

use crate::session::SessionManager;

/// Configuração do daemon. Injetável (testes usam diretórios temporários — nunca
/// tocam `~/.perene2`, lição #1).
#[derive(Debug, Clone)]
pub struct Config {
    /// Endpoint IPC: caminho do unix socket, ou nome do named pipe no Windows.
    pub socket_path: PathBuf,
    pub lock_path: PathBuf,
    pub scrollback_dir: PathBuf,
    pub scrollback_cap: usize,
}

impl Config {
    /// Config de produção, derivada de `perene_core::paths` (respeita
    /// `PERENE2_STATE_DIR`).
    pub fn from_env() -> Self {
        Self {
            socket_path: paths::daemon_endpoint(),
            lock_path: paths::daemon_lock(),
            scrollback_dir: paths::scrollback_dir(),
            scrollback_cap: 4 * 1024 * 1024,
        }
    }
}

/// Conexão com um cliente. Precisa ser clonável porque uma thread lê comandos
/// enquanto outra escreve o output dos PTYs na mesma conexão.
pub trait Transport: Read + Write + Send + Sized + 'static {
    fn split(&self) -> std::io::Result<Self>;
}

#[cfg(unix)]
impl Transport for std::os::unix::net::UnixStream {
    fn split(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

#[cfg(windows)]
impl Transport for crate::winpipe::PipeStream {
    fn split(&self) -> std::io::Result<Self> {
        self.try_clone()
    }
}

/// Guard do lock de single-instance. Solta o lock quando dropado (fim do processo).
pub struct SingleInstance {
    #[allow(dead_code)]
    file: std::fs::File,
}

fn open_lock_file(lock_path: &Path) -> std::io::Result<std::fs::OpenOptions> {
    if let Some(parent) = lock_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let mut opts = std::fs::OpenOptions::new();
    opts.create(true).write(true).truncate(false);
    Ok(opts)
}

#[cfg(unix)]
pub fn acquire_single_instance(lock_path: &Path) -> anyhow::Result<SingleInstance> {
    use std::os::unix::io::AsRawFd;
    let file = open_lock_file(lock_path)?.open(lock_path)?;
    // flock exclusivo não-bloqueante.
    let ret = unsafe { libc::flock(file.as_raw_fd(), libc::LOCK_EX | libc::LOCK_NB) };
    if ret != 0 {
        anyhow::bail!("outro daemon já está rodando (lock ocupado)");
    }
    Ok(SingleInstance { file })
}

#[cfg(windows)]
pub fn acquire_single_instance(lock_path: &Path) -> anyhow::Result<SingleInstance> {
    use std::os::windows::fs::OpenOptionsExt;
    // share_mode(0) = FILE_SHARE_NONE: o segundo processo a abrir leva
    // ERROR_SHARING_VIOLATION (os error 32). É o equivalente do flock aqui.
    let file = match open_lock_file(lock_path)?.share_mode(0).open(lock_path) {
        Ok(f) => f,
        Err(e) if e.raw_os_error() == Some(32) => {
            anyhow::bail!("outro daemon já está rodando (lock ocupado)")
        }
        Err(e) => return Err(e.into()),
    };
    Ok(SingleInstance { file })
}

/// Sobe o daemon: adquire o lock, faz bind do endpoint e serve para sempre.
#[cfg(unix)]
pub fn run(config: Config) -> anyhow::Result<()> {
    use std::os::unix::net::UnixListener;

    let _lock = acquire_single_instance(&config.lock_path)?;

    if let Some(parent) = config.socket_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    // Sob o flock já garantimos exclusão; um socket remanescente é lixo — remove.
    if config.socket_path.exists() {
        let _ = std::fs::remove_file(&config.socket_path);
    }
    let listener = UnixListener::bind(&config.socket_path)?;

    let mgr = SessionManager::new(config.scrollback_dir.clone(), config.scrollback_cap);

    for stream in listener.incoming() {
        match stream {
            Ok(stream) => {
                let mgr = Arc::clone(&mgr);
                std::thread::spawn(move || handle_client(mgr, stream));
            }
            Err(_) => continue,
        }
    }
    Ok(())
}

#[cfg(windows)]
pub fn run(config: Config) -> anyhow::Result<()> {
    use crate::winpipe::PipeListener;

    let _lock = acquire_single_instance(&config.lock_path)?;

    // Named pipes vivem num namespace do kernel, não no filesystem: nada de
    // criar diretório ou limpar sobra de socket como no unix.
    let mut listener = PipeListener::bind(&config.socket_path)?;

    let mgr = SessionManager::new(config.scrollback_dir.clone(), config.scrollback_cap);

    // Falhas isoladas de accept são toleráveis; falha em série significa pipe
    // quebrado — melhor sair do que rodar em loop quente para sempre.
    let mut consecutive_errors = 0;
    loop {
        match listener.accept() {
            Ok(stream) => {
                consecutive_errors = 0;
                let mgr = Arc::clone(&mgr);
                std::thread::spawn(move || handle_client(mgr, stream));
            }
            Err(e) => {
                consecutive_errors += 1;
                if consecutive_errors >= 16 {
                    return Err(e.into());
                }
            }
        }
    }
}

fn handle_client<T: Transport>(mgr: Arc<SessionManager>, stream: T) {
    use std::sync::mpsc;

    use perene_protocol::{decode_line, encode_line, ClientMessage, DaemonMessage};

    let client_id = mgr.next_client_id();
    let (tx, rx) = mpsc::channel::<DaemonMessage>();

    // Thread de escrita: dreno do canal → conexão. Isola a conexão de escritas
    // concorrentes (as threads de PTY enviam pelo canal).
    let write_stream = match stream.split() {
        Ok(s) => s,
        Err(_) => return,
    };
    let writer = std::thread::spawn(move || {
        let mut w = BufWriter::new(write_stream);
        for msg in rx {
            match encode_line(&msg) {
                Ok(line) => {
                    if w.write_all(line.as_bytes()).is_err() {
                        break;
                    }
                    if w.flush().is_err() {
                        break;
                    }
                }
                Err(_) => {}
            }
        }
    });

    // Loop de leitura: conexão → mensagens do cliente.
    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let msg: ClientMessage = match decode_line(&line) {
            Ok(m) => m,
            Err(e) => {
                let _ = tx.send(DaemonMessage::Error {
                    message: format!("json inválido: {e}"),
                });
                continue;
            }
        };
        if dispatch(&mgr, client_id, &tx, msg).is_break() {
            break;
        }
    }

    mgr.remove_client(client_id);
    drop(tx); // encerra a thread de escrita quando todos os clones caírem.
    let _ = writer.join();
}

fn dispatch(
    mgr: &Arc<SessionManager>,
    client_id: u64,
    tx: &std::sync::mpsc::Sender<perene_protocol::DaemonMessage>,
    msg: perene_protocol::ClientMessage,
) -> std::ops::ControlFlow<()> {
    use base64::Engine;
    use perene_protocol::{ClientMessage, DaemonMessage, PROTOCOL_VERSION};
    use std::ops::ControlFlow;

    match msg {
        ClientMessage::Hello { .. } => {
            let _ = tx.send(DaemonMessage::Welcome {
                protocol_version: PROTOCOL_VERSION,
                daemon_pid: std::process::id(),
            });
        }
        ClientMessage::Spawn(req) => {
            if let Err(e) = mgr.spawn(&req) {
                let _ = tx.send(DaemonMessage::Error { message: e });
            }
        }
        // Pane ACP e pane de terminal atacham pelo mesmo comando: quem sabe o
        // tipo é o daemon, não a UI.
        ClientMessage::Attach { pane_id } => {
            if !mgr.acp().attach(client_id, tx, &pane_id) {
                mgr.attach(client_id, tx, &pane_id);
            }
        }
        ClientMessage::Detach { pane_id } => {
            mgr.acp().detach(client_id, &pane_id);
            mgr.detach(client_id, &pane_id);
        }
        ClientMessage::Write { pane_id, data_b64 } => {
            if let Ok(bytes) = base64::engine::general_purpose::STANDARD.decode(data_b64.as_bytes())
            {
                mgr.write_input(&pane_id, &bytes);
            }
        }
        ClientMessage::Resize {
            pane_id,
            cols,
            rows,
        } => mgr.resize(&pane_id, cols, rows),
        ClientMessage::Kill { pane_id } => {
            mgr.acp().kill(&pane_id);
            mgr.kill(&pane_id);
        }
        ClientMessage::ListPanes => {
            let _ = tx.send(DaemonMessage::Panes {
                panes: mgr.list_panes(),
            });
        }
        ClientMessage::Ping => {
            let _ = tx.send(DaemonMessage::Pong);
        }
        ClientMessage::Shutdown => {
            // Shutdown limpo: despeja scrollback e sai (mata todos os PTYs).
            mgr.flush_scrollback();
            std::process::exit(0);
        }

        // ── Modo ACP ────────────────────────────────────────────────────────
        ClientMessage::AcpSpawn {
            pane_id,
            cwd,
            program,
            args,
            allow_terminal,
        } => mgr
            .acp()
            .spawn(&pane_id, &cwd, &program, &args, allow_terminal),
        ClientMessage::AcpPrompt { pane_id, text } => mgr.acp().prompt(&pane_id, &text),
        ClientMessage::AcpCancel { pane_id } => mgr.acp().cancel(&pane_id),
        ClientMessage::AcpPermission {
            pane_id,
            request_id,
            option_id,
        } => mgr.acp().answer_permission(&pane_id, request_id, option_id),
    }
    ControlFlow::Continue(())
}
