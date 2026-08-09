//! Ponto de entrada do shell Tauri do Perene v2.
//!
//! A partir do M1 a UI é um cliente fino do `perene-daemon`: os comandos de
//! terminal viram mensagens IPC. O daemon detém os PTYs e sobrevive à janela.
//! No M3 entram os comandos de estado (manifest/settings/paste).

mod cli_notify;
mod client;
mod files;
mod shells;
mod state;

use client::DaemonClient;
use state::Persist;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Configura o bell (BEL) do claude/codex/opencode em background: I/O em
    // disco não pode atrasar a janela abrindo, e uma falha aqui (ex.: config
    // corrompido) nunca deve derrubar o app.
    std::thread::spawn(cli_notify::ensure_all);

    tauri::Builder::default()
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_notification::init())
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
            shells::list_shells,
            state::session_history_load,
            state::session_transcript,
            state::usage_load,
            files::fs_list_dir,
            files::fs_read_file,
            files::fs_write_file,
            files::fs_list_files,
            files::search_in_files,
            files::replace_in_files,
            files::git_status,
            files::git_diff,
            files::git_file_versions,
            files::git_branches,
            files::git_checkout,
            files::git_create_branch,
            files::git_fetch,
            files::git_pull,
            files::git_push,
            files::git_open_pr,
            files::git_log,
            files::git_show,
            files::git_commit,
            files::git_worktree_list,
            files::git_worktree_add,
            files::create_project_worktree,
        ])
        .run(tauri::generate_context!())
        .expect("erro ao iniciar o Perene");
}
