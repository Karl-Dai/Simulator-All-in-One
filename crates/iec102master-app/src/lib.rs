mod commands;
#[path = "../../shared/config_close.rs"]
mod config_close;
#[path = "../../shared/config_store.rs"]
mod config_store;
#[path = "../../shared/simlab_preferences.rs"]
mod simlab_preferences;
mod state;
mod suite_storage;
#[path = "../../shared/suite.rs"]
mod suite;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    let builder = config_close::protect(builder);
    builder
        .plugin(tauri_plugin_log::Builder::default().build())
        .manage(AppState::new())
        .invoke_handler(tauri::generate_handler![
            commands::connect,
            commands::disconnect,
            commands::interrogate,
            commands::snapshot,
            config_close::simlab_enable_configuration_close,
            config_close::simlab_confirm_configuration_close,
            simlab_preferences::simlab_theme,
            simlab_preferences::simlab_appearance,
            commands::simlab_load_preferences,
            commands::simlab_save_preferences,
        ])
        .setup(|app| {
            let role = "iec102.master";
            let root = suite::initialize(app, role, "cloud.carldai.simlab.iec102.master")?;
            app.state::<AppState>().suite_storage.initialize(&root, role)?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("error while building tauri application")
        .run(config_close::on_run_event);
}
