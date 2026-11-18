#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_close.rs"]
mod config_close;

#[path = "../../shared/simlab_preferences.rs"]
mod simlab_preferences;

#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_store.rs"]
mod config_store;

#[cfg(not(feature = "simlab-suite"))]
use pmusim_app::update;
use pmusim_app::{commands, state::AppState};

#[cfg(feature = "simlab-suite")]
use pmusim_app::suite_storage;

fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default());
    #[cfg(feature = "simlab-suite")]
    let builder = builder.manage(suite_storage::SuiteStorage::new("pmu.master"));
    #[cfg(not(feature = "simlab-suite"))]
    let builder = builder
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::UpdateState::default());
    #[cfg(feature = "simlab-suite")]
    let builder = config_close::protect(builder);
    let builder = builder
        .invoke_handler(tauri::generate_handler![
            #[cfg(feature = "simlab-suite")]
            config_close::simlab_enable_configuration_close,
            #[cfg(feature = "simlab-suite")]
            config_close::simlab_confirm_configuration_close,
            simlab_preferences::simlab_theme,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_load_preferences,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_save_preferences,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_load_document,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_save_document,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_export_document,
            #[cfg(feature = "simlab-suite")]
            suite_storage::simlab_import_document,
            commands::start_server,
            commands::stop_server,
            commands::connect_substation,
            commands::disconnect_substation,
            commands::send_command,
            commands::auto_handshake,
            commands::skip_cfg2_open,
            commands::set_heartbeat_interval,
            commands::poll_events,
            commands::open_url,
            #[cfg(not(feature = "simlab-suite"))]
            update::check_for_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::install_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::skip_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::schedule_update_on_next_launch,
        ])
        .setup(|_app| {
            #[cfg(not(feature = "simlab-suite"))]
            {
                let update_app = _app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = update::install_pending_update(update_app).await {
                        log::warn!("automatic update on launch failed: {error}");
                    }
                });
            }
            Ok(())
        });
    #[cfg(feature = "simlab-suite")]
    builder
        .build(tauri::generate_context!())
        .expect("error while building suite application")
        .run(config_close::on_run_event);
    #[cfg(not(feature = "simlab-suite"))]
    builder
        .run(tauri::generate_context!())
        .expect("error while running application");
}
