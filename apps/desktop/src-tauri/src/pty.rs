//! PTY direto no processo Tauri (M0).
//!
//! No M0 não há daemon: a UI fala com estes comandos, que abrem um PTY por pane
//! via `portable-pty` (ConPTY no Windows, forkpty no unix). A saída é lida numa
//! thread bloqueante e **coalescida por frame** (~8ms) antes de virar evento Tauri
//! — senão output pesado (builds, logs) inflaria o IPC e travaria a UI (lição da
//! v1). No M1 esta lógica migra para o `perene-daemon`; os comandos aqui viram
//! um cliente fino.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::sync::mpsc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, CommandBuilder, MasterPty, PtySize};
use tauri::{AppHandle, Emitter};

use perene_protocol::{events, PaneId, SpawnRequest, TerminalExit, TerminalOutput};

/// Uma sessão de PTY viva.
struct Session {
    /// Mantido para `resize`. Segurar o master também mantém o PTY aberto.
    master: Box<dyn MasterPty + Send>,
    /// Canal de escrita (input do usuário → shell).
    writer: Box<dyn Write + Send>,
    /// Processo filho, para matar/reapear.
    child: Box<dyn portable_pty::Child + Send + Sync>,
}

/// Estado gerenciado pelo Tauri: todos os PTYs ativos, indexados por pane.
#[derive(Default)]
pub struct PtyManager {
    sessions: Mutex<HashMap<PaneId, Session>>,
}

impl PtyManager {
    fn spawn(&self, app: AppHandle, req: SpawnRequest) -> Result<(), String> {
        // Idempotente: não abre dois PTYs para o mesmo pane.
        if self.sessions.lock().contains_key(&req.pane_id) {
            return Ok(());
        }

        let pty_system = native_pty_system();
        let pair = pty_system
            .openpty(PtySize {
                rows: req.rows.max(1),
                cols: req.cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            })
            .map_err(|e| format!("openpty falhou: {e}"))?;

        let cmd = build_command(&req);
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn falhou: {e}"))?;
        // O slave não é mais necessário neste processo; segurá-lo atrasaria o EOF.
        drop(pair.slave);

        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("clone_reader falhou: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("take_writer falhou: {e}"))?;

        spawn_reader(app, req.pane_id.clone(), reader);

        self.sessions.lock().insert(
            req.pane_id.clone(),
            Session {
                master: pair.master,
                writer,
                child,
            },
        );
        Ok(())
    }

    fn write(&self, pane_id: &str, data: &[u8]) -> Result<(), String> {
        let mut sessions = self.sessions.lock();
        if let Some(s) = sessions.get_mut(pane_id) {
            s.writer.write_all(data).map_err(|e| e.to_string())?;
            s.writer.flush().map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn resize(&self, pane_id: &str, cols: u16, rows: u16) -> Result<(), String> {
        let sessions = self.sessions.lock();
        if let Some(s) = sessions.get(pane_id) {
            s.master
                .resize(PtySize {
                    rows: rows.max(1),
                    cols: cols.max(1),
                    pixel_width: 0,
                    pixel_height: 0,
                })
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }

    fn kill(&self, pane_id: &str) {
        if let Some(mut s) = self.sessions.lock().remove(pane_id) {
            let _ = s.child.kill();
        }
    }
}

/// Lê o PTY numa thread e emite eventos coalescidos por frame noutra.
fn spawn_reader(app: AppHandle, pane_id: PaneId, mut reader: Box<dyn Read + Send>) {
    let (tx, rx) = mpsc::channel::<Vec<u8>>();

    // Thread de leitura bloqueante: PTY → canal.
    thread::spawn(move || {
        let mut buf = [0u8; 8192];
        loop {
            match reader.read(&mut buf) {
                Ok(0) => break, // EOF: o processo terminou.
                Ok(n) => {
                    if tx.send(buf[..n].to_vec()).is_err() {
                        break;
                    }
                }
                Err(_) => break,
            }
        }
        // `tx` é dropado aqui → o batcher vê `Disconnected` e emite PTY_EXIT.
    });

    // Thread de batching: junta ~8ms de output num único evento Tauri.
    thread::spawn(move || {
        const FRAME: Duration = Duration::from_millis(8);
        const MAX_BATCH: usize = 256 * 1024;
        loop {
            // Bloqueia até chegar o primeiro chunk.
            let first = match rx.recv() {
                Ok(v) => v,
                Err(_) => break, // canal fechado → processo saiu.
            };
            let mut batch = first;
            let deadline = Instant::now() + FRAME;
            loop {
                if batch.len() >= MAX_BATCH {
                    break;
                }
                let now = Instant::now();
                if now >= deadline {
                    break;
                }
                match rx.recv_timeout(deadline - now) {
                    Ok(v) => batch.extend_from_slice(&v),
                    Err(mpsc::RecvTimeoutError::Timeout) => break,
                    Err(mpsc::RecvTimeoutError::Disconnected) => {
                        emit_output(&app, &pane_id, &batch);
                        emit_exit(&app, &pane_id);
                        return;
                    }
                }
            }
            emit_output(&app, &pane_id, &batch);
        }
        emit_exit(&app, &pane_id);
    });
}

fn emit_output(app: &AppHandle, pane_id: &str, bytes: &[u8]) {
    if bytes.is_empty() {
        return;
    }
    let data_b64 = base64::engine::general_purpose::STANDARD.encode(bytes);
    let _ = app.emit(
        events::PTY_OUTPUT,
        TerminalOutput {
            pane_id: pane_id.to_string(),
            data_b64,
        },
    );
}

fn emit_exit(app: &AppHandle, pane_id: &str) {
    let _ = app.emit(
        events::PTY_EXIT,
        TerminalExit {
            pane_id: pane_id.to_string(),
            code: None,
        },
    );
}

/// Monta o `CommandBuilder` do login shell com PATH/aliases carregados.
fn build_command(req: &SpawnRequest) -> CommandBuilder {
    let mut cmd = platform_shell(req.command.as_deref());
    let cwd = req
        .cwd
        .clone()
        .or_else(home_dir)
        .unwrap_or_else(|| ".".to_string());
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("PERENE", "2");
    cmd
}

#[cfg(not(windows))]
fn platform_shell(command: Option<&str>) -> CommandBuilder {
    // Login shell: o PTY já o deixa interativo, então `-l` basta para carregar
    // ~/.zprofile + ~/.zshrc (ou equivalentes bash) e achar claude/codex/opencode.
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
    let mut cmd = CommandBuilder::new(&shell);
    match command {
        None => {
            cmd.arg("-l");
        }
        // Roda o comando e cai de volta no shell para o pane não fechar quando a
        // CLI sair (ex.: `claude` encerra → volta pro prompt).
        Some(c) => {
            cmd.arg("-l");
            cmd.arg("-c");
            cmd.arg(format!("{c}; exec {shell} -l"));
        }
    }
    cmd
}

#[cfg(windows)]
fn platform_shell(command: Option<&str>) -> CommandBuilder {
    // PowerShell carrega o profile do usuário (PATH). ConPTY cuida do TTY.
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.arg("-NoLogo");
    if let Some(c) = command {
        cmd.arg("-NoExit");
        cmd.arg("-Command");
        cmd.arg(c);
    }
    cmd
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

// ── Comandos Tauri (fronteira UI ⇄ Rust) ────────────────────────────────────

#[tauri::command]
pub fn terminal_spawn(
    app: AppHandle,
    state: tauri::State<'_, PtyManager>,
    req: SpawnRequest,
) -> Result<(), String> {
    state.spawn(app, req)
}

#[tauri::command]
pub fn terminal_write(
    state: tauri::State<'_, PtyManager>,
    pane_id: String,
    data: String,
) -> Result<(), String> {
    state.write(&pane_id, data.as_bytes())
}

#[tauri::command]
pub fn terminal_resize(
    state: tauri::State<'_, PtyManager>,
    pane_id: String,
    cols: u16,
    rows: u16,
) -> Result<(), String> {
    state.resize(&pane_id, cols, rows)
}

#[tauri::command]
pub fn terminal_kill(state: tauri::State<'_, PtyManager>, pane_id: String) {
    state.kill(&pane_id);
}
