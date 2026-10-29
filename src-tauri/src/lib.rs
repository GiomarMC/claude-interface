mod claude;
mod files;
mod keys;
mod pty;
mod setup;
mod shell_env;
mod terminal;
mod updates;

use std::sync::Mutex;

use tauri::Manager;

/// Errores de la página en la terminal de `pnpm tauri dev` (ver app.html).
#[tauri::command]
fn log_js_error(message: String) {
    eprintln!("[página] {message}");
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    shell_env::warm_up();
    let builder = tauri::Builder::default();
    #[cfg(target_os = "macos")]
    let builder = builder.menu(app_menu);
    builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .manage(claude::Sessions(Mutex::new(Default::default())))
        .manage(pty::Ptys(Mutex::new(Default::default())))
        .manage(setup::SetupTask(Mutex::new(None)))
        // Cada ventana tiene sus propias sesiones ("<ventana>:<panel>").
        .on_window_event(|window, event| {
            if let tauri::WindowEvent::Destroyed = event {
                claude::stop_window(&window.state::<claude::Sessions>(), window.label());
                pty::kill_window(&window.state::<pty::Ptys>(), window.label());
            }
        })
        .invoke_handler(tauri::generate_handler![
            claude::start_session,
            claude::send_line,
            claude::stop_session,
            claude::claude_version,
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
            keys::terminal_keys,
            log_js_error,
            pty::pty_spawn,
            pty::pty_write,
            pty::pty_resize,
            pty::pty_kill,
            updates::install_kind,
            setup::claude_status,
            setup::install_claude,
            setup::login_claude,
            setup::setup_input,
            setup::setup_cancel,
            updates::install_linux_package,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

/// En macOS el menú por defecto de Tauri asigna Cmd+W a "Close Window", y
/// ese atajo tiene que llegar a la página (cerrar panel, como en iTerm2).
/// Este es el mismo menú sin ese ítem. En Linux y Windows no hay menú.
#[cfg(target_os = "macos")]
fn app_menu<R: tauri::Runtime>(app: &tauri::AppHandle<R>) -> tauri::Result<tauri::menu::Menu<R>> {
    use tauri::menu::{Menu, PredefinedMenuItem as Item, Submenu};
    let name = app.package_info().name.clone();
    Menu::with_items(
        app,
        &[
            &Submenu::with_items(
                app,
                name,
                true,
                &[
                    &Item::about(app, None, None)?,
                    &Item::separator(app)?,
                    &Item::services(app, None)?,
                    &Item::separator(app)?,
                    &Item::hide(app, None)?,
                    &Item::hide_others(app, None)?,
                    &Item::separator(app)?,
                    &Item::quit(app, None)?,
                ],
            )?,
            &Submenu::with_items(
                app,
                "Edit",
                true,
                &[
                    &Item::undo(app, None)?,
                    &Item::redo(app, None)?,
                    &Item::separator(app)?,
                    &Item::cut(app, None)?,
                    &Item::copy(app, None)?,
                    &Item::paste(app, None)?,
                    &Item::select_all(app, None)?,
                ],
            )?,
            &Submenu::with_items(app, "View", true, &[&Item::fullscreen(app, None)?])?,
            &Submenu::with_items(app, "Window", true, &[&Item::minimize(app, None)?, &Item::maximize(app, None)?])?,
        ],
    )
}
