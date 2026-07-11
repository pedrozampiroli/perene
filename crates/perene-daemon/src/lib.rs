//! `perene-daemon` — servidor de sessões de terminal.
//!
//! Processo separado da UI: gerencia 1 PTY por pane (portable-pty), guarda
//! scrollback em memória e sobrevive ao fechamento da janela. A UI é um cliente
//! que atacha/detacha via IPC (JSON-lines) e recebe replay de scrollback no
//! reattach. Single-instance garantido por flock (lição #2).

pub mod pty;
pub mod server;
pub mod session;

pub use server::{acquire_single_instance, run, Config, SingleInstance};
pub use session::SessionManager;

/// Reexporta o protocolo para clientes/testes.
pub use perene_protocol as protocol;

/// Versão do protocolo IPC (espelha o crate de protocolo).
pub const PROTOCOL_VERSION: u32 = perene_protocol::PROTOCOL_VERSION;
