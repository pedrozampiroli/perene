//! JSON-RPC 2.0 sobre streams de linha (o transporte do ACP é stdio).
//!
//! É **genérico sobre `Read`/`Write`** de propósito: assim dá para testar o
//! protocolo inteiro com pipes em memória, sem depender de ter um agente de IA
//! instalado na máquina.
//!
//! A conexão é bidirecional: nós fazemos requisições ao agente *e* o agente faz
//! requisições a nós (pedir permissão, ler arquivo, rodar comando). Por isso um
//! [`PeerHandler`], e não só um cliente.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::mpsc::{self, Receiver, Sender, SyncSender};
use std::sync::Arc;
use std::thread;
use std::time::Duration;

use parking_lot::Mutex;
use serde_json::{json, Value};

/// Erro devolvido pelo outro lado (ou gerado por nós ao responder).
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct RpcError {
    pub code: i64,
    pub message: String,
}

impl RpcError {
    pub fn method_not_found(method: &str) -> Self {
        Self {
            code: -32601,
            message: format!("método não suportado: {method}"),
        }
    }
    pub fn internal(msg: impl Into<String>) -> Self {
        Self {
            code: -32603,
            message: msg.into(),
        }
    }
}

impl std::fmt::Display for RpcError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{} ({})", self.message, self.code)
    }
}
impl std::error::Error for RpcError {}

/// O que fazer com o que CHEGA do outro lado.
pub trait PeerHandler: Send + Sync + 'static {
    /// Requisição (tem `id`): precisa devolver resultado ou erro.
    fn on_request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
        let _ = params;
        Err(RpcError::method_not_found(method))
    }
    /// Notificação (sem `id`): não devolve nada.
    fn on_notification(&self, method: &str, params: Value) {
        let _ = (method, params);
    }
    /// A conexão terminou (processo saiu, pipe fechou).
    fn on_closed(&self) {}
}

type Pending = Arc<Mutex<HashMap<u64, SyncSender<Result<Value, RpcError>>>>>;

/// Conexão JSON-RPC viva: uma thread lê, outra escreve.
pub struct Connection {
    out: Sender<String>,
    pending: Pending,
    next_id: AtomicU64,
}

impl Connection {
    /// Começa a servir sobre os streams dados.
    pub fn start<R, W>(reader: R, writer: W, handler: Arc<dyn PeerHandler>) -> Arc<Self>
    where
        R: Read + Send + 'static,
        W: Write + Send + 'static,
    {
        let (tx, rx) = mpsc::channel::<String>();
        let conn = Arc::new(Self {
            out: tx,
            pending: Arc::new(Mutex::new(HashMap::new())),
            next_id: AtomicU64::new(1),
        });
        spawn_writer(writer, rx);
        spawn_reader(reader, Arc::clone(&conn), handler);
        conn
    }

    /// Requisição com resposta. `timeout` evita travar para sempre se o agente
    /// morrer no meio.
    pub fn request(
        &self,
        method: &str,
        params: Value,
        timeout: Duration,
    ) -> Result<Value, RpcError> {
        let id = self.next_id.fetch_add(1, Ordering::Relaxed);
        let (tx, rx) = mpsc::sync_channel(1);
        self.pending.lock().insert(id, tx);

        let line = json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params});
        if self.send(line).is_err() {
            self.pending.lock().remove(&id);
            return Err(RpcError::internal("conexão com o agente fechada"));
        }
        match rx.recv_timeout(timeout) {
            Ok(res) => res,
            Err(_) => {
                self.pending.lock().remove(&id);
                Err(RpcError::internal(format!("sem resposta para {method}")))
            }
        }
    }

    /// Notificação (sem resposta).
    pub fn notify(&self, method: &str, params: Value) {
        let _ = self.send(json!({"jsonrpc": "2.0", "method": method, "params": params}));
    }

    fn send(&self, value: Value) -> Result<(), ()> {
        let mut line = value.to_string();
        line.push('\n');
        self.out.send(line).map_err(|_| ())
    }

    /// Responde uma requisição que o agente nos fez.
    fn reply(&self, id: Value, result: Result<Value, RpcError>) {
        let msg = match result {
            Ok(v) => json!({"jsonrpc": "2.0", "id": id, "result": v}),
            Err(e) => json!({"jsonrpc": "2.0", "id": id, "error": {"code": e.code, "message": e.message}}),
        };
        let _ = self.send(msg);
    }
}

fn spawn_writer<W: Write + Send + 'static>(mut writer: W, rx: Receiver<String>) {
    thread::spawn(move || {
        for line in rx {
            if writer.write_all(line.as_bytes()).is_err() || writer.flush().is_err() {
                break;
            }
        }
    });
}

fn spawn_reader<R: Read + Send + 'static>(
    reader: R,
    conn: Arc<Connection>,
    handler: Arc<dyn PeerHandler>,
) {
    thread::spawn(move || {
        let buf = BufReader::new(reader);
        for line in buf.lines() {
            let Ok(line) = line else { break };
            if line.trim().is_empty() {
                continue;
            }
            let Ok(msg) = serde_json::from_str::<Value>(&line) else {
                continue; // lixo na linha: ignora em vez de derrubar a conexão
            };

            let has_id = !msg["id"].is_null();
            let method = msg["method"].as_str().map(String::from);

            match (has_id, method) {
                // Resposta a algo que pedimos.
                (true, None) => {
                    let Some(id) = msg["id"].as_u64() else { continue };
                    let waiter = conn.pending.lock().remove(&id);
                    if let Some(tx) = waiter {
                        let res = if msg["error"].is_null() {
                            Ok(msg["result"].clone())
                        } else {
                            Err(RpcError {
                                code: msg["error"]["code"].as_i64().unwrap_or(-32603),
                                message: msg["error"]["message"]
                                    .as_str()
                                    .unwrap_or("erro desconhecido")
                                    .to_string(),
                            })
                        };
                        let _ = tx.send(res);
                    }
                }
                // Requisição do agente para nós.
                (true, Some(m)) => {
                    let id = msg["id"].clone();
                    let params = msg["params"].clone();
                    // Em thread própria: o handler pode bloquear (ex.: esperar o
                    // usuário aprovar) e não pode travar a leitura.
                    let conn2 = Arc::clone(&conn);
                    let handler2 = Arc::clone(&handler);
                    thread::spawn(move || {
                        let res = handler2.on_request(&m, params);
                        conn2.reply(id, res);
                    });
                }
                // Notificação.
                (false, Some(m)) => handler.on_notification(&m, msg["params"].clone()),
                (false, None) => {}
            }
        }
        // Fim da conexão: destrava quem estiver esperando resposta.
        let waiters: Vec<_> = conn.pending.lock().drain().map(|(_, tx)| tx).collect();
        for tx in waiters {
            let _ = tx.send(Err(RpcError::internal("agente encerrou a conexão")));
        }
        handler.on_closed();
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicBool;

    /// Handler que grava as notificações recebidas e responde um ping.
    struct TestHandler {
        notes: Mutex<Vec<(String, Value)>>,
        closed: AtomicBool,
    }
    impl PeerHandler for TestHandler {
        fn on_request(&self, method: &str, params: Value) -> Result<Value, RpcError> {
            match method {
                "ask" => Ok(json!({"answer": params["q"].as_str().unwrap_or("").len()})),
                other => Err(RpcError::method_not_found(other)),
            }
        }
        fn on_notification(&self, method: &str, params: Value) {
            self.notes.lock().push((method.to_string(), params));
        }
        fn on_closed(&self) {
            self.closed.store(true, Ordering::SeqCst);
        }
    }

    /// Sobe um par de conexões ligadas por pipes (sem processo externo).
    fn duplex(
        h1: Arc<dyn PeerHandler>,
        h2: Arc<dyn PeerHandler>,
    ) -> (Arc<Connection>, Arc<Connection>) {
        let (a_read, b_write) = std::io::pipe().unwrap();
        let (b_read, a_write) = std::io::pipe().unwrap();
        (
            Connection::start(a_read, a_write, h1),
            Connection::start(b_read, b_write, h2),
        )
    }

    fn handler() -> Arc<TestHandler> {
        Arc::new(TestHandler {
            notes: Mutex::new(Vec::new()),
            closed: AtomicBool::new(false),
        })
    }

    #[test]
    fn request_gets_a_response() {
        let (ha, hb) = (handler(), handler());
        let (a, _b) = duplex(ha.clone(), hb.clone());
        let out = a
            .request("ask", json!({"q": "oi!"}), Duration::from_secs(5))
            .unwrap();
        assert_eq!(out["answer"], 3);
    }

    #[test]
    fn unknown_method_returns_error_without_killing_the_connection() {
        let (a, _b) = duplex(handler(), handler());
        let err = a
            .request("nope", json!({}), Duration::from_secs(5))
            .unwrap_err();
        assert_eq!(err.code, -32601);
        // A conexão continua utilizável depois do erro.
        let ok = a.request("ask", json!({"q": "12"}), Duration::from_secs(5));
        assert!(ok.is_ok());
    }

    #[test]
    fn notifications_reach_the_other_side() {
        let hb = handler();
        let (a, _b) = duplex(handler(), hb.clone());
        a.notify("session/update", json!({"sessionId": "s1"}));
        // A entrega é assíncrona; espera curta.
        for _ in 0..50 {
            if !hb.notes.lock().is_empty() {
                break;
            }
            thread::sleep(Duration::from_millis(10));
        }
        let notes = hb.notes.lock();
        assert_eq!(notes.len(), 1);
        assert_eq!(notes[0].0, "session/update");
        assert_eq!(notes[0].1["sessionId"], "s1");
    }

    #[test]
    fn garbage_lines_are_ignored() {
        let (a, b) = duplex(handler(), handler());
        // Injeta lixo pelo canal de saída de b (chega como linha inválida em a).
        b.out.send("isto não é json\n".to_string()).unwrap();
        // a continua respondendo normalmente.
        let ok = a.request("ask", json!({"q": "abc"}), Duration::from_secs(5));
        assert_eq!(ok.unwrap()["answer"], 3);
    }
}
