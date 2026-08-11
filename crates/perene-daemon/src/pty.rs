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

/// Aplica as correções de ambiente do [`perene_core::harness_env`]: variáveis
/// de sessão de harness herdadas e a poluição do AppImage. A lista é
/// compartilhada com o modo ACP de propósito — os caminhos não podem divergir.
fn sanitize_env(cmd: &mut CommandBuilder) {
    use perene_core::harness_env::EnvFix;
    for fix in perene_core::harness_env::child_env_fixes() {
        match fix {
            EnvFix::Remove(key) => cmd.env_remove(&key),
            EnvFix::Set(key, value) => cmd.env(&key, &value),
        }
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
