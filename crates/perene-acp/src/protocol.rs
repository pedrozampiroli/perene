//! Tipos do Agent Client Protocol (subconjunto que usamos).
//!
//! O wire é `camelCase`. Onde a spec é aberta (conteúdo de tool call, blocos de
//! conteúdo variados) guardamos `serde_json::Value`: modelar 100% do schema não
//! traria segurança real e quebraria a cada versão do protocolo.

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versão do protocolo que falamos.
pub const PROTOCOL_VERSION: u32 = 1;

// ── initialize ───────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FsCapabilities {
    pub read_text_file: bool,
    pub write_text_file: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ClientCapabilities {
    pub fs: FsCapabilities,
    /// Se `true`, o agente pode nos pedir para RODAR comandos (`terminal/*`).
    /// Desligar aqui é o jeito de negar execução ao agente.
    pub terminal: bool,
}

impl Default for ClientCapabilities {
    fn default() -> Self {
        Self {
            fs: FsCapabilities {
                read_text_file: true,
                write_text_file: true,
            },
            terminal: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Implementation {
    pub name: String,
    pub title: String,
    pub version: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct InitializeParams {
    pub protocol_version: u32,
    pub client_capabilities: ClientCapabilities,
    pub client_info: Implementation,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct InitializeResult {
    pub protocol_version: u32,
    pub agent_capabilities: Value,
    pub agent_info: Value,
    pub auth_methods: Vec<Value>,
}

// ── sessões ──────────────────────────────────────────────────────────────────

pub type SessionId = String;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionParams {
    /// Diretório de trabalho da sessão (o do pane/worktree).
    pub cwd: String,
    #[serde(default)]
    pub mcp_servers: Vec<Value>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct NewSessionResult {
    pub session_id: SessionId,
}

/// Bloco de conteúdo de um prompt (só texto por enquanto).
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ContentBlock {
    Text { text: String },
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptParams {
    pub session_id: SessionId,
    pub prompt: Vec<ContentBlock>,
}

/// Por que o turno terminou.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum StopReason {
    EndTurn,
    MaxTokens,
    MaxTurnRequests,
    Refusal,
    Cancelled,
    /// Valor novo que ainda não conhecemos — não quebra o cliente.
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptResult {
    pub stop_reason: StopReason,
}

// ── session/update (streaming) ───────────────────────────────────────────────

/// Atualizações que o agente empurra durante o turno.
///
/// `sessionUpdate` é o discriminante. Variantes desconhecidas caem em
/// [`SessionUpdate::Other`] — o protocolo evolui e não queremos quebrar.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "sessionUpdate", rename_all = "snake_case")]
pub enum SessionUpdate {
    /// Pedaço da resposta do agente.
    AgentMessageChunk {
        #[serde(default)]
        content: Value,
    },
    /// Pedaço do "raciocínio".
    AgentThoughtChunk {
        #[serde(default)]
        content: Value,
    },
    /// Eco do que o usuário mandou.
    UserMessageChunk {
        #[serde(default)]
        content: Value,
    },
    /// Uma ferramenta começou.
    #[serde(rename_all = "camelCase")]
    ToolCall {
        tool_call_id: String,
        #[serde(default)]
        title: String,
        #[serde(default)]
        kind: Option<String>,
        #[serde(default)]
        status: Option<String>,
    },
    /// Progresso/fim de uma ferramenta.
    #[serde(rename_all = "camelCase")]
    ToolCallUpdate {
        tool_call_id: String,
        #[serde(default)]
        status: Option<String>,
        #[serde(default)]
        content: Value,
    },
    /// Plano de execução.
    Plan {
        #[serde(default)]
        entries: Vec<Value>,
    },
    /// Consumo de contexto/custo.
    #[serde(rename_all = "camelCase")]
    UsageUpdate {
        #[serde(default)]
        used: u64,
        #[serde(default)]
        size: u64,
    },
    #[serde(other)]
    Other,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionUpdateParams {
    pub session_id: SessionId,
    pub update: SessionUpdate,
}

// ── permissão ────────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PermissionOption {
    pub option_id: String,
    pub name: String,
    #[serde(default)]
    pub kind: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RequestPermissionParams {
    pub session_id: SessionId,
    #[serde(default)]
    pub tool_call: Value,
    #[serde(default)]
    pub options: Vec<PermissionOption>,
}

/// Resposta a um pedido de permissão.
#[derive(Debug, Clone, Serialize)]
#[serde(tag = "outcome", rename_all = "camelCase")]
pub enum PermissionOutcome {
    /// O usuário escolheu uma das opções.
    #[serde(rename_all = "camelCase")]
    Selected { option_id: String },
    /// O turno foi cancelado antes de o usuário decidir.
    Cancelled,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn initialize_params_match_the_spec_wire() {
        let p = InitializeParams {
            protocol_version: PROTOCOL_VERSION,
            client_capabilities: ClientCapabilities::default(),
            client_info: Implementation {
                name: "perene".into(),
                title: "Perene".into(),
                version: "0.1.0".into(),
            },
        };
        let v = serde_json::to_value(&p).unwrap();
        assert_eq!(v["protocolVersion"], 1);
        assert_eq!(v["clientCapabilities"]["fs"]["readTextFile"], true);
        assert_eq!(v["clientCapabilities"]["fs"]["writeTextFile"], true);
        assert_eq!(v["clientCapabilities"]["terminal"], true);
        assert_eq!(v["clientInfo"]["name"], "perene");
    }

    #[test]
    fn parses_the_update_variants_we_render() {
        let msg: SessionUpdateParams = serde_json::from_value(json!({
            "sessionId": "s1",
            "update": {"sessionUpdate": "agent_message_chunk",
                       "content": {"type": "text", "text": "oi"}}
        }))
        .unwrap();
        assert_eq!(msg.session_id, "s1");
        match msg.update {
            SessionUpdate::AgentMessageChunk { content } => {
                assert_eq!(content["text"], "oi");
            }
            other => panic!("variante errada: {other:?}"),
        }

        let tc: SessionUpdateParams = serde_json::from_value(json!({
            "sessionId": "s1",
            "update": {"sessionUpdate": "tool_call", "toolCallId": "c1",
                       "title": "Lendo arquivo", "kind": "read", "status": "pending"}
        }))
        .unwrap();
        match tc.update {
            SessionUpdate::ToolCall { tool_call_id, title, .. } => {
                assert_eq!(tool_call_id, "c1");
                assert_eq!(title, "Lendo arquivo");
            }
            other => panic!("variante errada: {other:?}"),
        }
    }

    #[test]
    fn unknown_update_does_not_break_the_client() {
        // O protocolo evolui: variante nova tem que virar `Other`, não erro.
        let msg: SessionUpdateParams = serde_json::from_value(json!({
            "sessionId": "s1",
            "update": {"sessionUpdate": "algo_que_ainda_nao_existe", "x": 1}
        }))
        .unwrap();
        assert!(matches!(msg.update, SessionUpdate::Other));
    }

    #[test]
    fn stop_reason_round_trips_and_tolerates_novelty() {
        let r: PromptResult = serde_json::from_value(json!({"stopReason": "end_turn"})).unwrap();
        assert_eq!(r.stop_reason, StopReason::EndTurn);
        let r: PromptResult = serde_json::from_value(json!({"stopReason": "algo_novo"})).unwrap();
        assert_eq!(r.stop_reason, StopReason::Unknown);
    }

    #[test]
    fn permission_outcome_wire() {
        let v = serde_json::to_value(PermissionOutcome::Selected {
            option_id: "allow".into(),
        })
        .unwrap();
        assert_eq!(v["outcome"], "selected");
        assert_eq!(v["optionId"], "allow");
    }
}
