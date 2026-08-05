//! Fluxo completo do ACP contra um **agente falso** em memória.
//!
//! Sem processo externo e sem agente de IA instalado: os dois lados conversam
//! por pipes. É o que garante que o cliente segue a spec (nomes de campo,
//! ordem das mensagens, streaming) mesmo no CI.

use std::sync::Arc;
use std::time::{Duration, Instant};

use parking_lot::Mutex;
use serde_json::{json, Value};

use perene_acp::{Agent, AgentEvent, AgentHandler, ClientCapabilities, SessionUpdate, StopReason};

/// Coleta o que a UI receberia.
#[derive(Default)]
struct Recorder {
    events: Mutex<Vec<AgentEvent>>,
    /// Resposta que daremos a um pedido de permissão.
    answer: Mutex<Option<String>>,
}

impl AgentHandler for Recorder {
    fn on_event(&self, event: AgentEvent) {
        self.events.lock().push(event);
    }
    fn request_permission(
        &self,
        params: perene_acp::RequestPermissionParams,
    ) -> perene_acp::PermissionOutcome {
        let chosen = self
            .answer
            .lock()
            .clone()
            .unwrap_or_else(|| params.options.first().map(|o| o.option_id.clone()).unwrap_or_default());
        perene_acp::PermissionOutcome::Selected { option_id: chosen }
    }
}

/// Agente falso: responde `initialize`/`session/new`/`session/prompt`, e durante
/// o prompt faz streaming e pede permissão — como um agente de verdade.
fn fake_agent(reader: std::io::PipeReader, mut writer: std::io::PipeWriter) {
    use std::io::{BufRead, BufReader, Write};

    let mut buf = BufReader::new(reader);
    let mut line = String::new();
    let send = move |v: Value, w: &mut std::io::PipeWriter| {
        let _ = w.write_all(format!("{v}\n").as_bytes());
        let _ = w.flush();
    };

    loop {
        line.clear();
        match buf.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg["id"].clone();
        match msg["method"].as_str().unwrap_or("") {
            "initialize" => send(
                json!({"jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion": 1,
                    "agentCapabilities": {"loadSession": true},
                    "agentInfo": {"name":"fake","title":"Fake","version":"1"},
                    "authMethods": []
                }}),
                &mut writer,
            ),
            "session/new" => send(
                json!({"jsonrpc":"2.0","id":id,"result":{"sessionId":"sess_1"}}),
                &mut writer,
            ),
            "session/prompt" => {
                // 1. streaming de texto
                send(
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_1",
                        "update":{"sessionUpdate":"agent_message_chunk",
                                  "content":{"type":"text","text":"pensando…"}}}}),
                    &mut writer,
                );
                // 2. uma tool call
                send(
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_1",
                        "update":{"sessionUpdate":"tool_call","toolCallId":"c1",
                                  "title":"Rodar testes","kind":"execute","status":"pending"}}}),
                    &mut writer,
                );
                // 3. pede permissão e ESPERA a resposta do cliente
                send(
                    json!({"jsonrpc":"2.0","id":9001,"method":"session/request_permission","params":{
                        "sessionId":"sess_1",
                        "toolCall":{"toolCallId":"c1"},
                        "options":[{"optionId":"allow","name":"Permitir","kind":"allow_once"},
                                   {"optionId":"deny","name":"Negar","kind":"reject_once"}]}}),
                    &mut writer,
                );
                // lê a resposta da permissão
                let mut perm = String::new();
                let _ = buf.read_line(&mut perm);
                // Negar também vem como "selected": o que decide é a OPÇÃO escolhida.
                let granted = perm.contains("\"optionId\":\"allow\"");
                // 4. variante desconhecida: o cliente tem que sobreviver
                send(
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_1",
                        "update":{"sessionUpdate":"variante_do_futuro","x":1}}}),
                    &mut writer,
                );
                // 5. fecha o turno
                send(
                    json!({"jsonrpc":"2.0","id":id,"result":{
                        "stopReason": if granted {"end_turn"} else {"refusal"}}}),
                    &mut writer,
                );
            }
            _ => {
                if !id.is_null() {
                    send(json!({"jsonrpc":"2.0","id":id,"result":{}}), &mut writer);
                }
            }
        }
    }
}

fn connect() -> (Agent, Arc<Recorder>) {
    let (agent_read, client_write) = std::io::pipe().unwrap();
    let (client_read, agent_write) = std::io::pipe().unwrap();
    std::thread::spawn(move || fake_agent(agent_read, agent_write));
    let rec = Arc::new(Recorder::default());
    let agent = Agent::connect(client_read, client_write, rec.clone());
    (agent, rec)
}

#[test]
fn full_turn_streams_updates_and_asks_permission() {
    let (agent, rec) = connect();

    let init = agent.initialize(ClientCapabilities::default()).unwrap();
    assert_eq!(init.protocol_version, 1);

    let session = agent.new_session("/tmp/projeto").unwrap();
    assert_eq!(session, "sess_1");

    let stop = agent.prompt(&session, "rode os testes").unwrap();
    assert_eq!(
        stop,
        StopReason::EndTurn,
        "permitimos a ação, então o turno devia terminar normalmente"
    );

    // O streaming precisa ter chegado à UI.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && rec.events.lock().len() < 3 {
        std::thread::sleep(Duration::from_millis(10));
    }
    let events = rec.events.lock();
    let mut saw_text = false;
    let mut saw_tool = false;
    let mut saw_unknown = false;
    for e in events.iter() {
        if let AgentEvent::Update { update, .. } = e {
            match update {
                SessionUpdate::AgentMessageChunk { content } => {
                    if content["text"] == "pensando…" {
                        saw_text = true;
                    }
                }
                SessionUpdate::ToolCall { title, .. } if title == "Rodar testes" => saw_tool = true,
                SessionUpdate::Other => saw_unknown = true,
                _ => {}
            }
        }
    }
    assert!(saw_text, "faltou o chunk de texto");
    assert!(saw_tool, "faltou a tool call");
    assert!(saw_unknown, "variante desconhecida devia virar Other, não sumir");
}

#[test]
fn denying_permission_stops_the_turn() {
    let (agent, rec) = connect();
    *rec.answer.lock() = Some("deny".to_string());

    agent.initialize(ClientCapabilities::default()).unwrap();
    let session = agent.new_session("/tmp/projeto").unwrap();
    let stop = agent.prompt(&session, "apague tudo").unwrap();

    assert_eq!(
        stop,
        StopReason::Refusal,
        "negar a permissão precisa refletir no fim do turno"
    );
}

#[test]
fn capabilities_can_deny_terminal_access() {
    // Negar `terminal` é o jeito de o usuário impedir o agente de rodar comandos.
    let caps = ClientCapabilities {
        terminal: false,
        ..Default::default()
    };
    let v = serde_json::to_value(&caps).unwrap();
    assert_eq!(v["terminal"], false);
    assert_eq!(v["fs"]["readTextFile"], true);
}
