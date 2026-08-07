//! Montagem do comando do login shell. Igual ao M0, agora morando no daemon
//! (dono único dos PTYs). Login shell é obrigatório (lição #6): senão as CLIs
//! (claude/codex/opencode) não estão no PATH.

use portable_pty::CommandBuilder;
use perene_protocol::SpawnRequest;

/// Monta o `CommandBuilder` com PATH/aliases carregados e TERM/cwd corretos.
pub fn build_command(req: &SpawnRequest) -> CommandBuilder {
    let shell_override = req.shell.as_deref().filter(|s| !s.is_empty());
    let mut cmd = platform_shell(shell_override, req.command.as_deref());
    let cwd = req
        .cwd
        .clone()
        .or_else(home_dir)
        .unwrap_or_else(|| ".".to_string());
    cmd.cwd(cwd);
    sanitize_env(&mut cmd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("PERENE", "2");
    cmd
}

/// Remove variáveis de ambiente de *sessão de harness* herdadas.
///
/// Se o Perene for aberto de dentro de uma sessão do Claude Code (ou de outro
/// harness), o processo herda marcadores como `CLAUDE_CODE_CHILD_SESSION` e
/// `CLAUDE_CODE_SESSION_ID`. Herdados pelo `claude` que abrimos no PTY, eles
/// **desligam o salvamento do transcript** ("Transcript saving is off") e a
/// conversa nunca é gravada em disco — então o `--resume` depois falha com
/// "No conversation found with session ID". Os terminais precisam nascer limpos.
fn sanitize_env(cmd: &mut CommandBuilder) {
    const EXACT: &[&str] = &[
        "CLAUDECODE",
        "CLAUDE_PID",
        "CLAUDE_EFFORT",
        "CODEX_SANDBOX",
        "CODEX_SESSION_ID",
        "OPENCODE_SESSION_ID",
    ];
    for (key, _) in std::env::vars() {
        let up = key.to_ascii_uppercase();
        if up.starts_with("CLAUDE_CODE_") || EXACT.contains(&up.as_str()) {
            cmd.env_remove(&key);
        }
    }
    sanitize_appimage_env(cmd);
}

/// Remove a poluição de ambiente que o `AppRun` do AppImage injeta.
///
/// O bundle Linux roda sob um `AppRun` que exporta `PYTHONHOME=$APPDIR/usr/`,
/// `PERLLIB`, `QT_PLUGIN_PATH` e prefixos de `LD_LIBRARY_PATH`/`XDG_DATA_DIRS`
/// apontando pra dentro do bundle. O processo do app **precisa** disso (o WebKit
/// spawna `WebKitWebProcess`/`WebKitNetworkProcess`, que acham as libs
/// empacotadas por ali) — mas o login shell não: com `PYTHONHOME` herdado,
/// qualquer python do sistema morre com "Fatal Python error: failed to import
/// encodings module". Por isso a limpeza é por filho, nunca global.
///
/// Fora de AppImage (`APPDIR` ausente) é no-op.
#[cfg(target_os = "linux")]
fn sanitize_appimage_env(cmd: &mut CommandBuilder) {
    let appdir = match std::env::var("APPDIR") {
        Ok(dir) if !dir.is_empty() => dir,
        _ => return,
    };

    // Variáveis que o AppRun/hook do linuxdeploy criam do zero: some com elas.
    const DROP: &[&str] = &[
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
    for key in DROP {
        cmd.env_remove(key);
    }

    // Listas de caminhos: o AppRun prefixou as entradas do bundle e preservou o
    // valor original no fim. Tira só o que aponta pro $APPDIR — o resto é do
    // usuário e deve sobreviver.
    const PATH_LISTS: &[&str] = &[
        "GSETTINGS_SCHEMA_DIR",
        "LD_LIBRARY_PATH",
        "PERLLIB",
        "PYTHONPATH",
        "QT_PLUGIN_PATH",
        "XDG_DATA_DIRS",
    ];
    for key in PATH_LISTS {
        let Ok(value) = std::env::var(key) else { continue };
        match strip_appdir_entries(&appdir, &value) {
            Some(kept) => cmd.env(key, kept),
            None => cmd.env_remove(key),
        }
    }
}

#[cfg(not(target_os = "linux"))]
fn sanitize_appimage_env(_cmd: &mut CommandBuilder) {}

/// Filtra de uma lista `a:b:c` as entradas que apontam pra dentro do `appdir`.
/// Devolve `None` quando não sobra nada (a variável só existia por causa do bundle).
#[cfg(target_os = "linux")]
fn strip_appdir_entries(appdir: &str, value: &str) -> Option<String> {
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

#[cfg(not(windows))]
fn platform_shell(shell_override: Option<&str>, command: Option<&str>) -> CommandBuilder {
    // Shell escolhido nas configurações, ou `$SHELL`, ou zsh.
    let shell = shell_override
        .map(String::from)
        .unwrap_or_else(|| std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string()));
    let mut cmd = CommandBuilder::new(&shell);
    match command {
        // Login shell puro; o PTY já o deixa interativo → carrega o profile.
        None => {
            cmd.arg("-l");
        }
        // Roda o comando e cai de volta no shell para o pane não fechar.
        Some(c) => {
            cmd.arg("-l");
            cmd.arg("-c");
            cmd.arg(format!("{c}; exec {shell} -l"));
        }
    }
    cmd
}

#[cfg(windows)]
fn platform_shell(shell_override: Option<&str>, command: Option<&str>) -> CommandBuilder {
    let prog = shell_override.unwrap_or("powershell.exe").to_string();
    let mut cmd = CommandBuilder::new(&prog);
    let lower = prog.to_lowercase();
    if lower.ends_with("powershell.exe") || lower.ends_with("pwsh.exe") {
        cmd.arg("-NoLogo");
        if let Some(c) = command {
            cmd.arg("-NoExit");
            cmd.arg("-Command");
            cmd.arg(c);
        }
    } else if lower.ends_with("bash.exe") {
        // Git Bash / WSL bash: login interativo.
        cmd.arg("-l");
        cmd.arg("-i");
        if let Some(c) = command {
            cmd.arg("-c");
            cmd.arg(format!("{c}; exec \"{prog}\" -l -i"));
        }
    } else if lower.ends_with("cmd.exe") {
        // /K roda o comando e SEGURA o prompt — sem isso o pane fecharia sozinho
        // ao fim do `claude`/`codex`.
        if let Some(c) = command {
            cmd.arg("/K");
            cmd.arg(c);
        }
    }
    // Outros (wsl.exe etc.): sem args extras — abrem o shell padrão. Um `command`
    // de perfil não é injetável aí sem saber a sintaxe do shell de destino.
    cmd
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

#[cfg(all(test, target_os = "linux"))]
mod tests {
    use super::strip_appdir_entries;

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
}
