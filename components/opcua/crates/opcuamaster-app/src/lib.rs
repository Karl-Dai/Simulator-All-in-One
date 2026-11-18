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
            commands::ping,
            commands::create_connection,
            commands::rename_connection,
            commands::connect,
            commands::disconnect,
            commands::delete_connection,
            commands::list_connections,
            commands::discover_endpoints,
            commands::browse_root,
            commands::browse_node,
            commands::collect_variables,
            commands::read_attributes,
            commands::write_value,
            commands::add_monitored_nodes,
            commands::add_variables_under_node,
            commands::remove_monitored_nodes,
            commands::get_monitored_nodes_since,
            commands::get_polling_nodes,
            commands::read_history,
            commands::subscribe_events,
            commands::unsubscribe_events,
            commands::clear_events,
            commands::get_events,
            commands::read_method_arguments,
            commands::call_method,
            commands::list_certificates,
            commands::move_certificate,
            commands::delete_certificate,
            commands::create_group,
            commands::delete_group,
            commands::add_to_group,
            commands::list_groups,
            commands::save_project,
            commands::load_project,
            commands::get_persistence_status,
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_communication_logs,
            #[cfg(not(feature = "simlab-suite"))]
            update::check_for_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::can_install_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::install_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::skip_update,
            #[cfg(not(feature = "simlab-suite"))]
            update::schedule_update_on_next_launch,
        ])
        .setup(|app| {
            let state = app.state::<AppState>();
            #[cfg(feature = "simlab-suite")]
            let data_dir =
                opcuasim_core::suite::role_dir("opcua-master").map_err(|e| e.to_string());
            #[cfg(not(feature = "simlab-suite"))]
            let data_dir = app.path().app_data_dir().map_err(|e| e.to_string());
            match data_dir {
                Ok(dir) => {
                    #[cfg(feature = "simlab-suite")]
                    let restore = state.initialize_suite_persistence(
                        dir.parent().ok_or("Missing SimLab data root")?,
                        &dir.join("last-session.opcuaproj"),
                    );
                    #[cfg(not(feature = "simlab-suite"))]
                    let restore = state.initialize_persistence(dir.join("last-session.opcuaproj"));
                    if let Err(error) = restore {
                        log::warn!("automatic project restore failed: {error}");
                    }
                }
                Err(error) => state.report_persistence_error(error.to_string()),
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
