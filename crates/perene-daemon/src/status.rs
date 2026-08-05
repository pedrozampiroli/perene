//! Detecção do que a sessão está fazendo, a partir do stream do PTY.
//!
//! Não existe canal estruturado: as CLIs de IA rodam como TUI interativa, então
//! o que temos é o que elas desenham na tela. A heurística é deliberadamente
//! conservadora — **na dúvida, silêncio**, porque um indicador errado incomoda
//! mais do que indicador nenhum:
//!
//! - saída chegando agora           → `Running`
//! - parou e a tela pede confirmação → `Waiting`
//! - parou e a tela mostra erro      → `Error`
//! - parou depois de ter rodado      → `Done` (some sozinho)
//!
//! O "parou" é medido por silêncio no PTY ([`QUIET`]), o que funciona igual para
//! claude/codex/opencode sem depender de detalhe de layout de nenhum deles.

use std::time::{Duration, Instant};

use perene_protocol::PaneState;

/// Silêncio a partir do qual consideramos que a execução parou.
pub const QUIET: Duration = Duration::from_millis(1200);
/// Por quanto tempo o "terminou" fica visível antes de sumir.
pub const DONE_TTL: Duration = Duration::from_secs(25);
/// Quanto da tela recente guardamos para casar padrões.
const TAIL_CAP: usize = 3000;

/// Estado + janela recente de saída de um pane.
pub struct StatusDetector {
    state: PaneState,
    /// Últimos bytes imprimíveis (sem ANSI), em minúsculas.
    tail: String,
    last_output: Instant,
    since: Instant,
    /// Houve saída desde o último período de silêncio (i.e., algo rodou).
    ran: bool,
}

impl Default for StatusDetector {
    fn default() -> Self {
        Self::new()
    }
}

impl StatusDetector {
    pub fn new() -> Self {
        let now = Instant::now();
        Self {
            state: PaneState::Idle,
            tail: String::new(),
            last_output: now,
            since: now,
            ran: false,
        }
    }

    pub fn state(&self) -> PaneState {
        self.state
    }

    /// Alimenta o detector com um chunk do PTY. Devolve o novo estado se mudou.
    pub fn on_output(&mut self, bytes: &[u8]) -> Option<PaneState> {
        push_visible(&mut self.tail, bytes);
        if self.tail.len() > TAIL_CAP {
            let cut = self.tail.len() - TAIL_CAP;
            // Corta em fronteira de char para não quebrar UTF-8.
            let idx = self
                .tail
                .char_indices()
                .find(|(i, _)| *i >= cut)
                .map(|(i, _)| i)
                .unwrap_or(self.tail.len());
            self.tail.drain(..idx);
        }
        self.last_output = Instant::now();
        self.ran = true;
        self.set(PaneState::Running)
    }

    /// Chamado periodicamente pelo daemon. Devolve o novo estado se mudou.
    pub fn tick(&mut self) -> Option<PaneState> {
        let quiet_for = self.last_output.elapsed();
        match self.state {
            // Rodando e ficou quieto → classifica pelo que está na tela.
            PaneState::Running if quiet_for >= QUIET => {
                let next = if asks_for_approval(&self.tail) {
                    PaneState::Waiting
                } else if shows_error(&self.tail) {
                    PaneState::Error
                } else if self.ran {
                    PaneState::Done
                } else {
                    PaneState::Idle
                };
                self.ran = false;
                self.set(next)
            }
            // "Terminou" é transitório: some sozinho.
            PaneState::Done if self.since.elapsed() >= DONE_TTL => self.set(PaneState::Idle),
            _ => None,
        }
    }

    fn set(&mut self, next: PaneState) -> Option<PaneState> {
        if next == self.state {
            return None;
        }
        self.state = next;
        self.since = Instant::now();
        Some(next)
    }
}

/// Acrescenta ao buffer só o texto visível: descarta sequências ANSI (cores,
/// movimentação de cursor) para os padrões casarem com o que o usuário lê.
fn push_visible(out: &mut String, bytes: &[u8]) {
    let text = String::from_utf8_lossy(bytes);
    let mut chars = text.chars().peekable();
    while let Some(c) = chars.next() {
        match c {
            '\x1b' => {
                // CSI (ESC [ … letra) e OSC (ESC ] … BEL/ST) são os casos comuns.
                match chars.peek() {
                    Some('[') => {
                        chars.next();
                        for c in chars.by_ref() {
                            if c.is_ascii_alphabetic() {
                                break;
                            }
                        }
                    }
                    Some(']') => {
                        chars.next();
                        for c in chars.by_ref() {
                            if c == '\x07' || c == '\x1b' {
                                break;
                            }
                        }
                    }
                    _ => {
                        chars.next();
                    }
                }
            }
            // Mantém quebras de linha (ajudam a delimitar prompts) e imprimíveis.
            '\n' => out.push('\n'),
            c if c == ' ' || !c.is_control() => out.extend(c.to_lowercase()),
            _ => {}
        }
    }
}

/// Só as últimas linhas interessam: é o que está "na tela" agora.
fn last_lines(tail: &str, n: usize) -> String {
    let lines: Vec<&str> = tail.lines().rev().take(n).collect();
    lines.join("\n")
}

/// A CLI está esperando o usuário aprovar/escolher algo?
fn asks_for_approval(tail: &str) -> bool {
    let t = last_lines(tail, 14);
    const PATTERNS: &[&str] = &[
        // Claude Code
        "do you want to",
        "would you like to",
        "1. yes",
        "❯ 1.",
        // Codex
        "allow command",
        "approve this",
        "approval required",
        // OpenCode / genéricos
        "permission to",
        "[y/n]",
        "(y/n)",
        "press enter to continue",
    ];
    PATTERNS.iter().any(|p| t.contains(p))
}

/// A última execução acabou em erro?
fn shows_error(tail: &str) -> bool {
    let t = last_lines(tail, 10);
    const PATTERNS: &[&str] = &[
        "error:",
        "fatal:",
        "traceback (most recent call last)",
        "command not found",
        "permission denied",
        "panicked at",
        "exception:",
    ];
    PATTERNS.iter().any(|p| t.contains(p))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn feed(d: &mut StatusDetector, s: &str) {
        d.on_output(s.as_bytes());
    }

    #[test]
    fn output_marks_running_and_silence_marks_done() {
        let mut d = StatusDetector::new();
        assert_eq!(d.state(), PaneState::Idle);
        assert_eq!(feed_state(&mut d, "trabalhando…"), Some(PaneState::Running));
        // Antes do silêncio mínimo, nada muda.
        assert_eq!(d.tick(), None);
        d.last_output -= QUIET;
        assert_eq!(d.tick(), Some(PaneState::Done));
        // E o "terminou" some sozinho depois do TTL.
        d.since -= DONE_TTL;
        assert_eq!(d.tick(), Some(PaneState::Idle));
    }

    #[test]
    fn detects_approval_prompt() {
        let mut d = StatusDetector::new();
        feed(&mut d, "Do you want to proceed?\n  1. Yes\n  2. No");
        d.last_output -= QUIET;
        assert_eq!(d.tick(), Some(PaneState::Waiting));
    }

    #[test]
    fn detects_error() {
        let mut d = StatusDetector::new();
        feed(&mut d, "npm ERR! error: build failed\n");
        d.last_output -= QUIET;
        assert_eq!(d.tick(), Some(PaneState::Error));
    }

    #[test]
    fn ansi_sequences_do_not_break_matching() {
        let mut d = StatusDetector::new();
        // Mesma pergunta, mas colorida e com movimentação de cursor.
        feed(&mut d, "\x1b[2K\x1b[1;33mDo you want to\x1b[0m proceed?\x1b[?25l");
        d.last_output -= QUIET;
        assert_eq!(d.tick(), Some(PaneState::Waiting));
    }

    #[test]
    fn plain_output_is_not_mistaken_for_error() {
        let mut d = StatusDetector::new();
        feed(&mut d, "compilando 12 arquivos... tudo certo\n");
        d.last_output -= QUIET;
        assert_eq!(d.tick(), Some(PaneState::Done));
    }

    fn feed_state(d: &mut StatusDetector, s: &str) -> Option<PaneState> {
        d.on_output(s.as_bytes())
    }
}
