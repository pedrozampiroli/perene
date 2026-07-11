//! Montagem do comando do login shell. Igual ao M0, agora morando no daemon
//! (dono único dos PTYs). Login shell é obrigatório (lição #6): senão as CLIs
//! (claude/codex/opencode) não estão no PATH.

use portable_pty::CommandBuilder;
use perene_protocol::SpawnRequest;

/// Monta o `CommandBuilder` com PATH/aliases carregados e TERM/cwd corretos.
pub fn build_command(req: &SpawnRequest) -> CommandBuilder {
    let mut cmd = platform_shell(req.command.as_deref());
    let cwd = req
        .cwd
        .clone()
        .or_else(home_dir)
        .unwrap_or_else(|| ".".to_string());
    cmd.cwd(cwd);
    cmd.env("TERM", "xterm-256color");
    cmd.env("COLORTERM", "truecolor");
    cmd.env("PERENE", "2");
    cmd
}

#[cfg(not(windows))]
fn platform_shell(command: Option<&str>) -> CommandBuilder {
    let shell = std::env::var("SHELL").unwrap_or_else(|_| "/bin/zsh".to_string());
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
fn platform_shell(command: Option<&str>) -> CommandBuilder {
    let mut cmd = CommandBuilder::new("powershell.exe");
    cmd.arg("-NoLogo");
    if let Some(c) = command {
        cmd.arg("-NoExit");
        cmd.arg("-Command");
        cmd.arg(c);
    }
    cmd
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}
