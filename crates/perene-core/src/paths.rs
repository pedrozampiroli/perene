//! Resolução dos caminhos de estado do app (`~/.perene2/` etc).
//!
//! **Lição #1 da v1:** testes JAMAIS podem escrever no estado real. Por isso todo
//! caminho passa por [`state_dir`], que respeita a env `PERENE2_STATE_DIR` — os
//! testes injetam um diretório temporário e nunca tocam o `~/.perene2` do usuário.

use std::path::PathBuf;

/// Nome da env que redireciona TODO o estado (usada por testes e dev isolado).
pub const STATE_DIR_ENV: &str = "PERENE2_STATE_DIR";

/// Diretório-raiz do estado do Perene v2.
///
/// Ordem: `PERENE2_STATE_DIR` → `~/.perene2` (unix) / `%APPDATA%\perene2` (Windows).
/// Nunca colide com `~/.perene/` da v1.
pub fn state_dir() -> PathBuf {
    if let Ok(dir) = std::env::var(STATE_DIR_ENV) {
        if !dir.is_empty() {
            return PathBuf::from(dir);
        }
    }
    platform_state_dir()
}

#[cfg(not(windows))]
fn platform_state_dir() -> PathBuf {
    let home = std::env::var("HOME").unwrap_or_else(|_| ".".to_string());
    PathBuf::from(home).join(".perene2")
}

#[cfg(windows)]
fn platform_state_dir() -> PathBuf {
    let base = std::env::var("APPDATA")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string());
    PathBuf::from(base).join("perene2")
}

/// Onde os scrollbacks são despejados no shutdown limpo do daemon.
pub fn scrollback_dir() -> PathBuf {
    state_dir().join("scrollback")
}

/// Endpoint IPC do daemon: unix socket (mac/linux) / named pipe (Windows).
#[cfg(not(windows))]
pub fn daemon_endpoint() -> PathBuf {
    state_dir().join("daemon.sock")
}

#[cfg(windows)]
pub fn daemon_endpoint() -> PathBuf {
    // Named pipes vivem num namespace próprio; derivamos do state_dir só para
    // isolar por instância em testes.
    let tag = state_dir()
        .to_string_lossy()
        .bytes()
        .fold(0u64, |h, b| h.wrapping_mul(31).wrapping_add(b as u64));
    PathBuf::from(format!(r"\\.\pipe\perene2-{tag:x}"))
}

/// Lockfile de single-instance do daemon.
pub fn daemon_lock() -> PathBuf {
    state_dir().join("daemon.lock")
}

/// Onde imagens coladas (Cmd/Ctrl+V) são salvas (M3).
pub fn paste_dir() -> PathBuf {
    state_dir().join("paste")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn state_dir_respects_env_injection() {
        // Não usamos std::env::set_var em paralelo (afeta o processo todo);
        // validamos a lógica montando o caminho esperado a partir da env atual.
        let key = STATE_DIR_ENV;
        // Salva/limpa em torno do teste.
        let prev = std::env::var(key).ok();
        std::env::set_var(key, "/tmp/perene2-test-xyz");
        assert_eq!(state_dir(), PathBuf::from("/tmp/perene2-test-xyz"));
        assert_eq!(
            scrollback_dir(),
            PathBuf::from("/tmp/perene2-test-xyz/scrollback")
        );
        match prev {
            Some(v) => std::env::set_var(key, v),
            None => std::env::remove_var(key),
        }
    }
}
