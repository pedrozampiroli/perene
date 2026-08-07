//! Cliente do daemon (substitui o PTY-direto do M0).
//!
//! Os comandos Tauri não abrem mais PTYs: eles falam com o `perene-daemon` via
//! IPC (JSON-lines). Se o daemon não estiver rodando, a UI o sobe (detached) e o
//! adota; se já estiver, só conecta. O output do daemon é reemitido para a webview
//! com os MESMOS eventos do M0 (`pty-output`/`pty-exit`) — o front não muda.
//!
//! Transporte: unix socket no mac/linux, named pipe no Windows
//! (`perene_daemon::winpipe`). Só o "como conectar" é condicional — o resto do
//! fluxo é o mesmo nas três plataformas.

use std::io::{BufWriter, Read, Write};
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, State};

use perene_daemon::Transport;
use perene_protocol::{encode_line, ClientMessage, SpawnRequest, PROTOCOL_VERSION};

/// Conexão com o daemon, por plataforma.
#[cfg(unix)]
type Stream = std::os::unix::net::UnixStream;
#[cfg(windows)]
type Stream = perene_daemon::winpipe::PipeStream;

/// Conexão persistente com o daemon. `None` até o primeiro comando.
#[derive(Default)]
pub struct DaemonClient {
    conn: Mutex<Option<Box<dyn Write + Send>>>,
}

impl DaemonClient {
    /// Garante conexão (subindo o daemon se preciso) e inicia a thread leitora.
    fn ensure(&self, app: &AppHandle) -> Result<(), String> {
        let mut guard = self.conn.lock();
        if guard.is_some() {
            return Ok(());
        }
        let writer = connect_and_start(app)?;
        *guard = Some(writer);
        Ok(())
    }

    /// Derruba a conexão corrente; o próximo `ensure` reconecta (ou ressuscita o
    /// daemon). Chamado quando a leitura termina ou uma escrita falha.
    fn drop_conn(&self) {
        *self.conn.lock() = None;
    }

    /// Envia uma mensagem. Sem conexão ainda (ex.: um resize disparado pelo fit()
    /// antes do primeiro spawn), vira no-op — o spawn é quem conecta e leva o
    /// tamanho corrente; não faz sentido explodir aqui.
    fn send(&self, msg: &ClientMessage) -> Result<(), String> {
        self.send_inner(msg, false)
    }

    /// Igual, mas exige conexão. Usado no input do teclado: digitar num terminal
    /// sem daemon tem que dar erro visível, não sumir em silêncio.
    fn send_connected(&self, msg: &ClientMessage) -> Result<(), String> {
        self.send_inner(msg, true)
    }

    fn send_inner(&self, msg: &ClientMessage, require_conn: bool) -> Result<(), String> {
        let line = encode_line(msg).map_err(|e| e.to_string())?;
        let mut guard = self.conn.lock();
        let Some(writer) = guard.as_mut() else {
            return if require_conn {
                Err("sem conexão com o daemon do Perene".into())
            } else {
                Ok(())
            };
        };
        let result = writer
            .write_all(line.as_bytes())
            .and_then(|_| writer.flush());
        if let Err(e) = result {
            // Conexão morta: descarta para o próximo spawn reconectar.
            *guard = None;
            return Err(e.to_string());
        }
        Ok(())
    }
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ── Transporte ───────────────────────────────────────────────────────────────

fn connect_and_start(app: &AppHandle) -> Result<Box<dyn Write + Send>, String> {
    let stream = connect_or_spawn_daemon()?;
    let read_stream = stream.split().map_err(|e| e.to_string())?;
    let app2 = app.clone();
    std::thread::spawn(move || read_loop(read_stream, app2));

    let mut writer = BufWriter::new(stream);
    let hello = encode_line(&ClientMessage::Hello {
        protocol_version: PROTOCOL_VERSION,
    })
    .map_err(|e| e.to_string())?;
    writer
        .write_all(hello.as_bytes())
        .map_err(|e| e.to_string())?;
    writer.flush().map_err(|e| e.to_string())?;
    Ok(Box::new(writer))
}

/// Espelha `DaemonMessage::AttachDone` num formato serializável — a variante do
/// enum não implementa `Serialize` sozinha fora do `DaemonMessage` (tag `type`).
#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct AttachDonePayload {
    pane_id: String,
}

fn read_loop<R: Read>(stream: R, app: AppHandle) {
    use std::io::{BufRead, BufReader};

    use tauri::Emitter;

    use perene_protocol::{decode_line, events, DaemonMessage};

    let reader = BufReader::new(stream);
    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => break,
        };
        if line.trim().is_empty() {
            continue;
        }
        let msg: DaemonMessage = match decode_line(&line) {
            Ok(m) => m,
            Err(_) => continue,
        };
        match msg {
            // Scrollback (replay do reattach) e Output ao vivo vão pelo mesmo
            // evento: o xterm reconstrói a tela escrevendo os bytes em ordem.
            DaemonMessage::Output(o) | DaemonMessage::Scrollback(o) => {
                let _ = app.emit(events::PTY_OUTPUT, o);
            }
            DaemonMessage::Exit(e) => {
                let _ = app.emit(events::PTY_EXIT, e);
            }
            DaemonMessage::AttachDone { pane_id } => {
                let _ = app.emit(events::PTY_ATTACH_DONE, AttachDonePayload { pane_id });
            }
            _ => {}
        }
    }
    // Conexão caiu (daemon morreu?). Solta o writer morto: o próximo terminal
    // aberto reconecta — ou sobe um daemon novo.
    if let Some(state) = app.try_state::<DaemonClient>() {
        state.drop_conn();
    }
}

/// Conecta no daemon; se não houver ninguém escutando, sobe um e espera subir.
fn connect_or_spawn_daemon() -> Result<Stream, String> {
    if let Ok(s) = try_connect() {
        return Ok(s); // daemon já rodando → adota
    }
    spawn_daemon()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    let mut last_err;
    loop {
        match try_connect() {
            Ok(s) => return Ok(s),
            Err(e) => last_err = e.to_string(),
        }
        if Instant::now() >= deadline {
            return Err(format!("daemon não respondeu a tempo: {last_err}"));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn try_connect() -> std::io::Result<Stream> {
    Stream::connect(perene_core::paths::daemon_endpoint())
}

#[cfg(windows)]
fn try_connect() -> std::io::Result<Stream> {
    perene_daemon::winpipe::connect(&perene_core::paths::daemon_endpoint())
}

#[cfg(unix)]
fn spawn_daemon() -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::Stdio;

    let mut cmd = daemon_command()?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    // setsid: desacopla o daemon da sessão da UI para ele sobreviver ao fechamento.
    unsafe {
        cmd.pre_exec(|| {
            libc::setsid();
            Ok(())
        });
    }
    cmd.spawn()
        .map_err(|e| format!("falha ao subir o daemon: {e}"))?;
    Ok(())
}

#[cfg(windows)]
fn spawn_daemon() -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    use std::process::Stdio;

    // DETACHED_PROCESS: o daemon não herda (nem abre) console — é o análogo do
    // setsid, e evita a janelinha preta piscando. CREATE_NEW_PROCESS_GROUP
    // impede que um Ctrl+C na UI derrube junto os terminais.
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;

    let mut cmd = daemon_command()?;
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .creation_flags(DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
    cmd.spawn()
        .map_err(|e| format!("falha ao subir o daemon: {e}"))?;
    Ok(())
}

/// Comando que sobe o daemon. Por padrão reexecuta ESTE binário com `--daemon`
/// (sem sidecar). `PERENE_DAEMON_BIN` permite apontar um binário standalone.
fn daemon_command() -> Result<std::process::Command, String> {
    if let Ok(p) = std::env::var("PERENE_DAEMON_BIN") {
        return Ok(std::process::Command::new(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let mut cmd = std::process::Command::new(exe);
    cmd.arg("--daemon");
    Ok(cmd)
}

// ── Comandos Tauri (fronteira UI ⇄ Rust) — mesma assinatura do M0 ────────────

#[tauri::command]
pub fn terminal_spawn(
    app: AppHandle,
    state: State<'_, DaemonClient>,
    req: SpawnRequest,
) -> Result<(), String> {
    state.ensure(&app)?;
    let pane_id = req.pane_id.clone();
    state.send(&ClientMessage::Spawn(req))?;
    state.send(&ClientMessage::Attach { pane_id })?;
    Ok(())
}

#[tauri::command]
pub fn terminal_write(
    state: State<'_, DaemonClient>,
    pane_id: String,
    data: String,
) -> Result<(), String> {
    state.send_connected(&ClientMessage::Write {
        pane_id,
        data_b64: b64(data.as_bytes()),
    })
}

#[tauri::command]
pub fn terminal_resize(
    state: State<'_, DaemonClient>,
    pane_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.send(&ClientMessage::Resize {
        pane_id,
        cols,
        rows,
    })
}

#[tauri::command]
pub fn terminal_kill(state: State<'_, DaemonClient>, pane_id: String) {
    let _ = state.send(&ClientMessage::Kill { pane_id });
}
