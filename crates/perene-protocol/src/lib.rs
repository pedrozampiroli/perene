//! Tipos do protocolo compartilhados entre a UI (Tauri), o daemon e o core.
//!
//! No M0 apenas os tipos de terminal são usados (PTY direto no processo Tauri).
//! A partir do M1 estes mesmos tipos viajam pelo IPC (unix socket / named pipe)
//! em formato JSON-lines. Manter serialização estável: `camelCase` no wire, para
//! casar com o front (xterm.js) sem conversões manuais.

use serde::{Deserialize, Serialize};

/// Identificador imutável de um pane (nunca reciclar). Formato `pane_<uuid-curto>`.
pub type PaneId = String;

/// Pedido de criação de um terminal.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SpawnRequest {
    pub pane_id: PaneId,
    pub cols: u16,
    pub rows: u16,
    /// Diretório inicial. `None` = `$HOME`.
    #[serde(default)]
    pub cwd: Option<String>,
    /// Comando a rodar antes de cair no shell interativo. `None` = login shell puro.
    #[serde(default)]
    pub command: Option<String>,
    /// Programa do shell (ex.: `/bin/bash`, `wsl.exe`). `None` = padrão do sistema.
    #[serde(default)]
    pub shell: Option<String>,
}

/// Chunk de saída do PTY (coalescido por frame no lado Rust), já em base64.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalOutput {
    pub pane_id: PaneId,
    /// Bytes crus do PTY em base64 (preserva UTF-8/binário sem corromper).
    pub data_b64: String,
}

/// Notificação de que o processo do pane terminou.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TerminalExit {
    pub pane_id: PaneId,
    /// Código de saída, quando conhecido.
    #[serde(default)]
    pub code: Option<i32>,
}

/// Nomes de evento Tauri (webview) — mantidos aqui para não divergirem entre
/// Rust e TypeScript.
pub mod events {
    pub const PTY_OUTPUT: &str = "pty-output";
    pub const PTY_EXIT: &str = "pty-exit";
}

/// Versão do protocolo IPC UI ⇄ daemon. Bump quando o wire mudar de forma
/// incompatível — o daemon rejeita clientes com versão diferente.
pub const PROTOCOL_VERSION: u32 = 1;

/// Metadados de um pane vivo no daemon.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PaneInfo {
    pub pane_id: PaneId,
    /// `false` quando o processo já saiu mas o pane ainda existe (mostra saída).
    pub alive: bool,
}

/// Mensagens que a UI (cliente) envia ao daemon. Uma por linha (JSON-lines).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ClientMessage {
    /// Handshake inicial.
    Hello { protocol_version: u32 },
    /// Cria um PTY (idempotente por `pane_id`).
    Spawn(SpawnRequest),
    /// Passa a receber output do pane + replay do scrollback.
    Attach { pane_id: PaneId },
    /// Para de receber output (mas o PTY segue vivo).
    Detach { pane_id: PaneId },
    /// Input do usuário → shell (bytes crus em base64).
    Write { pane_id: PaneId, data_b64: String },
    /// Redimensiona o PTY.
    Resize { pane_id: PaneId, cols: u16, rows: u16 },
    /// Mata o processo do pane e remove-o.
    Kill { pane_id: PaneId },
    /// Lista os panes vivos.
    ListPanes,
    /// Keepalive.
    Ping,
    /// Pede shutdown limpo do daemon (flush de scrollback).
    Shutdown,
}

/// Mensagens que o daemon envia à UI. Uma por linha (JSON-lines).
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum DaemonMessage {
    /// Resposta ao `Hello`.
    Welcome { protocol_version: u32, daemon_pid: u32 },
    /// Output ao vivo (coalescido por frame).
    Output(TerminalOutput),
    /// Replay de scrollback enviado logo após um `Attach`.
    Scrollback(TerminalOutput),
    /// Fim do replay de scrollback do pane.
    AttachDone { pane_id: PaneId },
    /// O processo do pane terminou.
    Exit(TerminalExit),
    /// Resposta ao `ListPanes`.
    Panes { panes: Vec<PaneInfo> },
    /// Resposta ao `Ping`.
    Pong,
    /// Erro (ex.: pane inexistente, versão incompatível).
    Error { message: String },
}

/// Framing JSON-lines: serializa `msg` numa linha terminada em `\n`.
pub fn encode_line<T: Serialize>(msg: &T) -> Result<String, serde_json::Error> {
    let mut s = serde_json::to_string(msg)?;
    s.push('\n');
    Ok(s)
}

/// Desserializa uma linha (sem o `\n`) numa mensagem.
pub fn decode_line<T: for<'de> Deserialize<'de>>(line: &str) -> Result<T, serde_json::Error> {
    serde_json::from_str(line)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spawn_request_uses_camel_case_on_the_wire() {
        let req = SpawnRequest {
            pane_id: "pane_abc".into(),
            cols: 80,
            rows: 24,
            cwd: None,
            command: None,
        };
        let json = serde_json::to_string(&req).unwrap();
        assert!(json.contains("\"paneId\""), "wire deve ser camelCase: {json}");
    }

    #[test]
    fn terminal_output_round_trips() {
        let out = TerminalOutput {
            pane_id: "pane_1".into(),
            data_b64: "aGVsbG8=".into(),
        };
        let json = serde_json::to_string(&out).unwrap();
        let back: TerminalOutput = serde_json::from_str(&json).unwrap();
        assert_eq!(back.pane_id, out.pane_id);
        assert_eq!(back.data_b64, out.data_b64);
    }

    #[test]
    fn client_message_is_tagged_and_line_framed() {
        let msg = ClientMessage::Attach {
            pane_id: "pane_x".into(),
        };
        let line = encode_line(&msg).unwrap();
        assert!(line.ends_with('\n'));
        assert!(line.contains("\"type\":\"attach\""), "linha: {line}");
        let back: ClientMessage = decode_line(line.trim_end()).unwrap();
        matches!(back, ClientMessage::Attach { .. })
            .then_some(())
            .expect("deve voltar como Attach");
    }

    #[test]
    fn daemon_message_variants_round_trip() {
        let msgs = vec![
            DaemonMessage::Welcome {
                protocol_version: PROTOCOL_VERSION,
                daemon_pid: 42,
            },
            DaemonMessage::Output(TerminalOutput {
                pane_id: "p".into(),
                data_b64: "AA==".into(),
            }),
            DaemonMessage::AttachDone {
                pane_id: "p".into(),
            },
            DaemonMessage::Pong,
        ];
        for m in msgs {
            let line = encode_line(&m).unwrap();
            let _back: DaemonMessage = decode_line(line.trim_end()).unwrap();
        }
    }
}
