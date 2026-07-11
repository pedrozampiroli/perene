//! Ponto de entrada do shell Tauri do Perene v2.

mod pty;

use pty::PtyManager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(PtyManager::default())
        .invoke_handler(tauri::generate_handler![
            pty::terminal_spawn,
            pty::terminal_write,
            pty::terminal_resize,
            pty::terminal_kill,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Perene");
}
