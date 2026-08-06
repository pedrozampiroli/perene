//! Agente ACP **falso**, usado pelo teste de integração (`tests/acp.rs`).
//!
//! Existe como binário de verdade porque o caminho que queremos testar começa
//! num `Command::spawn`: só assim o teste exercita processo, stdio e JSON-RPC
//! como em produção — sem precisar de `npx`, rede ou conta de IA. Funciona igual
//! nas três plataformas, então o e2e do ACP roda no CI inteiro.
//!
//! Não é usado pelo app.

use std::io::{BufRead, BufReader, Write};

use serde_json::{json, Value};

fn send(out: &mut impl Write, v: Value) {
    let _ = writeln!(out, "{v}");
    let _ = out.flush();
}

/// Modo "neto": só carimba um arquivo para sempre.
///
/// Serve para o teste provar que o `kill` alcança a árvore inteira — em produção
/// quem está aqui é o `node` que o `npx` spawna, e que já vazou de verdade.
fn heartbeat(path: &str) -> ! {
    loop {
        let _ = std::fs::write(path, b"tick");
        std::thread::sleep(std::time::Duration::from_millis(50));
    }
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if let Some(i) = args.iter().position(|a| a == "--heartbeat") {
        heartbeat(&args[i + 1]);
    }
    // `--spawn-heartbeat <path>`: vira o "npx" da história e deixa um neto vivo.
    if let Some(i) = args.iter().position(|a| a == "--spawn-heartbeat") {
        let _ = std::process::Command::new(&args[0])
            .args(["--heartbeat", &args[i + 1]])
            .spawn();
    }

    let stdin = std::io::stdin();
    let mut reader = BufReader::new(stdin.lock());
    let stdout = std::io::stdout();
    let mut out = stdout.lock();

    let mut line = String::new();
    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) | Err(_) => break,
            Ok(_) => {}
        }
        let Ok(msg) = serde_json::from_str::<Value>(&line) else {
            continue;
        };
        let id = msg["id"].clone();
        match msg["method"].as_str().unwrap_or("") {
            "initialize" => send(
                &mut out,
                json!({"jsonrpc":"2.0","id":id,"result":{
                    "protocolVersion": 1,
                    "agentCapabilities": {},
                    "agentInfo": {"name":"fake","title":"Fake","version":"1"},
                    "authMethods": []
                }}),
            ),
            "session/new" => send(
                &mut out,
                json!({"jsonrpc":"2.0","id":id,"result":{"sessionId":"sess_fake"}}),
            ),
            "session/prompt" => {
                let prompt = msg["params"]["prompt"][0]["text"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();
                send(
                    &mut out,
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_fake",
                        "update":{"sessionUpdate":"agent_message_chunk",
                                  "content":{"type":"text","text":format!("eco: {prompt}")}}}}),
                );
                send(
                    &mut out,
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_fake",
                        "update":{"sessionUpdate":"tool_call","toolCallId":"c1",
                                  "title":"Rodar testes","kind":"execute","status":"pending"}}}),
                );
                // Pede permissão e ESPERA — é o ponto onde o agente fica travado
                // até o usuário decidir na UI.
                send(
                    &mut out,
                    json!({"jsonrpc":"2.0","id":9001,"method":"session/request_permission","params":{
                        "sessionId":"sess_fake",
                        "toolCall":{"toolCallId":"c1","title":"Rodar testes"},
                        "options":[{"optionId":"allow","name":"Permitir","kind":"allow_once"},
                                   {"optionId":"deny","name":"Negar","kind":"reject_once"}]}}),
                );
                let mut answer = String::new();
                let _ = reader.read_line(&mut answer);
                // Negar também chega como "selected": quem decide é a OPÇÃO.
                let granted = answer.contains("\"optionId\":\"allow\"");
                send(
                    &mut out,
                    json!({"jsonrpc":"2.0","method":"session/update","params":{
                        "sessionId":"sess_fake",
                        "update":{"sessionUpdate":"tool_call_update","toolCallId":"c1",
                                  "status": if granted {"completed"} else {"failed"}}}}),
                );
                send(
                    &mut out,
                    json!({"jsonrpc":"2.0","id":id,"result":{
                        "stopReason": if granted {"end_turn"} else {"refusal"}}}),
                );
            }
            _ => {
                if !id.is_null() {
                    send(&mut out, json!({"jsonrpc":"2.0","id":id,"result":{}}));
                }
            }
        }
    }
}
