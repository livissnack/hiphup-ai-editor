mod commands;

use commands::{ai, cli_shim, fs, git, state, terminal};
use std::sync::{Arc, Mutex};

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let cli_launch = cli_shim::parse_cli_launch_entries();
    tauri::Builder::default()
        .manage(terminal::TerminalSessions(Arc::new(Mutex::new(std::collections::HashMap::new()))))
        .manage(cli_shim::CliLaunchState(Mutex::new(cli_launch)))
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .invoke_handler(tauri::generate_handler![
            greet,
            fs::list_dir,
            fs::search_files_by_name,
            fs::read_file,
            fs::read_file_with_encoding,
            fs::read_image_data_url,
            fs::write_file,
            fs::write_file_with_encoding,
            fs::create_file,
            fs::create_folder,
            fs::rename_path,
            fs::delete_path,
            fs::copy_path,
            fs::move_path,
            fs::path_exists,
            fs::open_path_in_file_manager,
            fs::reveal_in_folder,
            git::git_overview,
            git::git_tree_decorations,
            git::git_exec,
            git::git_fetch,
            git::git_pull,
            git::git_push,
            git::git_commit_all,
            git::git_stash_save,
            git::git_stash_list,
            git::git_stash_pop,
            state::load_app_state,
            state::save_app_state,
            state::load_ai_config,
            state::save_ai_config,
            state::load_ai_chat_state,
            state::save_ai_chat_state,
            state::clear_ai_chat_state,
            terminal::terminal_create,
            terminal::terminal_write,
            terminal::terminal_resize,
            terminal::terminal_kill,
            ai::aihub_chat,
            ai::aihub_chat_stream,
            ai::aihub_connection_status,
            ai::openrouter_list_models,
            cli_shim::take_cli_launch_paths,
            cli_shim::install_cli_in_path
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
