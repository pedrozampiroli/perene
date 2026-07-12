//! Ponto de entrada do shell Tauri do Perene v2.
//!
//! A partir do M1 a UI é um cliente fino do `perene-daemon`: os comandos de
//! terminal viram mensagens IPC. O daemon detém os PTYs e sobrevive à janela.
//! No M3 entram os comandos de estado (manifest/settings/paste).

mod client;
mod files;
mod state;

use client::DaemonClient;
use state::Persist;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .manage(DaemonClient::default())
        .manage(Persist::default())
        .invoke_handler(tauri::generate_handler![
            client::terminal_spawn,
            client::terminal_write,
            client::terminal_resize,
            client::terminal_kill,
            state::manifest_load,
            state::manifest_save,
            state::settings_load,
            state::settings_save,
            state::home_dir,
            state::save_paste_image,
            state::session_history_load,
            state::session_transcript,
            state::usage_load,
            files::fs_list_dir,
            files::fs_read_file,
            files::fs_write_file,
            files::git_status,
            files::git_diff,
            files::git_branches,
            files::git_checkout,
            files::git_create_branch,
            files::git_fetch,
            files::git_pull,
            files::git_open_pr,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Perene");
}
