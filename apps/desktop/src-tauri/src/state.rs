//! Comandos de estado: manifest, settings, paste de imagem e utilitários.
//!
//! A UI é a dona do manifest (mutações vêm de ações do usuário); estes comandos
//! só fazem I/O atômico via `perene-core`. Um mutex serializa as escritas para
//! nunca haver duas gravações concorrentes no mesmo tmp.

use parking_lot::Mutex;

use perene_core::models::now_millis;
use perene_core::{paths, Manifest, ManifestStore, Settings, SettingsStore};

/// Serializa gravações de estado.
#[derive(Default)]
pub struct Persist {
    lock: Mutex<()>,
}

fn home_dir_string() -> String {
    std::env::var("HOME")
        .or_else(|_| std::env::var("USERPROFILE"))
        .unwrap_or_else(|_| ".".to_string())
}

#[tauri::command]
pub fn manifest_load() -> Result<Manifest, String> {
    let store = ManifestStore::at_state_dir();
    match store.load().map_err(|e| e.to_string())? {
        Some(m) => Ok(m),
        None => {
            // Primeira execução: manifest inicial no home, já persistido.
            let m = Manifest::bootstrap(&home_dir_string());
            store.save(&m).map_err(|e| e.to_string())?;
            Ok(m)
        }
    }
}

#[tauri::command]
pub fn manifest_save(state: tauri::State<'_, Persist>, manifest: Manifest) -> Result<(), String> {
    let _g = state.lock.lock();
    ManifestStore::at_state_dir()
        .save(&manifest)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_load() -> Result<Settings, String> {
    SettingsStore::at_state_dir().load().map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_save(state: tauri::State<'_, Persist>, settings: Settings) -> Result<(), String> {
    let _g = state.lock.lock();
    SettingsStore::at_state_dir()
        .save(&settings)
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn home_dir() -> String {
    home_dir_string()
}

/// Salva bytes de imagem (já PNG, vindos do clipboard) em `~/.perene2/paste/` e
/// devolve o caminho absoluto — a UI cola esse path no terminal.
#[tauri::command]
pub fn save_paste_image(data_b64: String) -> Result<String, String> {
    use base64::Engine;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(data_b64.as_bytes())
        .map_err(|e| format!("base64 inválido: {e}"))?;
    let dir = paths::paste_dir();
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let path = dir.join(format!("paste_{}.png", now_millis()));
    std::fs::write(&path, &bytes).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().to_string())
}
