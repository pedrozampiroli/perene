//! Ponto de entrada do shell Tauri do Perene v2.
//!
//! A partir do M1 a UI é um cliente fino do `perene-daemon`: os comandos de
//! terminal viram mensagens IPC. O daemon detém os PTYs e sobrevive à janela.

mod client;

use client::DaemonClient;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(DaemonClient::default())
        .invoke_handler(tauri::generate_handler![
            client::terminal_spawn,
            client::terminal_write,
            client::terminal_resize,
            client::terminal_kill,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Perene");
}
