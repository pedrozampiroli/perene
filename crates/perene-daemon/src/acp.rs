//! Sessões ACP vivendo no daemon.
//!
//! Mesmo contrato dos PTYs: a sessão pertence ao daemon, não à janela. Fechar a
//! UI não mata a conversa; ao reatachar, o cliente recebe o **transcript** de
//! volta (o análogo do scrollback) e continua do ponto em que estava.
//!
//! Permissão é a parte delicada: o agente *pergunta* e fica parado esperando. O
//! handler roda numa thread do JSON-RPC e **bloqueia** até o usuário decidir —
//! por isso cada pedido ganha um `request_id` e um canal, e a resposta vinda da
//! UI destrava exatamente aquele pedido.

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{self, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};

use perene_acp::{
    Agent, AgentEvent, AgentHandler, ClientCapabilities, FsCapabilities, PermissionOutcome,
    RequestPermissionParams, RpcError, SpawnConfig,
};
use perene_protocol::{AcpEvent, AcpMessage, DaemonMessage, PaneId, PaneState, PaneStatus};

use crate::acp_client::ClientTools;
use crate::status::DONE_TTL;

/// Quanto esperamos o usuário decidir uma permissão antes de desistir. Generoso
/// de propósito: o normal é o usuário sair para almoçar no meio de um diff.
const PERMISSION_TIMEOUT: Duration = Duration::from_secs(60 * 30);
/// Teto de eventos guardados para replay.
const TRANSCRIPT_CAP: usize = 4000;

/// Uma sessão ACP viva.
///
/// É ela quem implementa [`AgentHandler`]: o que vem do agente vira evento para
/// os assinantes, sem intermediário.
struct AcpSession {
    pane_id: PaneId,
    /// `None` até o processo subir; zerado no `kill` para matar o filho.
    agent: Mutex<Option<Arc<Agent>>>,
    session_id: Mutex<Option<String>>,
    /// Eventos já emitidos, para o replay no reattach.
    transcript: Mutex<Vec<AcpEvent>>,
    /// Pedidos de permissão aguardando resposta da UI.
    pending: Mutex<HashMap<u64, SyncSender<PermissionOutcome>>>,
    subscribers: Mutex<Vec<(u64, Sender<DaemonMessage>)>>,
    next_request: AtomicU64,
    /// Estado do indicador + geração (para o "verde" expirar sozinho).
    state: Mutex<PaneState>,
    state_gen: AtomicU64,
    /// `true` depois do `kill`: encerramento pedido por nós não é erro.
    closing: AtomicBool,
    /// Executor do que o agente pede (`fs/*`, `terminal/*`), preso ao diretório
    /// da sessão.
    tools: ClientTools,
}

impl AcpSession {
    fn new(pane_id: &str, cwd: &str, allow_terminal: bool) -> Self {
        Self {
            pane_id: pane_id.to_string(),
            agent: Mutex::new(None),
            session_id: Mutex::new(None),
            transcript: Mutex::new(Vec::new()),
            pending: Mutex::new(HashMap::new()),
            subscribers: Mutex::new(Vec::new()),
            next_request: AtomicU64::new(1),
            state: Mutex::new(PaneState::Idle),
            state_gen: AtomicU64::new(0),
            closing: AtomicBool::new(false),
            tools: ClientTools::new(cwd, allow_terminal),
        }
    }

    fn broadcast(&self, msg: DaemonMessage) {
        self.subscribers
            .lock()
            .retain(|(_, tx)| tx.send(msg.clone()).is_ok());
    }

    /// Publica um evento: guarda no transcript e manda para quem está atachado.
    fn emit(&self, event: AcpEvent) {
        {
            let mut t = self.transcript.lock();
            t.push(event.clone());
            if t.len() > TRANSCRIPT_CAP {
                let overflow = t.len() - TRANSCRIPT_CAP;
                t.drain(0..overflow);
            }
        }
        self.broadcast(DaemonMessage::Acp(AcpMessage {
            pane_id: self.pane_id.clone(),
            event,
        }));
    }

    /// Move o indicador. `Done` volta sozinho para `Idle` depois de um tempo —
    /// mesma regra do terminal, para as duas bolinhas se comportarem igual.
    fn set_state(self: &Arc<Self>, state: PaneState) {
        {
            let mut cur = self.state.lock();
            if *cur == state {
                return;
            }
            *cur = state;
        }
        let generation = self.state_gen.fetch_add(1, Ordering::Relaxed) + 1;
        self.broadcast(DaemonMessage::Status(PaneStatus {
            pane_id: self.pane_id.clone(),
            state,
        }));
        if state == PaneState::Done {
            let session = Arc::clone(self);
            thread::spawn(move || {
                thread::sleep(DONE_TTL);
                // Só apaga o verde se nada aconteceu nesse meio-tempo.
                if session.state_gen.load(Ordering::Relaxed) == generation {
                    session.set_state(PaneState::Idle);
                }
            });
        }
    }
}

impl AgentHandler for AcpSession {
    fn on_event(&self, event: AgentEvent) {
        match event {
            AgentEvent::Update { update, .. } => {
                let value = serde_json::to_value(update).unwrap_or_else(|_| json!({}));
                self.emit(AcpEvent::Update { update: value });
            }
            AgentEvent::Closed => {
                // Fechamos nós? Então não é erro — é o usuário fechando o pane.
                if self.closing.load(Ordering::Relaxed) {
                    return;
                }
                // Não mexe no estado aqui: quem estava no meio de um prompt já
                // vai receber o erro do turno, com mensagem melhor.
                self.emit(AcpEvent::Failed {
                    message: "o agente encerrou a conexão".into(),
                });
            }
        }
    }

    fn request_permission(&self, params: RequestPermissionParams) -> PermissionOutcome {
        let request_id = self.next_request.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending.lock().insert(request_id, tx);

        self.emit(AcpEvent::Permission {
            request_id,
            tool_call: params.tool_call.clone(),
            options: serde_json::to_value(&params.options).unwrap_or_else(|_| json!([])),
        });
        self.broadcast(DaemonMessage::Status(PaneStatus {
            pane_id: self.pane_id.clone(),
            state: PaneState::Waiting,
        }));
        *self.state.lock() = PaneState::Waiting;

        // Bloqueia esperando a UI. O timeout evita prender o agente para sempre
        // se ninguém responder (usuário fechou a janela e foi embora).
        match rx.recv_timeout(PERMISSION_TIMEOUT) {
            Ok(outcome) => outcome,
            Err(_) => {
                self.pending.lock().remove(&request_id);
                PermissionOutcome::Cancelled
            }
        }
    }

    /// `fs/*` e `terminal/*`: o agente pede, nós executamos — dentro do escopo.
    fn on_client_method(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        self.tools.handle(method, params)
    }
}

/// Todas as sessões ACP do daemon.
#[derive(Default)]
pub struct AcpManager {
    sessions: Mutex<HashMap<PaneId, Arc<AcpSession>>>,
}

impl AcpManager {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn has(&self, pane_id: &str) -> bool {
        self.sessions.lock().contains_key(pane_id)
    }

    pub fn pane_ids(&self) -> Vec<PaneId> {
        self.sessions.lock().keys().cloned().collect()
    }

    fn get(&self, pane_id: &str) -> Option<Arc<AcpSession>> {
        self.sessions.lock().get(pane_id).cloned()
    }

    /// Sobe o agente e abre a sessão. Idempotente por `pane_id`.
    ///
    /// A sessão entra no mapa **antes** do processo subir: se o spawn falhar, o
    /// erro tem um transcript onde cair e a UI o vê ao atachar.
    pub fn spawn(
        &self,
        pane_id: &str,
        cwd: &str,
        program: &str,
        args: &[String],
        allow_terminal: bool,
    ) {
        let session = {
            let mut sessions = self.sessions.lock();
            if sessions.contains_key(pane_id) {
                return;
            }
            let session = Arc::new(AcpSession::new(pane_id, cwd, allow_terminal));
            sessions.insert(pane_id.to_string(), Arc::clone(&session));
            session
        };

        let cfg = SpawnConfig {
            program: program.to_string(),
            args: args.to_vec(),
            cwd: Some(cwd.to_string()),
        };
        let cwd = cwd.to_string();
        // Fora do lock: subir o processo e fazer o handshake conversa com o
        // agente e pode demorar segundos (o `npx` às vezes baixa o adapter).
        thread::spawn(move || {
            let agent = match Agent::spawn(&cfg, Arc::clone(&session) as Arc<dyn AgentHandler>) {
                Ok(a) => Arc::new(a),
                Err(e) => {
                    session.emit(AcpEvent::Failed {
                        message: format!("não consegui subir `{}`: {e}", cfg.program),
                    });
                    session.set_state(PaneState::Error);
                    return;
                }
            };
            *session.agent.lock() = Some(Arc::clone(&agent));

            let caps = ClientCapabilities {
                fs: FsCapabilities {
                    read_text_file: true,
                    write_text_file: true,
                },
                terminal: allow_terminal,
            };
            if let Err(e) = agent.initialize(caps) {
                session.emit(AcpEvent::Failed {
                    message: format!("handshake falhou: {e}"),
                });
                session.set_state(PaneState::Error);
                return;
            }
            match agent.new_session(&cwd) {
                Ok(id) => {
                    *session.session_id.lock() = Some(id);
                    session.emit(AcpEvent::Ready);
                }
                Err(e) => {
                    session.emit(AcpEvent::Failed {
                        message: format!("não consegui abrir a sessão: {e}"),
                    });
                    session.set_state(PaneState::Error);
                }
            }
        });
    }

    /// Manda um prompt. Não bloqueia o loop do cliente: o turno pode durar minutos.
    pub fn prompt(&self, pane_id: &str, text: &str) {
        let Some(session) = self.get(pane_id) else {
            return;
        };
        let Some(session_id) = session.session_id.lock().clone() else {
            session.emit(AcpEvent::Failed {
                message: "a sessão ainda não está pronta".into(),
            });
            return;
        };
        let Some(agent) = session.agent.lock().clone() else {
            return;
        };
        let text = text.to_string();
        thread::spawn(move || {
            session.set_state(PaneState::Running);
            match agent.prompt(&session_id, &text) {
                Ok(stop) => {
                    session.emit(AcpEvent::TurnEnded {
                        stop_reason: format!("{stop:?}"),
                    });
                    session.set_state(PaneState::Done);
                }
                Err(e) => {
                    session.emit(AcpEvent::Failed {
                        message: e.to_string(),
                    });
                    session.set_state(PaneState::Error);
                }
            }
        });
    }

    pub fn cancel(&self, pane_id: &str) {
        let Some(session) = self.get(pane_id) else {
            return;
        };
        let agent = session.agent.lock().clone();
        let id = session.session_id.lock().clone();
        if let (Some(agent), Some(id)) = (agent, id) {
            agent.cancel(&id);
        }
        // Um pedido de permissão pendente também precisa ser solto, senão a
        // thread do JSON-RPC fica presa até o timeout.
        let pending: Vec<_> = session.pending.lock().drain().map(|(_, tx)| tx).collect();
        for tx in pending {
            let _ = tx.send(PermissionOutcome::Cancelled);
        }
    }

    /// Resposta do usuário a um pedido de permissão: destrava o agente.
    pub fn answer_permission(&self, pane_id: &str, request_id: u64, option_id: Option<String>) {
        let Some(session) = self.get(pane_id) else {
            return;
        };
        let waiter = session.pending.lock().remove(&request_id);
        if let Some(tx) = waiter {
            let outcome = match option_id {
                Some(option_id) => PermissionOutcome::Selected { option_id },
                None => PermissionOutcome::Cancelled,
            };
            let _ = tx.send(outcome);
            // O agente volta a trabalhar; o indicador acompanha.
            session.set_state(PaneState::Running);
        }
    }

    /// Atacha um cliente e reproduz o transcript (o "scrollback" do ACP).
    pub fn attach(&self, client_id: u64, tx: &Sender<DaemonMessage>, pane_id: &str) -> bool {
        let Some(session) = self.get(pane_id) else {
            return false;
        };
        // Snapshot sob lock, envio fora dele: evita segurar o transcript enquanto
        // escrevemos no socket.
        let events = session.transcript.lock().clone();
        for event in events {
            let _ = tx.send(DaemonMessage::Acp(AcpMessage {
                pane_id: pane_id.to_string(),
                event,
            }));
        }
        let _ = tx.send(DaemonMessage::AttachDone {
            pane_id: pane_id.to_string(),
        });
        let _ = tx.send(DaemonMessage::Status(PaneStatus {
            pane_id: pane_id.to_string(),
            state: *session.state.lock(),
        }));
        let mut subs = session.subscribers.lock();
        if !subs.iter().any(|(id, _)| *id == client_id) {
            subs.push((client_id, tx.clone()));
        }
        true
    }

    pub fn detach(&self, client_id: u64, pane_id: &str) {
        if let Some(session) = self.get(pane_id) {
            session
                .subscribers
                .lock()
                .retain(|(id, _)| *id != client_id);
        }
    }

    pub fn remove_client(&self, client_id: u64) {
        for session in self.sessions.lock().values() {
            session
                .subscribers
                .lock()
                .retain(|(id, _)| *id != client_id);
        }
    }

    /// Encerra a sessão e mata o processo do agente.
    ///
    /// Tirar o `Agent` do `Mutex` é o que importa: o agente segura um `Arc` da
    /// sessão (é o handler dele), então só remover do mapa deixaria o ciclo em
    /// pé — e o processo filho vivo.
    pub fn kill(&self, pane_id: &str) {
        let Some(session) = self.sessions.lock().remove(pane_id) else {
            return;
        };
        session.closing.store(true, Ordering::Relaxed);
        let pending: Vec<_> = session.pending.lock().drain().map(|(_, tx)| tx).collect();
        for tx in pending {
            let _ = tx.send(PermissionOutcome::Cancelled);
        }
        drop(session.agent.lock().take()); // `Agent::drop` mata o filho.
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn manager() -> AcpManager {
        AcpManager::new()
    }

    #[test]
    fn spawn_failure_lands_in_the_transcript() {
        // Programa que não existe: o erro precisa chegar na UI, não sumir.
        let mgr = manager();
        mgr.spawn("pane_1", ".", "perene-programa-que-nao-existe", &[], false);
        let (tx, rx) = mpsc::channel();

        // O spawn é assíncrono; espera o evento aparecer.
        let mut failed = None;
        for _ in 0..100 {
            let attached = mgr.attach(1, &tx, "pane_1");
            assert!(attached, "a sessão entra no mapa antes de o processo subir");
            while let Ok(msg) = rx.try_recv() {
                if let DaemonMessage::Acp(AcpMessage {
                    event: AcpEvent::Failed { message },
                    ..
                }) = msg
                {
                    failed = Some(message);
                }
            }
            if failed.is_some() {
                break;
            }
            mgr.detach(1, "pane_1");
            thread::sleep(Duration::from_millis(20));
        }
        let message = failed.expect("spawn falho tem que virar AcpEvent::Failed");
        assert!(
            message.contains("perene-programa-que-nao-existe"),
            "a mensagem deve dizer QUAL programa faltou: {message}"
        );
    }

    #[test]
    fn spawn_is_idempotent_per_pane() {
        let mgr = manager();
        mgr.spawn("pane_1", ".", "perene-inexistente", &[], false);
        mgr.spawn("pane_1", ".", "perene-inexistente", &[], false);
        assert_eq!(mgr.pane_ids().len(), 1);
    }

    #[test]
    fn permission_answer_unblocks_the_waiting_agent() {
        // Sem processo nenhum: exercita só a correlação request_id → canal, que
        // é o que trava o agente de verdade.
        let session = Arc::new(AcpSession::new("pane_1", ".", false));
        let mgr = manager();
        mgr.sessions
            .lock()
            .insert("pane_1".into(), Arc::clone(&session));

        let waiter = {
            let session = Arc::clone(&session);
            thread::spawn(move || {
                session.request_permission(RequestPermissionParams {
                    session_id: "s1".into(),
                    tool_call: json!({"toolCallId": "c1"}),
                    options: serde_json::from_value(json!([
                        {"optionId": "allow", "name": "Permitir"},
                        {"optionId": "deny", "name": "Negar"}
                    ]))
                    .unwrap(),
                })
            })
        };

        // Espera o pedido ser registrado antes de responder.
        for _ in 0..200 {
            if !session.pending.lock().is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(5));
        }
        mgr.answer_permission("pane_1", 1, Some("deny".into()));

        let outcome = waiter.join().expect("thread da permissão");
        match outcome {
            PermissionOutcome::Selected { option_id } => assert_eq!(option_id, "deny"),
            other => panic!("esperava a opção escolhida, veio {other:?}"),
        }
    }

    #[test]
    fn attach_replays_the_transcript() {
        let session = Arc::new(AcpSession::new("pane_1", ".", false));
        let mgr = manager();
        mgr.sessions
            .lock()
            .insert("pane_1".into(), Arc::clone(&session));
        session.emit(AcpEvent::Ready);
        session.emit(AcpEvent::Update {
            update: json!({"sessionUpdate": "agent_message_chunk"}),
        });

        let (tx, rx) = mpsc::channel();
        assert!(mgr.attach(7, &tx, "pane_1"));

        let mut acp_events = 0;
        let mut saw_attach_done = false;
        while let Ok(msg) = rx.try_recv() {
            match msg {
                DaemonMessage::Acp(_) => acp_events += 1,
                DaemonMessage::AttachDone { .. } => saw_attach_done = true,
                _ => {}
            }
        }
        assert_eq!(
            acp_events, 2,
            "quem chega depois tem que ver a conversa toda"
        );
        assert!(saw_attach_done, "a UI espera o fim do replay");
    }

    #[test]
    fn attach_to_unknown_pane_is_not_an_acp_session() {
        let mgr = manager();
        let (tx, _rx) = mpsc::channel();
        assert!(!mgr.attach(1, &tx, "pane_fantasma"));
    }
}
