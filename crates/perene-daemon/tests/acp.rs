//! E2E do modo ACP pelo IPC, sem UI e sem agente de IA instalado.
//!
//! O agente é um binário falso (`perene-fake-acp-agent`) que fala o protocolo de
//! verdade — então o caminho testado é o mesmo de produção: `Command::spawn` →
//! stdio → JSON-RPC → daemon → IPC → cliente.
//!
//! O que precisa ficar provado:
//!  1. o turno chega streamando na UI;
//!  2. um pedido de permissão trava o agente até o usuário responder;
//!  3. **fechar a janela no meio da conversa não perde nada** — reconectar e
//!     atachar reproduz o transcript inteiro. É a promessa do Perene aplicada ao
//!     chat, do mesmo jeito que o scrollback vale para o terminal.
//!
//! Lição #1: nada toca `~/.perene2` — tudo em diretório temporário.

use std::io::{Read, Write};
use std::path::Path;
use std::sync::mpsc;
use std::time::{Duration, Instant};

use perene_protocol::{decode_line, encode_line, AcpEvent, ClientMessage, DaemonMessage};

mod platform {
    use std::path::{Path, PathBuf};

    #[cfg(unix)]
    pub type Stream = std::os::unix::net::UnixStream;
    #[cfg(windows)]
    pub type Stream = perene_daemon::winpipe::PipeStream;

    #[cfg(unix)]
    pub fn endpoint(dir: &Path) -> PathBuf {
        dir.join("daemon.sock")
    }

    #[cfg(windows)]
    pub fn endpoint(_dir: &Path) -> PathBuf {
        use std::sync::atomic::{AtomicU32, Ordering};
        static SEQ: AtomicU32 = AtomicU32::new(0);
        let n = SEQ.fetch_add(1, Ordering::Relaxed);
        PathBuf::from(format!(r"\\.\pipe\perene2-acp-{}-{n}", std::process::id()))
    }

    #[cfg(unix)]
    pub fn connect(endpoint: &Path) -> std::io::Result<Stream> {
        Stream::connect(endpoint)
    }

    #[cfg(windows)]
    pub fn connect(endpoint: &Path) -> std::io::Result<Stream> {
        perene_daemon::winpipe::connect(endpoint)
    }

    #[cfg(unix)]
    pub fn shutdown(stream: &Stream) {
        let _ = stream.shutdown(std::net::Shutdown::Both);
    }

    #[cfg(windows)]
    pub fn shutdown(stream: &Stream) {
        stream.shutdown();
    }
}

use platform::Stream;

/// Cliente de linha com timeout (mesmo desenho do teste de protocolo).
struct LineClient {
    stream: Stream,
    rx: mpsc::Receiver<Vec<u8>>,
    buf: Vec<u8>,
}

impl LineClient {
    fn connect(endpoint: &Path) -> std::io::Result<Self> {
        let stream = platform::connect(endpoint)?;
        let mut read_side = stream.try_clone()?;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let mut tmp = [0u8; 8192];
            loop {
                match read_side.read(&mut tmp) {
                    Ok(0) | Err(_) => break,
                    Ok(n) => {
                        if tx.send(tmp[..n].to_vec()).is_err() {
                            break;
                        }
                    }
                }
            }
        });
        Ok(Self {
            stream,
            rx,
            buf: Vec::new(),
        })
    }

    fn send(&mut self, msg: &ClientMessage) {
        let line = encode_line(msg).unwrap();
        self.stream.write_all(line.as_bytes()).unwrap();
        self.stream.flush().unwrap();
    }

    fn next_msg(&mut self, deadline: Instant) -> Option<DaemonMessage> {
        loop {
            if let Some(pos) = self.buf.iter().position(|&b| b == b'\n') {
                let line: Vec<u8> = self.buf.drain(0..=pos).collect();
                let text = String::from_utf8_lossy(&line[..line.len() - 1]).to_string();
                if text.trim().is_empty() {
                    continue;
                }
                return decode_line(&text).ok();
            }
            let now = Instant::now();
            if now >= deadline {
                return None;
            }
            match self.rx.recv_timeout(deadline - now) {
                Ok(chunk) => self.buf.extend_from_slice(&chunk),
                Err(_) => return None,
            }
        }
    }

    /// Coleta eventos ACP até `stop` dizer que já viu o bastante (ou dar timeout).
    fn collect_acp(
        &mut self,
        timeout: Duration,
        stop: impl Fn(&[AcpEvent]) -> bool,
    ) -> Vec<AcpEvent> {
        let deadline = Instant::now() + timeout;
        let mut events = Vec::new();
        while Instant::now() < deadline {
            if let Some(DaemonMessage::Acp(m)) = self.next_msg(deadline) {
                events.push(m.event);
                if stop(&events) {
                    break;
                }
            }
        }
        events
    }
}

impl Drop for LineClient {
    fn drop(&mut self) {
        // Sem isto o daemon não vê o cliente sair — e é justamente "a janela
        // fechou" que este teste precisa simular.
        platform::shutdown(&self.stream);
    }
}

fn start_daemon(dir: &Path) -> std::path::PathBuf {
    let endpoint = platform::endpoint(dir);
    let config = perene_daemon::Config {
        socket_path: endpoint.clone(),
        lock_path: dir.join("daemon.lock"),
        scrollback_dir: dir.join("scrollback"),
        scrollback_cap: 1024 * 1024,
    };
    std::thread::spawn(move || {
        let _ = perene_daemon::run(config);
    });
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline {
        if platform::connect(&endpoint).is_ok() {
            return endpoint;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("daemon não subiu em {}", endpoint.display());
}

fn hello(client: &mut LineClient) {
    client.send(&ClientMessage::Hello {
        protocol_version: perene_daemon::PROTOCOL_VERSION,
    });
}

fn spawn_fake_agent(client: &mut LineClient, pane_id: &str, cwd: &Path) {
    spawn_fake_agent_with(client, pane_id, cwd, Vec::new(), false);
}

fn spawn_fake_agent_with(
    client: &mut LineClient,
    pane_id: &str,
    cwd: &Path,
    args: Vec<String>,
    allow_terminal: bool,
) {
    client.send(&ClientMessage::AcpSpawn {
        pane_id: pane_id.to_string(),
        cwd: cwd.to_string_lossy().to_string(),
        program: env!("CARGO_BIN_EXE_perene-fake-acp-agent").to_string(),
        args,
        allow_terminal,
    });
    client.send(&ClientMessage::Attach {
        pane_id: pane_id.to_string(),
    });
}

fn text_of(update: &serde_json::Value) -> String {
    update["content"]["text"].as_str().unwrap_or("").to_string()
}

/// Manda um prompt e devolve o que o agente respondeu em texto.
fn ask(client: &mut LineClient, pane_id: &str, prompt: &str) -> String {
    client.send(&ClientMessage::AcpPrompt {
        pane_id: pane_id.to_string(),
        text: prompt.to_string(),
    });
    client
        .collect_acp(Duration::from_secs(15), |evs| {
            evs.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. }))
        })
        .iter()
        .filter_map(|e| match e {
            AcpEvent::Update { update } => Some(text_of(update)),
            AcpEvent::Failed { message } => Some(format!("FALHOU: {message}")),
            _ => None,
        })
        .collect()
}

fn ready(client: &mut LineClient) {
    let evs = client.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Ready))
    });
    assert!(
        evs.iter().any(|e| matches!(e, AcpEvent::Ready)),
        "a sessão devia ficar pronta: {evs:?}"
    );
}

#[test]
fn full_turn_streams_asks_permission_and_survives_the_window_closing() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());
    let pane_id = "pane_acp";

    // ── Janela 1: abre a sessão e conversa ────────────────────────────────
    let mut c1 = LineClient::connect(&endpoint).unwrap();
    hello(&mut c1);
    spawn_fake_agent(&mut c1, pane_id, dir.path());

    let ready = c1.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Ready))
    });
    assert!(
        ready.iter().any(|e| matches!(e, AcpEvent::Ready)),
        "a sessão devia ficar pronta; veio {ready:?}"
    );

    c1.send(&ClientMessage::AcpPrompt {
        pane_id: pane_id.to_string(),
        text: "rode os testes".into(),
    });

    // Streaming + o pedido de permissão, que trava o agente.
    let until_permission = c1.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Permission { .. }))
    });
    assert!(
        until_permission.iter().any(|e| matches!(
            e,
            AcpEvent::Update { update } if text_of(update).contains("rode os testes")
        )),
        "faltou o streaming do turno: {until_permission:?}"
    );
    let request_id = until_permission
        .iter()
        .find_map(|e| match e {
            AcpEvent::Permission { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("o agente devia ter pedido permissão");

    // Enquanto ninguém responde, o turno NÃO termina.
    let quiet = c1.collect_acp(Duration::from_millis(400), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. }))
    });
    assert!(
        !quiet
            .iter()
            .any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "o agente não pode seguir sem a resposta do usuário: {quiet:?}"
    );

    c1.send(&ClientMessage::AcpPermission {
        pane_id: pane_id.to_string(),
        request_id,
        option_id: Some("allow".into()),
    });
    let ended = c1.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. }))
    });
    let stop = ended
        .iter()
        .find_map(|e| match e {
            AcpEvent::TurnEnded { stop_reason } => Some(stop_reason.clone()),
            _ => None,
        })
        .expect("o turno devia terminar após a permissão");
    assert!(
        stop.to_lowercase().contains("endturn"),
        "permitimos a ação, então o fim tinha que ser normal: {stop}"
    );

    // ── A janela fecha ────────────────────────────────────────────────────
    drop(c1);

    // ── Janela 2: reatacha e reencontra a conversa inteira ────────────────
    let mut c2 = LineClient::connect(&endpoint).unwrap();
    hello(&mut c2);
    c2.send(&ClientMessage::Attach {
        pane_id: pane_id.to_string(),
    });

    let replay = c2.collect_acp(Duration::from_secs(5), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. }))
    });
    assert!(
        replay.iter().any(|e| matches!(e, AcpEvent::Ready)),
        "o replay começa do início da sessão: {replay:?}"
    );
    assert!(
        replay.iter().any(|e| matches!(
            e,
            AcpEvent::Update { update } if text_of(update).contains("rode os testes")
        )),
        "a resposta do agente tinha que voltar no replay: {replay:?}"
    );
    assert!(
        replay
            .iter()
            .any(|e| matches!(e, AcpEvent::TurnEnded { .. })),
        "o fim do turno também faz parte do transcript: {replay:?}"
    );

    // A sessão continua utilizável depois de reabrir a janela.
    c2.send(&ClientMessage::AcpPrompt {
        pane_id: pane_id.to_string(),
        text: "de novo".into(),
    });
    let again = c2.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| {
            matches!(
                e,
                AcpEvent::Update { update } if text_of(update).contains("de novo")
            )
        })
    });
    assert!(
        again.iter().any(|e| matches!(
            e,
            AcpEvent::Update { update } if text_of(update).contains("de novo")
        )),
        "depois do reattach ainda dá para conversar: {again:?}"
    );
}

#[test]
fn the_agent_reads_files_through_us_and_only_inside_the_session() {
    // O caminho inverso: quem lê o disco é o Perene, a pedido do agente. E é
    // aqui que o escopo vale — no terminal a CLI leria o que quisesse.
    let dir = tempfile::tempdir().unwrap();
    let root = dir.path().join("projeto");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("dentro.txt"), "CONTEUDO_PERMITIDO").unwrap();
    std::fs::write(dir.path().join("fora.txt"), "CONTEUDO_PROIBIDO").unwrap();

    let endpoint = start_daemon(dir.path());
    let pane_id = "pane_fs";
    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    spawn_fake_agent(&mut c, pane_id, &root);
    ready(&mut c);

    let dentro = ask(&mut c, pane_id, "#fs dentro.txt");
    assert!(
        dentro.contains("CONTEUDO_PERMITIDO"),
        "ler dentro do escopo tem que funcionar: {dentro}"
    );

    let fora = ask(&mut c, pane_id, "#fs ../fora.txt");
    assert!(
        !fora.contains("CONTEUDO_PROIBIDO"),
        "vazou conteúdo de fora do diretório da sessão: {fora}"
    );
    assert!(
        fora.contains("fora do diretório"),
        "o agente devia receber um erro claro: {fora}"
    );
}

#[cfg(unix)]
#[test]
fn the_agent_runs_commands_through_us_only_when_allowed() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());
    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);

    // Sem a capacidade declarada, rodar comando é recusado.
    spawn_fake_agent_with(&mut c, "pane_sem", dir.path(), Vec::new(), false);
    ready(&mut c);
    let negado = ask(&mut c, "pane_sem", "#sh echo MARCA_PROIBIDA");
    assert!(
        !negado.contains("MARCA_PROIBIDA"),
        "comando rodou sem permissão: {negado}"
    );
    assert!(
        negado.contains("não permite"),
        "o agente devia saber por que foi recusado: {negado}"
    );

    // Com a capacidade, o comando roda e a saída volta pelo streaming.
    spawn_fake_agent_with(&mut c, "pane_com", dir.path(), Vec::new(), true);
    ready(&mut c);
    let saida = ask(&mut c, "pane_com", "#sh echo MARCA_PERMITIDA");
    assert!(
        saida.contains("MARCA_PERMITIDA"),
        "a saída do comando devia chegar ao agente: {saida}"
    );
}

#[test]
fn killing_the_pane_takes_the_whole_process_tree_down() {
    // O adapter real roda via `npx`, que vira `node`: matar só o filho direto
    // deixaria o neto vivo segurando a sessão e ~100 MB — um por pane fechado.
    // O agente falso reproduz a topologia com um neto que carimba um arquivo;
    // se o carimbo parar, a árvore morreu de verdade.
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());
    let pane_id = "pane_kill";
    let beat = dir.path().join("heartbeat");

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    c.send(&ClientMessage::AcpSpawn {
        pane_id: pane_id.to_string(),
        cwd: dir.path().to_string_lossy().to_string(),
        program: env!("CARGO_BIN_EXE_perene-fake-acp-agent").to_string(),
        args: vec![
            "--spawn-heartbeat".into(),
            beat.to_string_lossy().to_string(),
        ],
        allow_terminal: false,
    });
    c.send(&ClientMessage::Attach {
        pane_id: pane_id.to_string(),
    });
    let evs = c.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Ready))
    });
    assert!(evs.iter().any(|e| matches!(e, AcpEvent::Ready)));

    // O neto tem que estar batendo antes de julgarmos a morte dele.
    let deadline = Instant::now() + Duration::from_secs(5);
    while Instant::now() < deadline && !beat.exists() {
        std::thread::sleep(Duration::from_millis(20));
    }
    assert!(beat.exists(), "o neto devia estar vivo antes do kill");

    c.send(&ClientMessage::Kill {
        pane_id: pane_id.to_string(),
    });
    std::thread::sleep(Duration::from_millis(300));
    let after_kill = std::fs::metadata(&beat).unwrap().modified().unwrap();
    std::thread::sleep(Duration::from_millis(500)); // 10× o intervalo do carimbo
    let later = std::fs::metadata(&beat).unwrap().modified().unwrap();
    assert_eq!(
        after_kill, later,
        "o neto continuou batendo: o kill não alcançou a árvore"
    );

    // E o pane some da listagem — não fica fantasma no daemon.
    c.send(&ClientMessage::ListPanes);
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut listed = None;
    while Instant::now() < deadline && listed.is_none() {
        if let Some(DaemonMessage::Panes { panes }) = c.next_msg(deadline) {
            listed = Some(panes);
        }
    }
    let panes = listed.expect("o daemon devia responder ListPanes");
    assert!(
        !panes.iter().any(|p| p.pane_id == pane_id),
        "o pane morto ainda aparece: {panes:?}"
    );
}

#[test]
fn denying_permission_refuses_the_turn() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());
    let pane_id = "pane_deny";

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    spawn_fake_agent(&mut c, pane_id, dir.path());
    c.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Ready))
    });

    c.send(&ClientMessage::AcpPrompt {
        pane_id: pane_id.to_string(),
        text: "apague tudo".into(),
    });
    let evs = c.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Permission { .. }))
    });
    let request_id = evs
        .iter()
        .find_map(|e| match e {
            AcpEvent::Permission { request_id, .. } => Some(*request_id),
            _ => None,
        })
        .expect("devia ter pedido permissão");

    c.send(&ClientMessage::AcpPermission {
        pane_id: pane_id.to_string(),
        request_id,
        option_id: Some("deny".into()),
    });
    let ended = c.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::TurnEnded { .. }))
    });
    let stop = ended
        .iter()
        .find_map(|e| match e {
            AcpEvent::TurnEnded { stop_reason } => Some(stop_reason.clone()),
            _ => None,
        })
        .expect("negar também encerra o turno");
    assert!(
        stop.to_lowercase().contains("refusal"),
        "negar tem que refletir no fim do turno: {stop}"
    );
}

/// Mesma coisa, mas contra o adapter **de verdade**. Fora do CI de propósito:
/// depende de `npx`, rede e de o usuário estar logado no Claude.
///
/// `cargo test -p perene-daemon --test acp -- --ignored --nocapture`
#[test]
#[ignore = "precisa de npx + login no Claude"]
fn real_adapter_answers_a_prompt() {
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());
    let pane_id = "pane_real";

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    c.send(&ClientMessage::AcpSpawn {
        pane_id: pane_id.into(),
        cwd: dir.path().to_string_lossy().to_string(),
        program: "npx".into(),
        args: vec!["-y".into(), "@zed-industries/claude-agent-acp".into()],
        allow_terminal: false,
    });
    c.send(&ClientMessage::Attach {
        pane_id: pane_id.into(),
    });

    // `npx` pode baixar o adapter na primeira vez.
    let evs = c.collect_acp(Duration::from_secs(180), |evs| {
        evs.iter()
            .any(|e| matches!(e, AcpEvent::Ready | AcpEvent::Failed { .. }))
    });
    println!("handshake: {evs:?}");
    assert!(
        evs.iter().any(|e| matches!(e, AcpEvent::Ready)),
        "não ficou pronto: {evs:?}"
    );

    c.send(&ClientMessage::AcpPrompt {
        pane_id: pane_id.into(),
        text: "Responda apenas: PERENE_OK".into(),
    });
    let turn = c.collect_acp(Duration::from_secs(180), |evs| {
        evs.iter()
            .any(|e| matches!(e, AcpEvent::TurnEnded { .. } | AcpEvent::Failed { .. }))
    });
    let said: String = turn
        .iter()
        .filter_map(|e| match e {
            AcpEvent::Update { update } => Some(text_of(update)),
            _ => None,
        })
        .collect();
    println!("resposta: {said}");
    assert!(
        said.contains("PERENE_OK"),
        "o agente devia ter respondido; eventos: {turn:?}"
    );
}

#[test]
fn spawn_failure_reaches_the_client() {
    // Adapter ausente (npx sem rede, nome errado) é o erro mais provável na vida
    // real: precisa virar mensagem na tela, não silêncio.
    let dir = tempfile::tempdir().unwrap();
    let endpoint = start_daemon(dir.path());

    let mut c = LineClient::connect(&endpoint).unwrap();
    hello(&mut c);
    c.send(&ClientMessage::AcpSpawn {
        pane_id: "pane_broken".into(),
        cwd: dir.path().to_string_lossy().to_string(),
        program: "perene-adapter-que-nao-existe".into(),
        args: Vec::new(),
        allow_terminal: false,
    });
    c.send(&ClientMessage::Attach {
        pane_id: "pane_broken".into(),
    });

    let evs = c.collect_acp(Duration::from_secs(10), |evs| {
        evs.iter().any(|e| matches!(e, AcpEvent::Failed { .. }))
    });
    assert!(
        evs.iter().any(|e| matches!(
            e,
            AcpEvent::Failed { message } if message.contains("perene-adapter-que-nao-existe")
        )),
        "o erro devia dizer qual programa faltou: {evs:?}"
    );
}
