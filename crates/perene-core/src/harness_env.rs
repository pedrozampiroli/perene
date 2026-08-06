//! Limpeza das variáveis de sessão de harness herdadas.
//!
//! Se o Perene for aberto de dentro de uma sessão do Claude Code (ou de outro
//! harness de IA), o processo herda marcadores como `CLAUDE_CODE_CHILD_SESSION`,
//! `CLAUDECODE` e `AI_AGENT`. Passados adiante para uma CLI de IA que *nós*
//! subimos, eles fazem a ferramenta se achar aninhada:
//!
//!  - no PTY, o `claude` desliga o salvamento do transcript ("Transcript saving
//!    is off") e o `--resume` depois falha com "No conversation found";
//!  - no modo ACP, o adapter recusa `session/new` com erro interno.
//!
//! Toda sessão que o Perene abre precisa nascer limpa. Isto vive aqui — e não
//! junto de quem spawna — porque são dois caminhos diferentes (PTY e ACP) que
//! não podem divergir.

/// Nomes de variáveis do ambiente ATUAL que devem ser removidas ao spawnar uma
/// ferramenta de IA. Devolve nomes existentes, com a grafia original (Windows é
/// case-insensitive, mas `env_remove` compara literal).
pub fn inherited_session_vars() -> Vec<String> {
    std::env::vars()
        .map(|(key, _)| key)
        .filter(|key| is_session_var(key))
        .collect()
}

/// `true` para variáveis que marcam "você está dentro de uma sessão de harness".
pub fn is_session_var(key: &str) -> bool {
    const EXACT: &[&str] = &[
        "CLAUDECODE",
        "CLAUDE_PID",
        "CLAUDE_EFFORT",
        "CODEX_SANDBOX",
        "CODEX_SESSION_ID",
        "OPENCODE_SESSION_ID",
        // O adapter ACP se identifica por aqui; herdado, ele se acha aninhado.
        "AI_AGENT",
    ];
    let up = key.to_ascii_uppercase();
    up.starts_with("CLAUDE_CODE_") || EXACT.contains(&up.as_str())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn recognizes_harness_markers_and_leaves_the_rest_alone() {
        assert!(is_session_var("CLAUDECODE"));
        assert!(is_session_var("CLAUDE_CODE_CHILD_SESSION"));
        assert!(is_session_var("CLAUDE_CODE_SESSION_ID"));
        assert!(is_session_var("AI_AGENT"));
        assert!(is_session_var("codex_sandbox"), "compara sem case");

        // Nada de arrastar o ambiente do usuário junto.
        assert!(!is_session_var("PATH"));
        assert!(!is_session_var("HOME"));
        assert!(!is_session_var("SHELL"));
        assert!(!is_session_var("CLAUDE_CONFIG_DIR"), "config não é sessão");
    }
}
