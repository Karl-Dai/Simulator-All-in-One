mod commands;
mod state;

use state::AppState;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::default().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::start,
            commands::stop,
            commands::set_point,
            commands::snapshot,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
