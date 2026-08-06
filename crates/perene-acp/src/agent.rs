//! Cliente de alto nível: sobe o agente e expõe as operações do ACP.

use std::collections::VecDeque;
use std::io::{Read, Write};
use std::process::{Child, Command, Stdio};
use std::sync::Arc;
use std::time::Duration;

use parking_lot::Mutex;

use serde_json::{json, Value};

use crate::jsonrpc::{Connection, PeerHandler, RpcError};
use crate::protocol::*;

/// Requisições longas (um turno pode demorar minutos).
const PROMPT_TIMEOUT: Duration = Duration::from_secs(60 * 30);
/// Requisições de controle, que precisam responder rápido.
const CONTROL_TIMEOUT: Duration = Duration::from_secs(60);

/// O que o Perene precisa saber/decidir enquanto o agente trabalha.
///
/// Os métodos de `fs`/`terminal` chegam na fase 3; por ora o default recusa,
/// o que é seguro (o agente trata como capacidade indisponível).
pub trait AgentHandler: Send + Sync + 'static {
    /// Streaming do turno (mensagem, raciocínio, tool calls, plano…).
    fn on_event(&self, event: AgentEvent);

    /// O agente quer permissão para uma ação. Bloqueia até o usuário decidir.
    fn request_permission(&self, params: RequestPermissionParams) -> PermissionOutcome {
        let _ = params;
        PermissionOutcome::Cancelled
    }

    /// Requisições `fs/*` e `terminal/*` (fase 3).
    fn on_client_method(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let _ = params;
        Err(RpcError::method_not_found(method))
    }
}

/// Eventos entregues à UI.
#[derive(Debug, Clone)]
pub enum AgentEvent {
    Update {
        session_id: SessionId,
        update: SessionUpdate,
    },
    /// O processo do agente terminou.
    Closed,
}

/// Como subir o agente.
#[derive(Debug, Clone)]
pub struct SpawnConfig {
    pub program: String,
    pub args: Vec<String>,
    /// Diretório de trabalho do processo.
    pub cwd: Option<String>,
}

impl SpawnConfig {
    /// Adapter oficial do Claude (roda via `npx`, sem instalar nada global).
    pub fn claude() -> Self {
        Self {
            program: "npx".into(),
            args: vec!["-y".into(), "@zed-industries/claude-agent-acp".into()],
            cwd: None,
        }
    }
}

/// Ponte entre o `PeerHandler` (cru) e o `AgentHandler` (do app).
struct Bridge {
    handler: Arc<dyn AgentHandler>,
}

impl PeerHandler for Bridge {
    fn on_request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        match method {
            "session/request_permission" => {
                let p: RequestPermissionParams = serde_json::from_value(params)
                    .map_err(|e| RpcError::internal(format!("params inválidos: {e}")))?;
                let outcome = self.handler.request_permission(p);
                serde_json::to_value(outcome).map_err(|e| RpcError::internal(e.to_string()))
            }
            other => self.handler.on_client_method(other, params),
        }
    }

    fn on_notification(&self, method: &str, params: Value) {
        if method == "session/update" {
            if let Ok(p) = serde_json::from_value::<SessionUpdateParams>(params) {
                self.handler.on_event(AgentEvent::Update {
                    session_id: p.session_id,
                    update: p.update,
                });
            }
        }
    }

    fn on_closed(&self) {
        self.handler.on_event(AgentEvent::Closed);
    }
}

/// Quantas linhas de stderr do adapter guardamos para explicar uma falha.
const STDERR_TAIL: usize = 20;

/// Um agente ACP vivo.
pub struct Agent {
    conn: Arc<Connection>,
    child: Option<Child>,
    /// Últimas linhas de stderr do adapter.
    ///
    /// Quando o handshake falha, o JSON-RPC não tem o que dizer — o processo
    /// nem chegou a falar. Quem explica é o stderr ("command not found",
    /// "Invalid permissions.defaultMode"), então ele precisa sobreviver até a
    /// hora de montar a mensagem de erro.
    stderr: Arc<Mutex<VecDeque<String>>>,
}

impl Agent {
    /// Sobe o processo do agente e começa a falar JSON-RPC pelo stdio dele.
    pub fn spawn(cfg: &SpawnConfig, handler: Arc<dyn AgentHandler>) -> std::io::Result<Self> {
        let mut cmd = Command::new(&cfg.program);
        // Sem isto o adapter se acha aninhado numa sessão de harness e recusa
        // abrir sessão (erro interno no `session/new`). Mesma limpeza dos PTYs.
        for key in perene_core::harness_env::inherited_session_vars() {
            cmd.env_remove(&key);
        }
        // Grupo de processos próprio: o adapter roda via `npx`, que vira `node`.
        // Sem o grupo, matar o filho direto deixaria o neto vivo — ver [`kill_tree`].
        #[cfg(unix)]
        {
            use std::os::unix::process::CommandExt;
            cmd.process_group(0);
        }
        cmd.args(&cfg.args)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            // Capturado, não herdado: o log do adapter é a única pista quando o
            // handshake falha, mas herdar nosso stderr vazaria o fd para netos
            // que não conseguimos matar (o `npx` vira `node`) — e aí quem nos
            // spawnou nunca vê o pipe fechar.
            .stderr(Stdio::piped());
        if let Some(cwd) = &cfg.cwd {
            cmd.current_dir(cwd);
        }
        let mut child = cmd.spawn()?;
        let stdout = child.stdout.take().expect("stdout pedido acima");
        let stdin = child.stdin.take().expect("stdin pedido acima");
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(stderr) = child.stderr.take() {
            let program = cfg.program.clone();
            let tail = Arc::clone(&tail);
            std::thread::spawn(move || {
                use std::io::BufRead;
                for line in std::io::BufReader::new(stderr)
                    .lines()
                    .map_while(Result::ok)
                {
                    eprintln!("[acp:{program}] {line}");
                    let mut tail = tail.lock();
                    tail.push_back(line);
                    if tail.len() > STDERR_TAIL {
                        tail.pop_front();
                    }
                }
            });
        }
        let conn = Connection::start(stdout, stdin, Arc::new(Bridge { handler }));
        Ok(Self {
            conn,
            child: Some(child),
            stderr: tail,
        })
    }

    /// Últimas linhas que o adapter escreveu em stderr (vazio se ficou quieto).
    pub fn stderr_tail(&self) -> String {
        self.stderr
            .lock()
            .iter()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    }

    /// Conecta em streams já existentes (usado nos testes, sem processo).
    pub fn connect<R, W>(reader: R, writer: W, handler: Arc<dyn AgentHandler>) -> Self
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        Self {
            conn: Connection::start(reader, writer, Arc::new(Bridge { handler })),
            child: None,
            stderr: Arc::new(Mutex::new(VecDeque::new())),
        }
    }

    /// Handshake. Declara o que NÓS sabemos fazer pelo agente.
    pub fn initialize(&self, caps: ClientCapabilities) -> Result<InitializeResult, RpcError> {
        let params = InitializeParams {
            protocol_version: PROTOCOL_VERSION,
            client_capabilities: caps,
            client_info: Implementation {
                name: "perene".into(),
                title: "Perene".into(),
                version: env!("CARGO_PKG_VERSION").into(),
            },
        };
        let v = self.conn.request(
            "initialize",
            serde_json::to_value(params).map_err(|e| RpcError::internal(e.to_string()))?,
            CONTROL_TIMEOUT,
        )?;
        serde_json::from_value(v).map_err(|e| RpcError::internal(e.to_string()))
    }

    /// Abre uma sessão no diretório dado.
    pub fn new_session(&self, cwd: &str) -> Result<SessionId, RpcError> {
        let params = NewSessionParams {
            cwd: cwd.to_string(),
            mcp_servers: Vec::new(),
        };
        let v = self.conn.request(
            "session/new",
            serde_json::to_value(params).map_err(|e| RpcError::internal(e.to_string()))?,
            CONTROL_TIMEOUT,
        )?;
        let r: NewSessionResult =
            serde_json::from_value(v).map_err(|e| RpcError::internal(e.to_string()))?;
        Ok(r.session_id)
    }

    /// Manda um prompt e espera o turno terminar. As respostas parciais chegam
    /// antes, pelo `AgentHandler::on_event`.
    pub fn prompt(&self, session_id: &str, text: &str) -> Result<StopReason, RpcError> {
        let params = PromptParams {
            session_id: session_id.to_string(),
            prompt: vec![ContentBlock::Text {
                text: text.to_string(),
            }],
        };
        let v = self.conn.request(
            "session/prompt",
            serde_json::to_value(params).map_err(|e| RpcError::internal(e.to_string()))?,
            PROMPT_TIMEOUT,
        )?;
        let r: PromptResult =
            serde_json::from_value(v).map_err(|e| RpcError::internal(e.to_string()))?;
        Ok(r.stop_reason)
    }

    /// Interrompe o turno atual (notificação: não espera resposta).
    pub fn cancel(&self, session_id: &str) {
        self.conn
            .notify("session/cancel", json!({ "sessionId": session_id }));
    }
}

impl Drop for Agent {
    fn drop(&mut self) {
        if let Some(child) = &mut self.child {
            kill_tree(child);
        }
    }
}

/// Mata o agente **e seus descendentes**.
///
/// `npx` só empacota: quem realmente roda é um `node` filho dele. Matando apenas
/// o processo que spawnamos, o `node` fica órfão segurando a sessão e ~100 MB
/// para sempre — um por pane fechado. Com o alvo de RAM do Perene, isso não é
/// detalhe.
#[cfg(unix)]
fn kill_tree(child: &mut Child) {
    // O filho nasce como líder do próprio grupo (`process_group(0)`), então o
    // grupo é exatamente ele + descendentes.
    let pid = child.id() as i32;
    unsafe {
        libc::killpg(pid, libc::SIGKILL);
    }
    let _ = child.kill();
    let _ = child.wait();
}

#[cfg(windows)]
fn kill_tree(child: &mut Child) {
    // No Windows não há grupo herdado; `taskkill /T` percorre a árvore.
    let _ = Command::new("taskkill")
        .args(["/T", "/F", "/PID", &child.id().to_string()])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status();
    let _ = child.kill();
    let _ = child.wait();
}
