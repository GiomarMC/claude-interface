mod claude;
mod files;
mod terminal;

use std::sync::Mutex;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(claude::Sessions(Mutex::new(Default::default())))
        .invoke_handler(tauri::generate_handler![
            claude::start_session,
            claude::send_line,
            claude::stop_session,
            files::read_attachment,
            files::git_info,
            files::list_sessions,
            files::load_session,
            files::write_text_file,
            files::run_shell,
            files::initial_cwd,
            files::user_settings,
            files::claude_theme,
            files::run_statusline,
            files::app_config,
            files::save_app_config,
            terminal::terminal_style,
            terminal::detected_terminals,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
