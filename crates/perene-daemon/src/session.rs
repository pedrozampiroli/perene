//! `SessionManager` — dono de todos os PTYs vivos.
//!
//! Cada pane tem: PTY (portable-pty), buffer de scrollback em memória (capado) e
//! uma lista de assinantes (clientes atachados). A saída do PTY é lida numa thread
//! e **coalescida por frame** (~8ms) antes de: (1) ir pro scrollback e (2) ser
//! difundida aos assinantes. No reattach, o scrollback é reproduzido — é isso que
//! faz "fechar e reabrir a janela" reencontrar o mesmo terminal.

use std::collections::HashMap;
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::Arc;
use std::thread;
use std::time::{Duration, Instant};

use base64::Engine;
use parking_lot::Mutex;
use portable_pty::{native_pty_system, MasterPty, PtySize};

use perene_protocol::{
    DaemonMessage, PaneId, PaneInfo, PaneStatus, SpawnRequest, TerminalExit, TerminalOutput,
};

use crate::acp::AcpManager;
use crate::pty::build_command;
use crate::status::{StatusDetector, QUIET};

pub type ClientId = u64;

fn b64(bytes: &[u8]) -> String {
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

struct Pane {
    /// Mantido para `resize`; segurá-lo mantém o PTY aberto.
    master: Box<dyn MasterPty + Send>,
    writer: Box<dyn Write + Send>,
    child: Box<dyn portable_pty::Child + Send + Sync>,
    /// Histórico cru de bytes (capado). Reproduzido no attach.
    scrollback: Vec<u8>,
    /// Clientes recebendo output ao vivo.
    subscribers: Vec<(ClientId, Sender<DaemonMessage>)>,
    /// `false` quando o processo já saiu (mas o pane fica pra mostrar a saída).
    alive: bool,
    /// O que a sessão está fazendo (para o indicador na UI).
    status: StatusDetector,
}

pub struct SessionManager {
    panes: Mutex<HashMap<PaneId, Pane>>,
    /// Panes em modo ACP. Vivem aqui pelo mesmo motivo dos PTYs: fechar a janela
    /// não pode matar a conversa.
    acp: AcpManager,
    scrollback_cap: usize,
    scrollback_dir: PathBuf,
    next_client: AtomicU64,
}

impl SessionManager {
    pub fn new(scrollback_dir: PathBuf, scrollback_cap: usize) -> Arc<Self> {
        let mgr = Arc::new(Self {
            panes: Mutex::new(HashMap::new()),
            acp: AcpManager::new(),
            scrollback_cap,
            scrollback_dir,
            next_client: AtomicU64::new(1),
        });
        mgr.clone().spawn_status_ticker();
        mgr
    }

    /// Sessões ACP (o dispatch do IPC fala direto com elas).
    pub fn acp(&self) -> &AcpManager {
        &self.acp
    }

    /// Estados que dependem de TEMPO (parou de produzir saída, "terminou" que
    /// expira) não têm evento que os dispare — daí o ticker.
    fn spawn_status_ticker(self: Arc<Self>) {
        thread::spawn(move || loop {
            thread::sleep(QUIET / 3);
            let mut panes = self.panes.lock();
            for (id, pane) in panes.iter_mut() {
                if let Some(state) = pane.status.tick() {
                    let msg = DaemonMessage::Status(PaneStatus {
                        pane_id: id.clone(),
                        state,
                    });
                    pane.subscribers.retain(|(_, tx)| tx.send(msg.clone()).is_ok());
                }
            }
        });
    }

    pub fn next_client_id(&self) -> ClientId {
        self.next_client.fetch_add(1, Ordering::Relaxed)
    }

    /// Cria um PTY (idempotente por `pane_id`).
    pub fn spawn(self: &Arc<Self>, req: &SpawnRequest) -> Result<(), String> {
        let mut panes = self.panes.lock();
        if panes.contains_key(&req.pane_id) {
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
        let cmd = build_command(req);
        let child = pair
            .slave
            .spawn_command(cmd)
            .map_err(|e| format!("spawn falhou: {e}"))?;
        drop(pair.slave);
        let reader = pair
            .master
            .try_clone_reader()
            .map_err(|e| format!("clone_reader falhou: {e}"))?;
        let writer = pair
            .master
            .take_writer()
            .map_err(|e| format!("take_writer falhou: {e}"))?;
        panes.insert(
            req.pane_id.clone(),
            Pane {
                master: pair.master,
                writer,
                child,
                scrollback: Vec::new(),
                subscribers: Vec::new(),
                alive: true,
                status: StatusDetector::new(),
            },
        );
        drop(panes);
        Arc::clone(self).spawn_reader(req.pane_id.clone(), reader);
        Ok(())
    }

    /// Threads de leitura (bloqueante) + batching (coalesce por frame).
    fn spawn_reader(self: Arc<Self>, pane_id: PaneId, mut reader: Box<dyn Read + Send>) {
        let (tx, rx) = mpsc::channel::<Vec<u8>>();
        thread::spawn(move || {
            let mut buf = [0u8; 8192];
            loop {
                match reader.read(&mut buf) {
                    Ok(0) => break,
                    Ok(n) => {
                        if tx.send(buf[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                    Err(_) => break,
                }
            }
        });

        let mgr = self;
        thread::spawn(move || {
            const FRAME: Duration = Duration::from_millis(8);
            const MAX_BATCH: usize = 256 * 1024;
            loop {
                let first = match rx.recv() {
                    Ok(v) => v,
                    Err(_) => break,
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
                            mgr.on_output(&pane_id, &batch);
                            mgr.on_exit(&pane_id);
                            return;
                        }
                    }
                }
                mgr.on_output(&pane_id, &batch);
            }
            mgr.on_exit(&pane_id);
        });
    }

    fn on_output(&self, pane_id: &str, bytes: &[u8]) {
        if bytes.is_empty() {
            return;
        }
        let mut panes = self.panes.lock();
        if let Some(pane) = panes.get_mut(pane_id) {
            pane.scrollback.extend_from_slice(bytes);
            if pane.scrollback.len() > self.scrollback_cap {
                let overflow = pane.scrollback.len() - self.scrollback_cap;
                pane.scrollback.drain(0..overflow);
            }
            let msg = DaemonMessage::Output(TerminalOutput {
                pane_id: pane_id.to_string(),
                data_b64: b64(bytes),
            });
            // `send` em canal ilimitado não bloqueia; assinantes mortos caem fora.
            pane.subscribers.retain(|(_, tx)| tx.send(msg.clone()).is_ok());

            // Indicador: só emite quando o estado MUDA (barato).
            if let Some(state) = pane.status.on_output(bytes) {
                let msg = DaemonMessage::Status(PaneStatus {
                    pane_id: pane_id.to_string(),
                    state,
                });
                pane.subscribers.retain(|(_, tx)| tx.send(msg.clone()).is_ok());
            }
        }
    }

    fn on_exit(&self, pane_id: &str) {
        let mut panes = self.panes.lock();
        if let Some(pane) = panes.get_mut(pane_id) {
            pane.alive = false;
            let msg = DaemonMessage::Exit(TerminalExit {
                pane_id: pane_id.to_string(),
                code: None,
            });
            pane.subscribers.retain(|(_, tx)| tx.send(msg.clone()).is_ok());
        }
    }

    /// Assina um cliente ao pane: replay do scrollback, depois output ao vivo.
    /// Tudo sob o mesmo lock → sem buracos nem duplicatas entre replay e live.
    pub fn attach(&self, client_id: ClientId, tx: &Sender<DaemonMessage>, pane_id: &str) {
        let mut panes = self.panes.lock();
        match panes.get_mut(pane_id) {
            Some(pane) => {
                if !pane.scrollback.is_empty() {
                    let _ = tx.send(DaemonMessage::Scrollback(TerminalOutput {
                        pane_id: pane_id.to_string(),
                        data_b64: b64(&pane.scrollback),
                    }));
                }
                let _ = tx.send(DaemonMessage::AttachDone {
                    pane_id: pane_id.to_string(),
                });
                // Estado atual, pra UI já nascer com o indicador certo.
                let _ = tx.send(DaemonMessage::Status(PaneStatus {
                    pane_id: pane_id.to_string(),
                    state: pane.status.state(),
                }));
                if !pane.subscribers.iter().any(|(id, _)| *id == client_id) {
                    pane.subscribers.push((client_id, tx.clone()));
                }
                if !pane.alive {
                    let _ = tx.send(DaemonMessage::Exit(TerminalExit {
                        pane_id: pane_id.to_string(),
                        code: None,
                    }));
                }
            }
            None => {
                let _ = tx.send(DaemonMessage::Error {
                    message: format!("pane inexistente: {pane_id}"),
                });
            }
        }
    }

    pub fn detach(&self, client_id: ClientId, pane_id: &str) {
        let mut panes = self.panes.lock();
        if let Some(pane) = panes.get_mut(pane_id) {
            pane.subscribers.retain(|(id, _)| *id != client_id);
        }
    }

    pub fn write_input(&self, pane_id: &str, data: &[u8]) {
        let mut panes = self.panes.lock();
        if let Some(pane) = panes.get_mut(pane_id) {
            let _ = pane.writer.write_all(data);
            let _ = pane.writer.flush();
        }
    }

    pub fn resize(&self, pane_id: &str, cols: u16, rows: u16) {
        let panes = self.panes.lock();
        if let Some(pane) = panes.get(pane_id) {
            let _ = pane.master.resize(PtySize {
                rows: rows.max(1),
                cols: cols.max(1),
                pixel_width: 0,
                pixel_height: 0,
            });
        }
    }

    pub fn kill(&self, pane_id: &str) {
        if let Some(mut pane) = self.panes.lock().remove(pane_id) {
            let _ = pane.child.kill();
        }
    }

    /// Panes vivos — terminal e ACP juntos: a UI só quer saber o que dá para
    /// reatachar.
    pub fn list_panes(&self) -> Vec<PaneInfo> {
        let mut panes: Vec<PaneInfo> = self
            .panes
            .lock()
            .iter()
            .map(|(id, p)| PaneInfo {
                pane_id: id.clone(),
                alive: p.alive,
            })
            .collect();
        panes.extend(self.acp.pane_ids().into_iter().map(|pane_id| PaneInfo {
            pane_id,
            alive: true,
        }));
        panes
    }

    /// Remove o cliente de todos os panes (chamado quando a conexão cai).
    pub fn remove_client(&self, client_id: ClientId) {
        let mut panes = self.panes.lock();
        for pane in panes.values_mut() {
            pane.subscribers.retain(|(id, _)| *id != client_id);
        }
        drop(panes);
        self.acp.remove_client(client_id);
    }

    /// Despeja o scrollback de cada pane em disco (escrita atômica). Chamado no
    /// shutdown limpo do daemon.
    pub fn flush_scrollback(&self) {
        let panes = self.panes.lock();
        if std::fs::create_dir_all(&self.scrollback_dir).is_err() {
            return;
        }
        for (id, pane) in panes.iter() {
            let path = self.scrollback_dir.join(format!("{id}.log"));
            let tmp = self.scrollback_dir.join(format!("{id}.log.tmp"));
            if std::fs::write(&tmp, &pane.scrollback).is_ok() {
                let _ = std::fs::rename(&tmp, &path);
            }
        }
    }
}
