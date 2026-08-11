//! Comandos de tema. A lógica toda vive em `perene-core::theme`/`themes_store`;
//! aqui só expomos para a webview.

use perene_core::{Theme, ThemesStore};

#[tauri::command]
pub fn themes_list() -> Vec<Theme> {
    ThemesStore::at_state_dir().list()
}

/// Tema ativo, resolvido a partir do id salvo nas settings. Um id que não existe
/// mais (tema removido na mão) cai no padrão em vez de deixar a UI sem cores.
#[tauri::command]
pub fn theme_active(id: String) -> Theme {
    let store = ThemesStore::at_state_dir();
    if id.is_empty() {
        return Theme::dark_plus();
    }
    store.get(&id).unwrap_or_else(Theme::dark_plus)
}

/// Importa um arquivo de tema do Zed (uma família pode trazer vários temas).
#[tauri::command]
pub fn theme_import_zed(path: String) -> Result<Vec<Theme>, String> {
    ThemesStore::at_state_dir().import_zed_file(std::path::Path::new(&path))
}

#[tauri::command]
pub fn theme_remove(id: String) -> Result<(), String> {
    ThemesStore::at_state_dir().remove(&id)
}
