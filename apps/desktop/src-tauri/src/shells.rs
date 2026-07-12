//! Descoberta de shells disponíveis para o usuário escolher nas configurações.
//! Unix: `/etc/shells` + caminhos comuns. Windows: PowerShell/cmd/WSL/Git Bash.

use serde::{Deserialize, Serialize};

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShellOption {
    /// Caminho do programa (o que vai pra `Settings.shell`).
    pub path: String,
    /// Rótulo amigável (ex.: "zsh", "WSL", "Git Bash").
    pub label: String,
}

#[cfg(not(windows))]
#[tauri::command]
pub fn list_shells() -> Vec<ShellOption> {
    use std::collections::BTreeSet;

    let mut paths: BTreeSet<String> = BTreeSet::new();

    // /etc/shells (lista oficial de login shells).
    if let Ok(content) = std::fs::read_to_string("/etc/shells") {
        for line in content.lines() {
            let l = line.trim();
            if l.starts_with('/') && std::path::Path::new(l).exists() {
                paths.insert(l.to_string());
            }
        }
    }
    // Candidatos comuns (Homebrew etc.).
    for c in [
        "/bin/zsh",
        "/bin/bash",
        "/bin/sh",
        "/opt/homebrew/bin/zsh",
        "/opt/homebrew/bin/bash",
        "/opt/homebrew/bin/fish",
        "/usr/local/bin/bash",
        "/usr/local/bin/fish",
        "/usr/bin/fish",
    ] {
        if std::path::Path::new(c).exists() {
            paths.insert(c.to_string());
        }
    }

    paths
        .into_iter()
        .map(|p| ShellOption {
            label: label_for(&p),
            path: p,
        })
        .collect()
}

#[cfg(windows)]
#[tauri::command]
pub fn list_shells() -> Vec<ShellOption> {
    let mut out = Vec::new();
    let mut add = |path: &str, label: &str| {
        if std::path::Path::new(path).exists() {
            out.push(ShellOption {
                path: path.to_string(),
                label: label.to_string(),
            });
        }
    };
    // PowerShell (moderno e clássico), cmd.
    add("C:\\Program Files\\PowerShell\\7\\pwsh.exe", "PowerShell 7");
    add(
        "C:\\Windows\\System32\\WindowsPowerShell\\v1.0\\powershell.exe",
        "Windows PowerShell",
    );
    add("C:\\Windows\\System32\\cmd.exe", "Prompt de comando");
    // WSL.
    add("C:\\Windows\\System32\\wsl.exe", "WSL");
    // Git Bash.
    add("C:\\Program Files\\Git\\bin\\bash.exe", "Git Bash");
    add("C:\\Program Files (x86)\\Git\\bin\\bash.exe", "Git Bash");
    out
}

#[cfg(not(windows))]
fn label_for(path: &str) -> String {
    path.rsplit('/').next().unwrap_or(path).to_string()
}
