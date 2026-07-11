//! Comandos de filesystem + git para o visualizador de arquivos (M5).
//!
//! Git é acessado via o binário `git` (como o v1) rodando com o cwd do repo;
//! PR abre no browser via `gh`. Tudo read-only exceto `fs_write_file` (⌘S) e as
//! ações git explícitas (checkout/branch/fetch/pull).

use std::path::PathBuf;
use std::process::Command;

use serde::{Deserialize, Serialize};

// ── Filesystem ───────────────────────────────────────────────────────────────

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DirEntry {
    pub name: String,
    pub path: String,
    pub is_dir: bool,
}

/// Lista um diretório (pastas primeiro, alfabético), escondendo `.git`.
#[tauri::command]
pub fn fs_list_dir(path: String) -> Result<Vec<DirEntry>, String> {
    let mut out = Vec::new();
    for entry in std::fs::read_dir(&path).map_err(|e| e.to_string())?.flatten() {
        let name = entry.file_name().to_string_lossy().to_string();
        if name == ".git" {
            continue;
        }
        let is_dir = entry.file_type().map(|t| t.is_dir()).unwrap_or(false);
        out.push(DirEntry {
            name,
            path: entry.path().to_string_lossy().to_string(),
            is_dir,
        });
    }
    out.sort_by(|a, b| match (a.is_dir, b.is_dir) {
        (true, false) => std::cmp::Ordering::Less,
        (false, true) => std::cmp::Ordering::Greater,
        _ => a.name.to_lowercase().cmp(&b.name.to_lowercase()),
    });
    Ok(out)
}

#[tauri::command]
pub fn fs_read_file(path: String) -> Result<String, String> {
    std::fs::read_to_string(&path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn fs_write_file(path: String, content: String) -> Result<(), String> {
    std::fs::write(&path, content).map_err(|e| e.to_string())
}

// ── Git ──────────────────────────────────────────────────────────────────────

fn git(repo: &str, args: &[&str]) -> Result<String, String> {
    let out = Command::new("git")
        .args(["-C", repo])
        .args(args)
        .output()
        .map_err(|e| format!("git não encontrado: {e}"))?;
    if !out.status.success() {
        return Err(String::from_utf8_lossy(&out.stderr).trim().to_string());
    }
    Ok(String::from_utf8_lossy(&out.stdout).to_string())
}

/// Sobe a árvore procurando a raiz do repo (onde há `.git`).
fn repo_root(start: &str) -> Option<String> {
    let mut dir = PathBuf::from(start);
    loop {
        if dir.join(".git").exists() {
            return Some(dir.to_string_lossy().to_string());
        }
        if !dir.pop() {
            return None;
        }
    }
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitFile {
    pub path: String,
    /// Código de 2 chars do porcelain (ex.: " M", "??", "A ", "D ").
    pub status: String,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GitStatus {
    pub is_repo: bool,
    pub root: Option<String>,
    pub branch: String,
    pub ahead: u32,
    pub behind: u32,
    pub dirty: bool,
    pub files: Vec<GitFile>,
}

/// Status do repo que contém `path` (branch, ahead/behind, arquivos modificados).
#[tauri::command]
pub fn git_status(path: String) -> GitStatus {
    let Some(root) = repo_root(&path) else {
        return GitStatus {
            is_repo: false,
            root: None,
            branch: String::new(),
            ahead: 0,
            behind: 0,
            dirty: false,
            files: Vec::new(),
        };
    };
    let raw = git(&root, &["status", "--porcelain=v1", "-b"]).unwrap_or_default();
    let mut branch = String::new();
    let (mut ahead, mut behind) = (0u32, 0u32);
    let mut files = Vec::new();
    for line in raw.lines() {
        if let Some(rest) = line.strip_prefix("## ") {
            // Ex.: "main...origin/main [ahead 1, behind 2]"
            branch = rest
                .split(['.', ' '])
                .next()
                .unwrap_or("")
                .to_string();
            if let Some(a) = extract_after(rest, "ahead ") {
                ahead = a;
            }
            if let Some(b) = extract_after(rest, "behind ") {
                behind = b;
            }
        } else if line.len() >= 3 {
            files.push(GitFile {
                status: line[..2].to_string(),
                path: line[3..].to_string(),
            });
        }
    }
    GitStatus {
        is_repo: true,
        branch,
        ahead,
        behind,
        dirty: !files.is_empty(),
        files,
        root: Some(root),
    }
}

fn extract_after(s: &str, marker: &str) -> Option<u32> {
    let idx = s.find(marker)? + marker.len();
    let digits: String = s[idx..].chars().take_while(|c| c.is_ascii_digit()).collect();
    digits.parse().ok()
}

/// Diff de um arquivo (inclui staged + unstaged vs HEAD). `file` relativo à raiz.
#[tauri::command]
pub fn git_diff(root: String, file: String) -> Result<String, String> {
    // `git diff HEAD` cobre mudanças staged e unstaged; para untracked, mostra
    // o conteúdo como adição.
    let tracked = git(&root, &["ls-files", "--error-unmatch", "--", &file]).is_ok();
    if tracked {
        git(&root, &["diff", "HEAD", "--", &file])
    } else {
        git(&root, &["diff", "--no-index", "--", "/dev/null", &file])
            .or_else(|e| if e.is_empty() { Ok(String::new()) } else { Ok(e) })
    }
}

#[tauri::command]
pub fn git_branches(root: String) -> Result<Vec<String>, String> {
    let raw = git(&root, &["branch", "--format=%(refname:short)"])?;
    Ok(raw.lines().map(|s| s.trim().to_string()).filter(|s| !s.is_empty()).collect())
}

#[tauri::command]
pub fn git_checkout(root: String, branch: String) -> Result<(), String> {
    git(&root, &["checkout", &branch]).map(|_| ())
}

#[tauri::command]
pub fn git_create_branch(root: String, branch: String) -> Result<(), String> {
    git(&root, &["checkout", "-b", &branch]).map(|_| ())
}

#[tauri::command]
pub fn git_fetch(root: String) -> Result<(), String> {
    git(&root, &["fetch", "--all", "--prune"]).map(|_| ())
}

#[tauri::command]
pub fn git_pull(root: String) -> Result<String, String> {
    git(&root, &["pull", "--ff-only"])
}

/// Abre o PR do branch atual no browser via `gh`. Cria (rascunho web) se não
/// existir; se já existir, apenas abre.
#[tauri::command]
pub fn git_open_pr(root: String) -> Result<(), String> {
    let run = |args: &[&str]| -> Result<(), String> {
        let out = Command::new("gh")
            .current_dir(&root)
            .args(args)
            .output()
            .map_err(|e| format!("gh não encontrado: {e}"))?;
        if out.status.success() {
            Ok(())
        } else {
            Err(String::from_utf8_lossy(&out.stderr).trim().to_string())
        }
    };
    // Abre um PR existente; se não houver, cai pra criação no browser.
    run(&["pr", "view", "--web"]).or_else(|_| run(&["pr", "create", "--web"]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn git_status_detects_this_repo() {
        // O crate vive dentro do repo perene-tauri.
        let cwd = std::env::current_dir().unwrap().to_string_lossy().to_string();
        let gs = git_status(cwd);
        assert!(gs.is_repo, "deveria detectar o repositório git");
        assert!(!gs.branch.is_empty(), "branch não pode ser vazio");
        assert!(gs.root.is_some());
    }

    #[test]
    fn fs_write_then_read_roundtrips() {
        let path = std::env::temp_dir().join(format!("perene-fs-{}.txt", std::process::id()));
        let p = path.to_string_lossy().to_string();
        fs_write_file(p.clone(), "olá mundo".into()).unwrap();
        assert_eq!(fs_read_file(p.clone()).unwrap(), "olá mundo");
        let _ = std::fs::remove_file(&path);
    }

    #[test]
    fn extract_after_parses_ahead_behind() {
        assert_eq!(extract_after("main...origin/main [ahead 3, behind 2]", "ahead "), Some(3));
        assert_eq!(extract_after("main...origin/main [ahead 3, behind 2]", "behind "), Some(2));
        assert_eq!(extract_after("main", "ahead "), None);
    }
}
