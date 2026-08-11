//! `perene-acp` — cliente do [Agent Client Protocol](https://agentclientprotocol.com).
//!
//! Modo alternativo ao terminal: em vez de rodar a CLI numa TUI e ler pixels, o
//! Perene conversa com o agente por JSON-RPC. A diferença que importa é **quem
//! manda**: no ACP o *cliente* é que executa comandos e mexe em arquivos, a
//! pedido do agente — então tudo passa por nós, com permissão e visibilidade.
//!
//! ```text
//!   Perene (cliente)                      Agente (adapter/CLI)
//!        │  initialize / session.new  ──────────▶
//!        │  session/prompt            ──────────▶
//!        │  ◀──────  session/update (streaming)
//!        │  ◀──────  session/request_permission     ← nosso diálogo
//!        │  ◀──────  fs/read · terminal/create      ← nós executamos
//! ```
//!
//! O núcleo JSON-RPC é genérico sobre `Read`/`Write` ([`jsonrpc`]), então o
//! protocolo é testado com pipes em memória, sem precisar de agente instalado.

pub mod jsonrpc;
pub mod protocol;

mod agent;

pub use agent::{Agent, AgentEvent, AgentHandler, SpawnConfig};
pub use jsonrpc::{PeerHandler, RpcError};
pub use protocol::*;
