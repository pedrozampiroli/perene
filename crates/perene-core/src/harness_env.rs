//! Correções de ambiente para todo processo que o Perene spawna.
//!
//! São duas limpezas, de origens diferentes, e **as duas valem para todos os
//! caminhos** — PTY, adapter ACP e comandos que o agente pede que rodemos:
//!
//! 1. **Sessão de harness herdada.** Se o Perene for aberto de dentro de uma
//!    sessão do Claude Code ou Codex, o processo herda marcadores como
//!    `CLAUDE_CODE_CHILD_SESSION`, `CLAUDECODE`, `CODEX_THREAD_ID`, `AI_AGENT`…
//!    Passados adiante, a ferramenta se acha aninhada:
//!    no PTY o `claude` desliga o transcript (e o `--resume` falha depois com
//!    "No conversation found"); no ACP o adapter recusa `session/new`. Algumas
//!    preferências do harness também não representam o usuário: o Codex exporta
//!    `NO_COLOR=1`, que deixaria monocromáticos todos os agentes abertos no app.
//! 2. **Poluição do AppImage.** No Linux o app roda sob um `AppRun` que exporta
//!    `PYTHONHOME=$APPDIR/usr/`, `PERLLIB`, `QT_PLUGIN_PATH` e prefixos de
//!    `LD_LIBRARY_PATH`. O processo do app **precisa** disso (o WebKit spawna
//!    processos que acham as libs empacotadas por ali), mas os filhos não: com
//!    `PYTHONHOME` herdado, qualquer python do sistema morre com "Fatal Python
//!    error: failed to import encodings module".
//!
//! Este módulo decide **o quê** mudar; quem spawna aplica no seu tipo de comando
//! (`portable_pty::CommandBuilder` no PTY, `std::process::Command` no ACP). Foi
//! por divergir entre caminhos que os dois bugs acima apareceram.

/// Uma mudança a aplicar no ambiente do processo filho.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EnvFix {
    Remove(String),
    Set(String, String),
}

/// Tudo que precisa ser corrigido no ambiente de um filho, na ordem.
///
/// Chame ao spawnar qualquer coisa: shell do PTY, adapter ACP ou comando pedido
/// pelo agente.
pub fn child_env_fixes() -> Vec<EnvFix> {
    let mut fixes: Vec<EnvFix> = inherited_session_vars()
        .into_iter()
        .map(EnvFix::Remove)
        .collect();
    fixes.extend(appimage_fixes());
    fixes
}

/// Variáveis do ambiente ATUAL que não devem chegar aos filhos quando o Perene
/// foi aberto por um harness. Devolve a grafia original (Windows é
/// case-insensitive, mas `env_remove` compara literal).
pub fn inherited_session_vars() -> Vec<String> {
    inherited_session_vars_from(std::env::vars().map(|(key, _)| key))
}

fn inherited_session_vars_from(keys: impl IntoIterator<Item = String>) -> Vec<String> {
    let keys: Vec<String> = keys.into_iter().collect();
    let launched_by_harness = keys.iter().any(|key| is_session_var(key));

    keys.into_iter()
        .filter(|key| {
            is_session_var(key) || (launched_by_harness && key.eq_ignore_ascii_case("NO_COLOR"))
        })
        .collect()
}

/// `true` para variáveis que marcam "você está dentro de uma sessão de harness".
pub fn is_session_var(key: &str) -> bool {
    const EXACT: &[&str] = &[
        "CLAUDECODE",
        "CLAUDE_PID",
        "CLAUDE_EFFORT",
        "CODEX_CI",
        "CODEX_SANDBOX",
        "CODEX_SESSION_ID",
        "CODEX_THREAD_ID",
        "OPENCODE_SESSION_ID",
        // O adapter ACP se identifica por aqui; herdado, ele se acha aninhado.
        "AI_AGENT",
    ];
    let up = key.to_ascii_uppercase();
    up.starts_with("CLAUDE_CODE_") || EXACT.contains(&up.as_str())
}

/// Variáveis que o `AppRun`/hook do linuxdeploy cria do zero: some com elas.
#[cfg(target_os = "linux")]
const APPIMAGE_DROP: &[&str] = &[
    "APPDIR",
    "APPIMAGE",
    "ARGV0",
    "OWD",
    "PYTHONHOME",
    "GDK_BACKEND",
    "GDK_PIXBUF_MODULE_FILE",
    "GIO_EXTRA_MODULES",
    "GTK_DATA_PREFIX",
    "GTK_EXE_PREFIX",
    "GTK_IM_MODULE_FILE",
    "GTK_PATH",
    "GTK_THEME",
];

/// Listas de caminhos que o `AppRun` prefixou com entradas do bundle,
/// preservando o valor original no fim. Tiramos só o que aponta pro `$APPDIR`.
#[cfg(target_os = "linux")]
const APPIMAGE_PATH_LISTS: &[&str] = &[
    "GSETTINGS_SCHEMA_DIR",
    "LD_LIBRARY_PATH",
    "PERLLIB",
    "PYTHONPATH",
    "QT_PLUGIN_PATH",
    "XDG_DATA_DIRS",
];

/// Correções do AppImage. Fora dele (`APPDIR` ausente) devolve vazio.
#[cfg(target_os = "linux")]
pub fn appimage_fixes() -> Vec<EnvFix> {
    let appdir = match std::env::var("APPDIR") {
        Ok(dir) if !dir.is_empty() => dir,
        _ => return Vec::new(),
    };
    let mut fixes: Vec<EnvFix> = APPIMAGE_DROP
        .iter()
        .map(|k| EnvFix::Remove((*k).to_string()))
        .collect();
    for key in APPIMAGE_PATH_LISTS {
        let Ok(value) = std::env::var(key) else {
            continue;
        };
        fixes.push(match strip_appdir_entries(&appdir, &value) {
            Some(kept) => EnvFix::Set((*key).to_string(), kept),
            None => EnvFix::Remove((*key).to_string()),
        });
    }
    fixes
}

#[cfg(not(target_os = "linux"))]
pub fn appimage_fixes() -> Vec<EnvFix> {
    Vec::new()
}

/// Filtra de uma lista `a:b:c` as entradas que apontam pra dentro do `appdir`.
/// `None` quando não sobra nada (a variável só existia por causa do bundle).
pub fn strip_appdir_entries(appdir: &str, value: &str) -> Option<String> {
    let kept: Vec<&str> = value
        .split(':')
        .filter(|entry| !entry.is_empty() && !entry.starts_with(appdir))
        .collect();
    if kept.is_empty() {
        None
    } else {
        Some(kept.join(":"))
    }
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
        assert!(is_session_var("CODEX_CI"));
        assert!(is_session_var("CODEX_THREAD_ID"));

        // Nada de arrastar o ambiente do usuário junto.
        assert!(!is_session_var("PATH"));
        assert!(!is_session_var("HOME"));
        assert!(!is_session_var("SHELL"));
        assert!(!is_session_var("CLAUDE_CONFIG_DIR"), "config não é sessão");
    }

    #[test]
    fn no_color_is_removed_only_for_harness_launches() {
        let launched_by_codex = inherited_session_vars_from(
            ["PATH", "NO_COLOR", "CODEX_THREAD_ID"]
                .into_iter()
                .map(str::to_string),
        );
        assert_eq!(
            launched_by_codex,
            vec!["NO_COLOR".to_string(), "CODEX_THREAD_ID".to_string()]
        );

        let launched_normally =
            inherited_session_vars_from(["PATH", "NO_COLOR"].into_iter().map(str::to_string));
        assert!(
            launched_normally.is_empty(),
            "NO_COLOR configurado pelo usuário deve ser preservado"
        );
    }

    // Lógica de string pura: os testes NÃO são `cfg(linux)` de propósito, para
    // que o CI do mac e do Windows também pegue regressão aqui. Do contrário só
    // o Ubuntu compilaria — e um erro passaria despercebido no dia a dia.
    const APPDIR: &str = "/tmp/.mount_PereneAbc123";

    #[test]
    fn preserva_o_valor_do_usuario_e_tira_o_do_bundle() {
        // Formato real do AppRun: entradas do bundle prefixadas, original no fim.
        let value = format!("{APPDIR}/usr/lib/:{APPDIR}/usr/lib64/:/opt/cuda/lib64");
        assert_eq!(
            strip_appdir_entries(APPDIR, &value).as_deref(),
            Some("/opt/cuda/lib64")
        );
    }

    #[test]
    fn some_quando_a_variavel_so_existia_por_causa_do_bundle() {
        let value = format!("{APPDIR}/usr/share/pyshared/:");
        assert_eq!(strip_appdir_entries(APPDIR, &value), None);
    }

    #[test]
    fn nao_mexe_em_valor_sem_appdir() {
        assert_eq!(
            strip_appdir_entries(APPDIR, "/usr/share:/usr/local/share").as_deref(),
            Some("/usr/share:/usr/local/share")
        );
    }

    #[test]
    fn fora_do_appimage_nao_mexe_em_nada_de_bundle() {
        // Sem APPDIR não há bundle: as correções são só as de harness.
        if std::env::var("APPDIR").is_ok() {
            return; // rodando dentro de um AppImage; nada a afirmar aqui
        }
        assert!(appimage_fixes().is_empty());
    }
}
