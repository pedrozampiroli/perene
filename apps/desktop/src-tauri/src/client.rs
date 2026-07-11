//! Cliente do daemon (substitui o PTY-direto do M0).
//!
//! Os comandos Tauri não abrem mais PTYs: eles falam com o `perene-daemon` via
//! IPC (JSON-lines). Se o daemon não estiver rodando, a UI o sobe (detached) e o
//! adota; se já estiver, só conecta. O output do daemon é reemitido para a webview
//! com os MESMOS eventos do M0 (`pty-output`/`pty-exit`) — o front não muda.

use std::io::Write;

use parking_lot::Mutex;
use tauri::{AppHandle, State};

use perene_protocol::{encode_line, ClientMessage, SpawnRequest};

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

    /// Envia uma mensagem. Sem conexão ainda (ex.: um resize disparado pelo fit()
    /// antes do primeiro spawn), vira no-op — o spawn é quem conecta e leva o
    /// tamanho corrente; não faz sentido explodir aqui.
    fn send(&self, msg: &ClientMessage) -> Result<(), String> {
        let mut guard = self.conn.lock();
        let Some(writer) = guard.as_mut() else {
            return Ok(());
        };
        let line = encode_line(msg).map_err(|e| e.to_string())?;
        writer
            .write_all(line.as_bytes())
            .map_err(|e| e.to_string())?;
        writer.flush().map_err(|e| e.to_string())?;
        Ok(())
    }
}

fn b64(bytes: &[u8]) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

// ── Transporte (unix: implementado; windows: stub que compila) ───────────────

#[cfg(unix)]
fn connect_and_start(app: &AppHandle) -> Result<Box<dyn Write + Send>, String> {
    use std::io::BufWriter;

    use perene_protocol::PROTOCOL_VERSION;

    let stream = connect_or_spawn_daemon()?;
    let read_stream = stream.try_clone().map_err(|e| e.to_string())?;
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

#[cfg(windows)]
fn connect_and_start(_app: &AppHandle) -> Result<Box<dyn Write + Send>, String> {
    Err("cliente do daemon no Windows ainda não implementado (named pipes) — TODO M6".into())
}

#[cfg(unix)]
fn read_loop(stream: std::os::unix::net::UnixStream, app: AppHandle) {
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
            _ => {}
        }
    }
    // Conexão caiu (daemon morreu?). Auto-reconnect fica pro M4 (resume).
}

#[cfg(unix)]
fn connect_or_spawn_daemon() -> Result<std::os::unix::net::UnixStream, String> {
    use std::os::unix::net::UnixStream;
    use std::time::{Duration, Instant};

    let path = perene_core::paths::daemon_endpoint();
    if let Ok(s) = UnixStream::connect(&path) {
        return Ok(s); // daemon já rodando → adota
    }
    spawn_daemon()?;
    let deadline = Instant::now() + Duration::from_secs(5);
    loop {
        if let Ok(s) = UnixStream::connect(&path) {
            return Ok(s);
        }
        if Instant::now() >= deadline {
            return Err("daemon não respondeu a tempo".into());
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

#[cfg(unix)]
fn spawn_daemon() -> Result<(), String> {
    use std::os::unix::process::CommandExt;
    use std::process::{Command, Stdio};

    let bin = daemon_bin_path()?;
    let mut cmd = Command::new(&bin);
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
        .map_err(|e| format!("falha ao subir o daemon {}: {e}", bin.display()))?;
    Ok(())
}

fn daemon_bin_path() -> Result<std::path::PathBuf, String> {
    if let Ok(p) = std::env::var("PERENE_DAEMON_BIN") {
        return Ok(std::path::PathBuf::from(p));
    }
    let exe = std::env::current_exe().map_err(|e| e.to_string())?;
    let dir = exe
        .parent()
        .ok_or_else(|| "sem diretório do executável".to_string())?;
    let name = if cfg!(windows) {
        "perene-daemon.exe"
    } else {
        "perene-daemon"
    };
    let candidate = dir.join(name);
    if candidate.exists() {
        return Ok(candidate);
    }
    Err(format!(
        "binário do daemon não encontrado em {} (defina PERENE_DAEMON_BIN)",
        candidate.display()
    ))
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
    state.send(&ClientMessage::Write {
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
