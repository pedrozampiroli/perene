//! Montagem do comando do login shell. Igual ao M0, agora morando no daemon
//! (dono único dos PTYs). Login shell é obrigatório (lição #6): senão as CLIs
//! (claude/codex/opencode) não estão no PATH.

use perene_protocol::SpawnRequest;
use portable_pty::CommandBuilder;

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
            // `-c` torna o shell não interativo. Sem `-i`, zsh/bash não leem o
            // rc onde nvm/fnm costumam pôr as CLIs no PATH (app aberto pelo Finder).
            cmd.arg("-i");
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

#[cfg(all(test, target_os = "macos"))]
mod tests {
    use super::platform_shell;
    use portable_pty::{native_pty_system, CommandBuilder, PtySize};
    use std::fs;
    use std::io::Read;
    use std::os::unix::fs::PermissionsExt;
    use std::sync::mpsc;
    use std::time::Duration;

    #[test]
    fn command_shell_loads_cli_path_from_zshrc_with_finder_environment() {
        let home = tempfile::tempdir().unwrap();
        let bin = home.path().join("bin");
        fs::create_dir(&bin).unwrap();

        let codex = bin.join("codex");
        fs::write(&codex, "#!/bin/sh\nprintf 'CODEX_FROM_ZSHRC\\n'\n").unwrap();
        let mut permissions = fs::metadata(&codex).unwrap().permissions();
        permissions.set_mode(0o755);
        fs::set_permissions(&codex, permissions).unwrap();

        fs::write(
            home.path().join(".zshrc"),
            "export PATH=\"$HOME/bin:$PATH\"\n",
        )
        .unwrap();

        let mut command: CommandBuilder = platform_shell(Some("/bin/zsh"), Some("codex; exit"));
        command.env_clear();
        command.env("HOME", home.path());
        command.env("PATH", "/usr/bin:/bin:/usr/sbin:/sbin");
        command.env("SHELL", "/bin/zsh");

        let pair = native_pty_system()
            .openpty(PtySize {
                rows: 24,
                cols: 80,
                pixel_width: 0,
                pixel_height: 0,
            })
            .unwrap();
        let mut reader = pair.master.try_clone_reader().unwrap();
        let mut child = pair.slave.spawn_command(command).unwrap();
        drop(pair.slave);

        let (tx, rx) = mpsc::channel();
        let reader_thread = std::thread::spawn(move || {
            let mut bytes = Vec::new();
            let mut chunk = [0_u8; 256];
            loop {
                match reader.read(&mut chunk) {
                    Ok(0) | Err(_) => break,
                    Ok(read) => {
                        bytes.extend_from_slice(&chunk[..read]);
                        if bytes
                            .windows(b"CODEX_FROM_ZSHRC".len())
                            .any(|window| window == b"CODEX_FROM_ZSHRC")
                        {
                            break;
                        }
                    }
                }
            }
            let _ = tx.send(String::from_utf8_lossy(&bytes).into_owned());
        });

        let observed = rx.recv_timeout(Duration::from_secs(3));
        let _ = child.kill();
        let _ = child.wait();
        let output = observed.unwrap_or_else(|_| {
            rx.recv_timeout(Duration::from_secs(1))
                .unwrap_or_else(|_| "timeout esperando saída do PTY".to_string())
        });
        reader_thread.join().unwrap();

        assert!(
            output.contains("CODEX_FROM_ZSHRC"),
            "o shell não carregou o PATH configurado no .zshrc: {output:?}"
        );
    }
}
