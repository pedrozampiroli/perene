//! Uso de tokens/custo por harness. Portado do v1 (`UsageProvider.swift`).
//!
//! Claude/Codex são pesados em arquivos → cache por arquivo (path + mtime) em
//! `~/.perene2/usage-cache.json`: só arquivos novos/alterados são relidos.
//! Alvo: < 10s frio / < 1s quente. Rodar fora da thread da UI.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

use crate::paths::home_dir;

/// Uso agregado de um harness.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UsageStats {
    pub harness: String,
    pub sessions: u64,
    pub input: u64,
    pub output: u64,
    /// Só o OpenCode registra custo.
    pub cost: f64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct FileUsage {
    m: i64, // mtime ms
    i: u64,
    o: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
struct Cache {
    #[serde(default)]
    claude: HashMap<String, FileUsage>,
    #[serde(default)]
    codex: HashMap<String, FileUsage>,
}

fn cache_path() -> PathBuf {
    crate::paths::state_dir().join("usage-cache.json")
}

fn mtime_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|m| m.modified())
        .ok()
        .and_then(|t| t.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|d| d.as_millis() as i64)
        .unwrap_or(0)
}

/// Carrega o uso dos 3 harnesses (cache em `~/.perene2/usage-cache.json`).
pub fn load() -> Vec<UsageStats> {
    load_with_cache(&cache_path())
}

/// Igual, mas com caminho de cache injetável (testes usam tempdir — lição #1).
pub fn load_with_cache(cache_file: &Path) -> Vec<UsageStats> {
    let mut cache: Cache = crate::store::read_json_with_backup(cache_file)
        .ok()
        .flatten()
        .unwrap_or_default();
    let result = vec![claude(&mut cache), codex(&mut cache), opencode()];
    let _ = crate::store::write_json_atomic(cache_file, &cache);
    result
}

// ── Claude (soma message.usage; cache por arquivo) ───────────────────────────

fn claude(cache: &mut Cache) -> UsageStats {
    let base = home_dir().join(".claude/projects");
    let mut files = Vec::new();
    if let Ok(projects) = std::fs::read_dir(&base) {
        for project in projects.flatten() {
            if let Ok(entries) = std::fs::read_dir(project.path()) {
                for f in entries.flatten() {
                    let p = f.path();
                    if p.extension().and_then(|e| e.to_str()) == Some("jsonl") {
                        files.push(p);
                    }
                }
            }
        }
    }
    aggregate_files("claude", files, &mut cache.claude, file_tokens)
}

/// Soma `"input_tokens"`/`"output_tokens"` via regex (sem parse de JSON).
fn file_tokens(path: &Path) -> (u64, u64) {
    use regex::Regex;
    use std::sync::OnceLock;
    static IN_RE: OnceLock<Regex> = OnceLock::new();
    static OUT_RE: OnceLock<Regex> = OnceLock::new();
    let in_re = IN_RE.get_or_init(|| Regex::new(r#""input_tokens":\s*(\d+)"#).unwrap());
    let out_re = OUT_RE.get_or_init(|| Regex::new(r#""output_tokens":\s*(\d+)"#).unwrap());
    let Ok(text) = std::fs::read_to_string(path) else {
        return (0, 0);
    };
    let sum = |re: &Regex| -> u64 {
        re.captures_iter(&text)
            .filter_map(|c| c.get(1))
            .filter_map(|m| m.as_str().parse::<u64>().ok())
            .sum()
    };
    (sum(in_re), sum(out_re))
}

// ── Codex (último total_token_usage cumulativo por rollout) ──────────────────

fn codex(cache: &mut Cache) -> UsageStats {
    let base = home_dir().join(".codex/sessions");
    let mut rollouts = Vec::new();
    collect_rollouts(&base, &mut rollouts);
    aggregate_files("codex", rollouts, &mut cache.codex, codex_file_tokens)
}

/// Último `total_token_usage` cumulativo de um rollout do Codex.
fn codex_file_tokens(path: &Path) -> (u64, u64) {
    let (mut i, mut o) = (0u64, 0u64);
    let Ok(content) = std::fs::read_to_string(path) else {
        return (0, 0);
    };
    for line in content.lines().filter(|l| l.contains("total_token_usage")) {
        if let Ok(obj) = serde_json::from_str::<serde_json::Value>(line) {
            if let Some(total) = obj
                .get("payload")
                .and_then(|p| p.get("info"))
                .and_then(|info| info.get("total_token_usage"))
            {
                i = total.get("input_tokens").and_then(|v| v.as_u64()).unwrap_or(i);
                o = total.get("output_tokens").and_then(|v| v.as_u64()).unwrap_or(o);
            }
        }
    }
    (i, o)
}

/// Agrega tokens de uma lista de arquivos com cache por mtime, processando os
/// arquivos novos/alterados EM PARALELO (rayon) — mantém o cold sob o alvo.
fn aggregate_files(
    harness: &str,
    files: Vec<PathBuf>,
    cache: &mut HashMap<String, FileUsage>,
    count: fn(&Path) -> (u64, u64),
) -> UsageStats {
    use rayon::prelude::*;

    let seen: std::collections::HashSet<String> =
        files.iter().map(|p| p.to_string_lossy().to_string()).collect();

    // Arquivos que precisam ser (re)lidos.
    let stale: Vec<(String, i64, PathBuf)> = files
        .into_iter()
        .filter_map(|p| {
            let key = p.to_string_lossy().to_string();
            let mtime = mtime_ms(&p);
            match cache.get(&key) {
                Some(hit) if hit.m == mtime => None,
                _ => Some((key, mtime, p)),
            }
        })
        .collect();

    // Leitura + contagem em paralelo (o gargalo).
    let fresh: Vec<(String, FileUsage)> = stale
        .par_iter()
        .map(|(key, mtime, path)| {
            let (i, o) = count(path);
            (key.clone(), FileUsage { m: *mtime, i, o })
        })
        .collect();
    for (key, fu) in fresh {
        cache.insert(key, fu);
    }
    cache.retain(|k, _| seen.contains(k));

    let mut s = UsageStats {
        harness: harness.to_string(),
        sessions: seen.len() as u64,
        ..Default::default()
    };
    for fu in cache.values() {
        s.input += fu.i;
        s.output += fu.o;
    }
    s
}

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

// ── OpenCode (agregação SQLite — instantâneo) ────────────────────────────────

fn opencode() -> UsageStats {
    let mut s = UsageStats {
        harness: "opencode".into(),
        ..Default::default()
    };
    let db = home_dir().join(".local/share/opencode/opencode.db");
    if !db.exists() {
        return s;
    }
    let sql = "SELECT count(*), coalesce(sum(tokens_input),0), coalesce(sum(tokens_output),0), coalesce(sum(cost),0) FROM session;";
    if let Some(cols) = crate::sqlite::query_row(&db, sql) {
        if cols.len() >= 4 {
            s.sessions = cols[0].parse().unwrap_or(0);
            s.input = cols[1].parse().unwrap_or(0);
            s.output = cols[2].parse().unwrap_or(0);
            s.cost = cols[3].parse().unwrap_or(0.0);
        }
    }
    s
}

/// Formata contagem de tokens de forma compacta (1.2M, 3.4K…).
pub fn format_tokens(n: u64) -> String {
    match n {
        1_000_000_000.. => format!("{:.2}B", n as f64 / 1e9),
        1_000_000.. => format!("{:.1}M", n as f64 / 1e6),
        1_000.. => format!("{:.1}K", n as f64 / 1e3),
        _ => n.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn format_tokens_scales() {
        assert_eq!(format_tokens(500), "500");
        assert_eq!(format_tokens(1500), "1.5K");
        assert_eq!(format_tokens(2_500_000), "2.5M");
    }

    #[test]
    fn load_never_panics_and_isolates_cache() {
        // Cache num tempdir — não toca ~/.perene2 (lição #1).
        let dir = tempfile::tempdir().unwrap();
        let _ = load_with_cache(&dir.path().join("usage-cache.json"));
    }
}
