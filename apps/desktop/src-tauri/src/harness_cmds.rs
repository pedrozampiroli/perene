//! Comandos de MCP e skills dos harnesses. Fachada fina sobre
//! `perene_core::harness` — nenhuma regra de negócio mora aqui.

use std::path::{Path, PathBuf};

use perene_core::harness::{Harness, HarnessStore, McpServer, Skill};

/// Estado de uma ferramenta para a UI: se está configurada e o que ela tem.
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HarnessInfo {
    pub id: String,
    /// Caminho do arquivo de config, para a UI poder mostrar/abrir.
    pub config_path: String,
    /// `true` se o arquivo existe — a CLI provavelmente está instalada.
    pub configured: bool,
    pub servers: Vec<McpServer>,
    /// Erro de leitura (ex.: JSON quebrado na mão). A UI mostra em vez de
    /// fingir que a ferramenta não tem servidor nenhum.
    pub error: Option<String>,
}

fn parse(id: &str) -> Result<Harness, String> {
    match id {
        "claude" => Ok(Harness::Claude),
        "codex" => Ok(Harness::Codex),
        "opencode" => Ok(Harness::OpenCode),
        other => Err(format!("harness desconhecido: {other}")),
    }
}

#[tauri::command]
pub fn harness_list() -> Vec<HarnessInfo> {
    let store = HarnessStore::at_home();
    Harness::all()
        .into_iter()
        .map(|h| {
            let (servers, error) = match store.list_mcp(h) {
                Ok(s) => (s, None),
                Err(e) => (vec![], Some(e)),
            };
            HarnessInfo {
                id: h.id().to_string(),
                config_path: store.paths().config_for(h).to_string_lossy().to_string(),
                configured: store.is_configured(h),
                servers,
                error,
            }
        })
        .collect()
}

#[tauri::command]
pub fn mcp_upsert(harness: String, server: McpServer) -> Result<(), String> {
    HarnessStore::at_home().upsert_mcp(parse(&harness)?, &server)
}

#[tauri::command]
pub fn mcp_remove(harness: String, name: String) -> Result<(), String> {
    HarnessStore::at_home().remove_mcp(parse(&harness)?, &name)
}

#[tauri::command]
pub fn mcp_set_enabled(harness: String, name: String, enabled: bool) -> Result<(), String> {
    HarnessStore::at_home().set_enabled(parse(&harness)?, &name, enabled)
}

/// Copia um servidor já configurado para outra ferramenta — o caso comum de
/// "quero esse MCP no Codex também" sem redigitar comando, args e env.
#[tauri::command]
pub fn mcp_copy_to(from: String, to: String, name: String) -> Result<(), String> {
    let store = HarnessStore::at_home();
    let (from, to) = (parse(&from)?, parse(&to)?);
    let server = store
        .list_mcp(from)?
        .into_iter()
        .find(|s| s.name == name)
        .ok_or_else(|| format!("servidor '{name}' não encontrado"))?;
    store.upsert_mcp(to, &server)
}

fn project_path(p: &Option<String>) -> Option<PathBuf> {
    p.as_ref()
        .filter(|s| !s.is_empty())
        .map(|s| PathBuf::from(s.as_str()))
}

#[tauri::command]
pub fn skills_list(project: Option<String>) -> Vec<Skill> {
    HarnessStore::at_home().list_skills(project_path(&project).as_deref())
}

#[tauri::command]
pub fn skill_install(source: String, project: Option<String>) -> Result<Skill, String> {
    HarnessStore::at_home()
        .install_skill(Path::new(&source), project_path(&project).as_deref())
}

#[tauri::command]
pub fn skill_remove(path: String, project: Option<String>) -> Result<(), String> {
    HarnessStore::at_home().remove_skill(Path::new(&path), project_path(&project).as_deref())
}
