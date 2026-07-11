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
}
