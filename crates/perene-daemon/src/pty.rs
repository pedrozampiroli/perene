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

/// Remove variáveis de *sessão de harness* herdadas — ver
/// [`perene_core::harness_env`] para o porquê. A lista é compartilhada com o
/// modo ACP de propósito: os dois caminhos não podem divergir.
fn sanitize_env(cmd: &mut CommandBuilder) {
    for key in perene_core::harness_env::inherited_session_vars() {
        cmd.env_remove(&key);
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
