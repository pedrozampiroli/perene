//! Histórico de sessões dos 3 harnesses (Claude/Codex/OpenCode).
//!
//! Portado do v1 (`SessionHistory.swift`). Listagem é barata (só metadados);
//! transcript é lido sob demanda. Fontes:
//!  - Claude:  `~/.claude/projects/<cwd-encoded>/<session-id>.jsonl`
//!  - Codex:   `~/.codex/sessions/**/rollout-*.jsonl` (1ª linha = session_meta)
//!  - OpenCode: SQLite `~/.local/share/opencode/opencode.db` (tabela `session`)

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::home_dir;

/// Um registro de sessão passada de qualquer harness.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionRecord {
    /// "claude" | "codex" | "opencode" (também é o toolProfileId).
    pub harness: String,
    /// Id da sessão usado para retomar.
    pub session_id: String,
    /// Diretório onde a sessão rodou.
    pub project_path: String,
    #[serde(default)]
    pub title: Option<String>,
    /// Epoch em milissegundos (para ordenar/filtrar no front).
    pub date_ms: i64,
    /// Arquivo de origem (jsonl) para o transcript; `None` no OpenCode (SQLite).
    #[serde(default)]
    pub source_path: Option<String>,
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

fn read_first_line(path: &Path) -> Option<String> {
    let content = std::fs::read_to_string(path).ok()?;
    content.lines().next().map(|s| s.to_string())
}

/// Carrega o histórico completo, ordenado do mais recente pro mais antigo.
pub fn load_all() -> Vec<SessionRecord> {
    let mut all = load_claude();
    all.extend(load_codex());
    all.extend(load_opencode());
    all.sort_by(|a, b| b.date_ms.cmp(&a.date_ms));
    all
}

// ── Claude ───────────────────────────────────────────────────────────────────

fn load_claude() -> Vec<SessionRecord> {
    let base = home_dir().join(".claude/projects");
    let mut out = Vec::new();
    let Ok(projects) = std::fs::read_dir(&base) else {
        return out;
    };
    for project in projects.flatten() {
        let cwd = decode_claude_dir(&project.file_name().to_string_lossy());
        let Ok(files) = std::fs::read_dir(project.path()) else {
            continue;
        };
        for file in files.flatten() {
            let path = file.path();
            if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
                continue;
            }
            let session_id = path
                .file_stem()
                .map(|s| s.to_string_lossy().to_string())
                .unwrap_or_default();
            out.push(SessionRecord {
                harness: "claude".into(),
                session_id,
                project_path: cwd.clone(),
                title: None,
                date_ms: mtime_ms(&path),
                source_path: Some(path.to_string_lossy().to_string()),
            });
        }
    }
    out
}

/// O Claude codifica o cwd trocando "/" por "-" (perde hífens reais, ok pra
/// exibir/filtrar).
fn decode_claude_dir(name: &str) -> String {
    if let Some(rest) = name.strip_prefix('-') {
        format!("/{}", rest.replace('-', "/"))
    } else {
        name.to_string()
    }
}

// ── Codex ────────────────────────────────────────────────────────────────────

fn load_codex() -> Vec<SessionRecord> {
    let base = home_dir().join(".codex/sessions");
    let mut out = Vec::new();
    let mut rollouts = Vec::new();
    collect_rollouts(&base, &mut rollouts);
    for path in rollouts {
        let Some(line) = read_first_line(&path) else {
            continue;
        };
        let Ok(meta) = serde_json::from_str::<serde_json::Value>(&line) else {
            continue;
        };
        if meta.get("type").and_then(|v| v.as_str()) != Some("session_meta") {
            continue;
        }
        let payload = &meta["payload"];
        let Some(id) = payload.get("id").and_then(|v| v.as_str()) else {
            continue;
        };
        let cwd = payload.get("cwd").and_then(|v| v.as_str()).unwrap_or("~");
        let date_ms = payload
            .get("timestamp")
            .and_then(|v| v.as_str())
            .and_then(iso_to_ms)
            .unwrap_or_else(|| mtime_ms(&path));
        out.push(SessionRecord {
            harness: "codex".into(),
            session_id: id.to_string(),
            project_path: cwd.to_string(),
            title: None,
            date_ms,
            source_path: Some(path.to_string_lossy().to_string()),
        });
    }
    out
}

/// Percorre recursivamente à procura de `rollout-*.jsonl`.
fn collect_rollouts(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_rollouts(&path, out);
        } else if let Some(name) = path.file_name().and_then(|n| n.to_str()) {
            if name.starts_with("rollout-") && name.ends_with(".jsonl") {
                out.push(path);
            }
        }
    }
}

/// ISO-8601 → epoch ms (aceita fração de segundos).
fn iso_to_ms(s: &str) -> Option<i64> {
    // Sem chrono: parse manual do formato "YYYY-MM-DDTHH:MM:SS[.fff]Z".
    // Suficiente para ordenar; datas locais não são o foco.
    let bytes = s.as_bytes();
    if bytes.len() < 19 {
        return None;
    }
    let year: i64 = s.get(0..4)?.parse().ok()?;
    let month: i64 = s.get(5..7)?.parse().ok()?;
    let day: i64 = s.get(8..10)?.parse().ok()?;
    let hour: i64 = s.get(11..13)?.parse().ok()?;
    let min: i64 = s.get(14..16)?.parse().ok()?;
    let sec: i64 = s.get(17..19)?.parse().ok()?;
    // Dias desde epoch via algoritmo civil (Howard Hinnant).
    let y = if month <= 2 { year - 1 } else { year };
    let era = if y >= 0 { y } else { y - 399 } / 400;
    let yoe = y - era * 400;
    let doy = (153 * (if month > 2 { month - 3 } else { month + 9 }) + 2) / 5 + day - 1;
    let doe = yoe * 365 + yoe / 4 - yoe / 100 + doy;
    let days = era * 146097 + doe - 719468;
    Some(((days * 86400 + hour * 3600 + min * 60 + sec) * 1000) as i64)
}

// ── OpenCode (SQLite) ────────────────────────────────────────────────────────

fn load_opencode() -> Vec<SessionRecord> {
    let db = home_dir().join(".local/share/opencode/opencode.db");
    if !db.exists() {
        return Vec::new();
    }
    let sql = "SELECT id, directory, title, time_updated FROM session ORDER BY time_updated DESC;";
    let Some(json) = crate::sqlite::query_json(&db, sql) else {
        return Vec::new();
    };
    let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else {
        return Vec::new();
    };
    rows.into_iter()
        .filter_map(|row| {
            let id = row.get("id")?.as_str()?.to_string();
            let ms = row
                .get("time_updated")
                .and_then(|v| v.as_f64())
                .unwrap_or(0.0);
            Some(SessionRecord {
                harness: "opencode".into(),
                session_id: id,
                project_path: row
                    .get("directory")
                    .and_then(|v| v.as_str())
                    .unwrap_or("~")
                    .to_string(),
                title: row
                    .get("title")
                    .and_then(|v| v.as_str())
                    .map(|s| s.to_string()),
                date_ms: ms as i64,
                source_path: None,
            })
        })
        .collect()
}

// ── Transcript (preview sob demanda) ─────────────────────────────────────────

/// Preview curto de um transcript (primeiros turnos user/assistant).
pub fn transcript(record: &SessionRecord, max_chars: usize) -> String {
    match record.harness.as_str() {
        "claude" => jsonl_transcript(record.source_path.as_deref(), max_chars, true),
        "codex" => jsonl_transcript(record.source_path.as_deref(), max_chars, false),
        "opencode" => opencode_transcript(&record.session_id, max_chars),
        _ => String::new(),
    }
}

fn jsonl_transcript(path: Option<&str>, max_chars: usize, claude: bool) -> String {
    let Some(path) = path else {
        return String::new();
    };
    let Ok(content) = std::fs::read_to_string(path) else {
        return String::new();
    };
    let mut out = String::new();
    for line in content.lines().take(600) {
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) else {
            continue;
        };
        let (role, container) = if claude {
            let role = obj.get("type").and_then(|v| v.as_str()).unwrap_or("");
            (role, obj.get("message").unwrap_or(&obj))
        } else {
            let payload = obj.get("payload").unwrap_or(&obj);
            let role = payload.get("role").and_then(|v| v.as_str()).unwrap_or("");
            (role, payload)
        };
        if role != "user" && role != "assistant" {
            continue;
        }
        let Some(text) = content_text(container) else {
            continue;
        };
        if text.is_empty() || text.starts_with('<') {
            continue;
        }
        let snippet: String = text.chars().take(400).collect();
        out.push_str(&format!("▸ {role}: {snippet}\n\n"));
        if out.len() >= max_chars {
            break;
        }
    }
    out.chars().take(max_chars).collect()
}

fn content_text(container: &serde_json::Value) -> Option<String> {
    if let Some(s) = container.get("content").and_then(|v| v.as_str()) {
        return Some(s.trim().to_string());
    }
    if let Some(parts) = container.get("content").and_then(|v| v.as_array()) {
        let texts: Vec<String> = parts
            .iter()
            .filter_map(|p| p.get("text").and_then(|v| v.as_str()).map(String::from))
            .collect();
        if !texts.is_empty() {
            return Some(texts.join(" ").trim().to_string());
        }
    }
    if let Some(s) = container.get("text").and_then(|v| v.as_str()) {
        return Some(s.trim().to_string());
    }
    None
}

fn opencode_transcript(session_id: &str, max_chars: usize) -> String {
    let db = home_dir().join(".local/share/opencode/opencode.db");
    let esc = session_id.replace('\'', "''");
    let sql = format!(
        "SELECT data FROM session_message WHERE session_id='{esc}' ORDER BY seq LIMIT 40;"
    );
    let Some(json) = crate::sqlite::query_json(&db, &sql) else {
        return String::new();
    };
    let Ok(rows) = serde_json::from_str::<Vec<serde_json::Value>>(&json) else {
        return String::new();
    };
    let mut out = String::new();
    for row in rows {
        let Some(data) = row.get("data").and_then(|v| v.as_str()) else {
            continue;
        };
        let Ok(obj) = serde_json::from_str::<serde_json::Value>(data) else {
            continue;
        };
        if let Some(text) = content_text(&obj) {
            if !text.is_empty() {
                out.push_str(&format!("▸ {text}\n\n"));
                if out.len() >= max_chars {
                    break;
                }
            }
        }
    }
    out.chars().take(max_chars).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn decode_claude_dir_restores_slashes() {
        assert_eq!(
            decode_claude_dir("-Users-pedro-Projects-foo"),
            "/Users/pedro/Projects/foo"
        );
    }

    #[test]
    fn iso_to_ms_parses_utc() {
        // 2024-01-01T00:00:00Z = 1704067200000 ms
        assert_eq!(iso_to_ms("2024-01-01T00:00:00Z"), Some(1704067200000));
    }

    #[test]
    fn load_all_never_panics_without_dirs() {
        // Não deve explodir mesmo sem ~/.claude etc. (só retorna vazio ou o que existir).
        let _ = load_all();
    }
}
