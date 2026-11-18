mod commands;
#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_close.rs"]
mod config_close;
#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_store.rs"]
mod config_store;
#[cfg(feature = "simlab-suite")]
mod suite_storage;

mod point_csv;
mod point_event_json;
#[path = "../../shared/simlab_preferences.rs"]
mod simlab_preferences;
mod state;
#[path = "../../shared/suite.rs"]
mod suite;
#[path = "../../shared/tls_file_dialog.rs"]
mod tls_file_dialog;
pub mod update;

use state::AppState;
use tauri::Manager;

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default();
    #[cfg(not(feature = "simlab-suite"))]
    let builder = builder.plugin(tauri_plugin_updater::Builder::new().build());
    #[cfg(feature = "simlab-suite")]
    let builder = config_close::protect(builder);
    let builder = builder
        .plugin(tauri_plugin_process::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .manage(AppState::new())
        .manage(update::UpdateState::default())
        .invoke_handler(tauri::generate_handler![
            #[cfg(feature = "simlab-suite")]
            config_close::simlab_enable_configuration_close,
            #[cfg(feature = "simlab-suite")]
            config_close::simlab_confirm_configuration_close,
            simlab_preferences::simlab_theme,
            tls_file_dialog::choose_tls_file,
            // Server commands
            #[cfg(feature = "simlab-suite")]
            commands::simlab_load_preferences,
            #[cfg(feature = "simlab-suite")]
            commands::simlab_save_preferences,
            #[cfg(feature = "simlab-suite")]
            commands::get_persistence_status,
            commands::create_server,
            commands::create_and_start_server,
            commands::start_server,
            commands::stop_server,
            commands::delete_server,
            commands::list_servers,
            commands::list_client_connections,
            commands::update_server_transport,
            commands::get_server_transport,
            commands::list_bind_address_suggestions,
            // Station commands
            commands::add_station,
            commands::update_station,
            commands::remove_station,
            commands::list_stations,
            // Data point commands
            commands::add_data_point,
            commands::update_data_point_definition,
            commands::list_control_mapping_targets,
            commands::batch_add_data_points,
            commands::remove_data_point,
            commands::batch_remove_data_points,
            commands::batch_migrate_data_point_types,
            commands::batch_update_control_options,
            commands::update_data_point,
            commands::set_data_point_quality,
            commands::batch_set_data_point_quality,
            commands::batch_update_data_points,
            commands::list_data_points,
            commands::list_data_points_since,
            commands::get_data_point,
            commands::get_data_point_values,
            // Log commands
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_logs_csv,
            commands::save_logs_csv,
            // Simulation commands
            commands::random_mutate_data_points,
            commands::set_cyclic_config,
            // Remote operation configuration (远动运行参数)
            commands::set_protocol_timing,
            commands::get_protocol_timing,
            commands::set_remote_operation_config,
            commands::get_remote_operation_config,
            commands::set_simulation_pacing,
            commands::start_point_mutation,
            commands::stop_point_mutation,
            commands::list_point_mutations,
            // Imported point-event schedules
            point_event_json::inspect_point_event_schedule,
            point_event_json::start_point_event_schedule,
            point_event_json::save_point_event_schedule_example,
            // Config file save/open
            commands::save_config,
            commands::load_config,
            // Per-station point CSV import/export (JSON config remains separate)
            point_csv::save_point_config_csv,
            point_csv::import_point_config_csv,
            point_csv::save_point_config_csv_template,
            // Tool commands
            commands::parse_hex,
            commands::parse_apci,
            commands::parse_frame_full,
            // Update commands
            update::check_for_update,
            update::install_update,
            update::skip_update,
            update::schedule_update_on_next_launch,
        ])
        .setup(|app| {
            #[cfg(feature = "simlab-suite")]
            suite::initialize(app, "iec104-slave", "cloud.carldai.simlab.iec104.slave")?;
            if cfg!(debug_assertions) {
                app.handle().plugin(
                    tauri_plugin_log::Builder::default()
                        .level(log::LevelFilter::Info)
                        .build(),
                )?;
            }
            let app_handle = app.handle().clone();
            tauri::async_runtime::spawn(async move {
                if let Err(error) = commands::restore_persisted_workspace(app_handle.clone()).await
                {
                    log::warn!("failed to restore persisted slave workspace: {error}");
                }
                app_handle.state::<AppState>().mark_workspace_ready();
                #[cfg(not(feature = "simlab-suite"))]
                if let Err(error) = update::install_pending_update(app_handle).await {
                    log::warn!("automatic update on launch failed: {error}");
                }
            });
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
