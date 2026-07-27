//! Teste automatizado do protocolo do daemon — SEM UI (aceite do M1).
//!
//! Cobre o cenário do critério de aceite: fechar a janela e reabrir → mesmo
//! shell, mesmo processo vivo, scrollback preservado. Modelamos "fechar/reabrir"
//! como desconectar um cliente e conectar outro; o PTY vive no daemon entre os dois.
//!
//! Lição #1: NADA toca `~/.perene2`. Tudo em diretório temporário injetado.

#![cfg(unix)]

use std::io::{Read, Write};
use std::os::unix::net::UnixStream;
use std::path::Path;
use std::time::{Duration, Instant};

use base64::Engine;
use perene_protocol::{decode_line, encode_line, ClientMessage, DaemonMessage, SpawnRequest};

const MARKER: &str = "PERENE_M1_MARK_42";
const ALIVE: &str = "PERENE_ALIVE_99";

fn b64_decode(s: &str) -> Vec<u8> {
    base64::engine::general_purpose::STANDARD
        .decode(s.as_bytes())
        .unwrap_or_default()
}

/// Cliente de linha com timeout, para os testes não travarem.
struct LineClient {
    stream: UnixStream,
    buf: Vec<u8>,
}

impl LineClient {
    fn connect(path: &Path) -> std::io::Result<Self> {
        let stream = UnixStream::connect(path)?;
        stream.set_read_timeout(Some(Duration::from_millis(150)))?;
        Ok(Self {
            stream,
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
            if Instant::now() >= deadline {
                return None;
            }
            let mut tmp = [0u8; 8192];
            match self.stream.read(&mut tmp) {
                Ok(0) => return None,
                Ok(n) => self.buf.extend_from_slice(&tmp[..n]),
                Err(ref e)
                    if e.kind() == std::io::ErrorKind::WouldBlock
                        || e.kind() == std::io::ErrorKind::TimedOut =>
                {
                    continue;
                }
                Err(_) => return None,
            }
        }
    }

    /// Lê mensagens até que os bytes de Output/Scrollback acumulados contenham
    /// `needle`. Retorna true se achou dentro do prazo.
    fn wait_for_output(&mut self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut acc: Vec<u8> = Vec::new();
        while Instant::now() < deadline {
            match self.next_msg(deadline) {
                Some(DaemonMessage::Output(o)) | Some(DaemonMessage::Scrollback(o)) => {
                    acc.extend_from_slice(&b64_decode(&o.data_b64));
                    if String::from_utf8_lossy(&acc).contains(needle) {
                        return true;
                    }
                }
                Some(_) => {}
                None => {}
            }
        }
        String::from_utf8_lossy(&acc).contains(needle)
    }

    /// Igual, mas só considera mensagens de Scrollback (replay do reattach).
    fn wait_for_scrollback(&mut self, needle: &str, timeout: Duration) -> bool {
        let deadline = Instant::now() + timeout;
        let mut acc: Vec<u8> = Vec::new();
        while Instant::now() < deadline {
            match self.next_msg(deadline) {
                Some(DaemonMessage::Scrollback(o)) => {
                    acc.extend_from_slice(&b64_decode(&o.data_b64));
                    if String::from_utf8_lossy(&acc).contains(needle) {
                        return true;
                    }
                }
                Some(DaemonMessage::AttachDone { .. }) => {
                    // Replay terminou; decide com o que já veio.
                    return String::from_utf8_lossy(&acc).contains(needle);
                }
                Some(_) => {}
                None => {}
            }
        }
        String::from_utf8_lossy(&acc).contains(needle)
    }
}

fn wait_for_socket(path: &Path, timeout: Duration) {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if UnixStream::connect(path).is_ok() {
            return;
        }
        std::thread::sleep(Duration::from_millis(20));
    }
    panic!("socket do daemon não apareceu em {}", path.display());
}

#[test]
fn spawn_attach_reattach_preserves_scrollback_and_process() {
    // Shell hermético (evita sourcing pesado do .zshrc do usuário no teste).
    std::env::set_var("SHELL", "/bin/sh");

    let dir = tempfile::tempdir().unwrap();
    let socket = dir.path().join("daemon.sock");
    let config = perene_daemon::Config {
        socket_path: socket.clone(),
        lock_path: dir.path().join("daemon.lock"),
        scrollback_dir: dir.path().join("scrollback"),
        scrollback_cap: 1024 * 1024,
    };

    // Sobe o daemon numa thread (morre com o processo de teste no fim).
    std::thread::spawn(move || {
        let _ = perene_daemon::run(config);
    });
    wait_for_socket(&socket, Duration::from_secs(5));

    // ── Cliente 1: cria o pane e vê o marcador ────────────────────────────
    let mut c1 = LineClient::connect(&socket).unwrap();
    c1.send(&ClientMessage::Hello {
        protocol_version: perene_daemon::PROTOCOL_VERSION,
    });
    assert!(
        matches!(
            c1.next_msg(Instant::now() + Duration::from_secs(2)),
            Some(DaemonMessage::Welcome { .. })
        ),
        "esperava Welcome"
    );

    let pane_id = "pane_test".to_string();
    c1.send(&ClientMessage::Spawn(SpawnRequest {
        pane_id: pane_id.clone(),
        cols: 80,
        rows: 24,
        cwd: Some(dir.path().to_string_lossy().to_string()),
        command: Some(format!("printf '{MARKER}\\n'")),
        shell: None,
    }));
    c1.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });
    assert!(
        c1.wait_for_output(MARKER, Duration::from_secs(10)),
        "cliente 1 devia ver o marcador do comando inicial"
    );

    // ── "Fecha a janela": desconecta o cliente 1. O PTY segue vivo. ────────
    drop(c1);
    std::thread::sleep(Duration::from_millis(200));

    // ── "Reabre a janela": cliente 2 atacha e recebe o scrollback. ────────
    let mut c2 = LineClient::connect(&socket).unwrap();
    c2.send(&ClientMessage::Hello {
        protocol_version: perene_daemon::PROTOCOL_VERSION,
    });
    let _ = c2.next_msg(Instant::now() + Duration::from_secs(2)); // Welcome
    c2.send(&ClientMessage::Attach {
        pane_id: pane_id.clone(),
    });
    assert!(
        c2.wait_for_scrollback(MARKER, Duration::from_secs(5)),
        "reattach devia reproduzir o scrollback com o marcador"
    );

    // ── Mesmo processo vivo: mandamos um comando e vemos a resposta. ───────
    c2.send(&ClientMessage::Write {
        pane_id: pane_id.clone(),
        data_b64: base64::engine::general_purpose::STANDARD.encode(format!("echo {ALIVE}\n")),
    });
    assert!(
        c2.wait_for_output(ALIVE, Duration::from_secs(10)),
        "o shell original devia continuar vivo e responder após o reattach"
    );

    // Lista de panes reporta o pane vivo.
    c2.send(&ClientMessage::ListPanes);
    let mut saw_pane = false;
    let deadline = Instant::now() + Duration::from_secs(2);
    while Instant::now() < deadline {
        if let Some(DaemonMessage::Panes { panes }) = c2.next_msg(deadline) {
            saw_pane = panes.iter().any(|p| p.pane_id == pane_id && p.alive);
            break;
        }
    }
    assert!(saw_pane, "ListPanes devia reportar o pane vivo");
}

#[test]
fn single_instance_lock_blocks_second_daemon() {
    let dir = tempfile::tempdir().unwrap();
    let lock = dir.path().join("daemon.lock");

    let g1 = perene_daemon::acquire_single_instance(&lock).expect("primeiro lock deve pegar");
    let g2 = perene_daemon::acquire_single_instance(&lock);
    assert!(g2.is_err(), "segundo daemon NUNCA pode adquirir o lock (lição #2)");

    drop(g1);
    let g3 = perene_daemon::acquire_single_instance(&lock);
    assert!(g3.is_ok(), "liberado o lock, um novo daemon pode assumir");
}
