//! Consulta SQLite via o binário `sqlite3` (mesma abordagem do v1 — evita uma
//! dependência pesada de crate). Usado só para o OpenCode (histórico + usage).
//! Retorna `None` se o `sqlite3` não estiver disponível (ex.: Windows sem ele).

use std::path::Path;
use std::process::Command;

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
    let out = Command::new(bin)
        .arg("-readonly")
        .arg("-json")
        .arg(db)
        .arg(sql)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(if s.is_empty() { "[]".to_string() } else { s })
}

/// Roda `sql` esperando UMA linha com colunas separadas por `|` (agregações).
pub fn query_row(db: &Path, sql: &str) -> Option<Vec<String>> {
    let bin = sqlite_bin()?;
    let out = Command::new(bin)
        .arg("-readonly")
        .arg("-separator")
        .arg("|")
        .arg(db)
        .arg(sql)
        .output()
        .ok()?;
    if !out.status.success() {
        return None;
    }
    let s = String::from_utf8_lossy(&out.stdout).trim().to_string();
    Some(s.split('|').map(|c| c.to_string()).collect())
}
