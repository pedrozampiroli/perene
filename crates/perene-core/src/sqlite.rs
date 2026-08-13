//! Consulta SQLite via o binário `sqlite3` (mesma abordagem do v1 — evita uma
//! dependência pesada de crate). Usado só para o OpenCode (histórico + usage).
//! Retorna `None` se o `sqlite3` não estiver disponível (ex.: Windows sem ele).

use std::path::Path;
use std::process::Command;

/// Evita a janela de console piscando no Windows ao spawnar o `sqlite3` (app de
/// console) a partir do Perene, que não tem console próprio. Sem efeito no
/// mac/Linux.
fn no_window(cmd: &mut Command) -> &mut Command {
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn sqlite_bin() -> Option<&'static str> {
    const CANDIDATES: &[&str] = &[
        "/usr/bin/sqlite3",
        "/opt/homebrew/bin/sqlite3",
        "/usr/local/bin/sqlite3",
        "sqlite3",
    ];
    CANDIDATES
        .iter()
        .copied()
        .find(|c| c == &"sqlite3" || Path::new(c).exists())
}

/// Roda `sql` em modo `-readonly -json` e devolve o JSON (array de objetos).
pub fn query_json(db: &Path, sql: &str) -> Option<String> {
    let bin = sqlite_bin()?;
    let mut cmd = Command::new(bin);
    cmd.arg("-readonly").arg("-json").arg(db).arg(sql);
    let out = no_window(&mut cmd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(if s.is_empty() { "[]".to_string() } else { s })
}

/// Roda `sql` esperando UMA linha com colunas separadas por `|` (agregações).
pub fn query_row(db: &Path, sql: &str) -> Option<Vec<String>> {
    let bin = sqlite_bin()?;
    let mut cmd = Command::new(bin);
    cmd.arg("-readonly").arg("-separator").arg("|").arg(db).arg(sql);
    let out = no_window(&mut cmd).output().ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(s.split('|').map(|c| c.to_string()).collect())
}
