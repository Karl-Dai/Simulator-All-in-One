mod commands;
#[cfg(feature = "simlab-suite")]
use saio_shared::config_close;
#[cfg(feature = "simlab-suite")]
use saio_shared::config_store;
#[cfg(feature = "simlab-suite")]
mod suite_storage;

mod project;
use saio_shared::simlab_preferences;
mod state;
#[cfg(not(feature = "simlab-suite"))]
pub mod update;

use state::AppState;
#[cfg(feature = "simlab-suite")]
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new());
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
            commands::simlab_load_preferences,
            #[cfg(feature = "simlab-suite")]
            commands::simlab_save_preferences,
            #[cfg(feature = "simlab-suite")]
            commands::get_persistence_status,
            commands::ping,
            commands::start_server,
            commands::stop_server,
            commands::refresh_status,
            commands::refresh_address_space,
            commands::get_config,
            commands::update_config,
            commands::add_folder,
            commands::add_node,
            commands::remove_node,
            commands::update_node,
            commands::save_project,
            commands::load_project,
            commands::get_simulation_values_since,
            #[cfg(not(feature = "simlab-suite"))]
            update::check_for_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::install_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::skip_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::schedule_update_on_next_launch,
        ])
        .setup(|app| {
            #[cfg(feature = "simlab-suite")]
            {
                let state = app.state::<AppState>();
                let restore = opcuasim_core::suite::role_dir("opcua-server")
                    .map_err(|e| e.to_string())
                    .and_then(|dir| {
                        project::initialize(&state, dir.parent().ok_or("Missing SimLab data root")?)
                    });
                if let Err(error) = restore {
                    state.suite_storage.block(error.clone());
                    log::error!("automatic restore failed: {error}");
                }
            }
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            #[cfg(not(feature = "simlab-suite"))]
            {
                let app_handle = app.handle().clone();
                tauri::async_runtime::spawn(async move {
                    if let Err(error) = update::install_pending_update(app_handle).await {
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
