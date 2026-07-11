//! `perene-daemon` — servidor de sessões de terminal.
//!
//! Placeholder do M0: no M1 este crate vira um binário que gerencia 1 PTY por
//! pane (portable-pty), com scrollback em memória, IPC por unix socket / named
//! pipe (JSON-lines), attach/detach e single-instance via lock no socket.
//! Por ora só reexporta o protocolo para fixar a dependência no workspace.

pub use perene_protocol as protocol;

/// Versão do protocolo do daemon. Bump quando o wire mudar de forma incompatível.
pub const PROTOCOL_VERSION: u32 = 1;
