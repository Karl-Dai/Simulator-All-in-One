mod commands;
#[cfg(feature = "simlab-suite")]
use saio_shared::config_close;
#[cfg(feature = "simlab-suite")]
use saio_shared::config_store;
#[cfg(feature = "simlab-suite")]
mod suite_storage;

pub mod reconnect;
use saio_shared::simlab_preferences;
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
            simlab_preferences::simlab_appearance,
            tls_file_dialog::choose_tls_file,
            // Connection commands
            commands::simlab_load_preferences,
            commands::simlab_save_preferences,
            commands::create_connection,
            commands::replace_connection,
            commands::connect_master,
            commands::disconnect_master,
            commands::delete_connection,
            commands::list_connections,
            // IEC 104 commands
            commands::send_interrogation,
            commands::send_interrogation_deactivation,
            commands::send_clock_sync,
            commands::send_counter_read,
            commands::send_counter_read_deactivation,
            commands::send_broadcast_gi,
            commands::send_broadcast_clock_sync,
            commands::send_broadcast_counter_read,
            commands::send_broadcast_gi_deactivation,
            commands::send_broadcast_counter_read_deactivation,
            commands::send_control_command,
            // Data commands
            commands::get_received_data,
            commands::get_received_data_since,
            // Log commands
            commands::get_communication_logs,
            commands::clear_communication_logs,
            commands::export_logs_csv,
            commands::save_logs_csv,
            commands::set_logging_enabled,
            // Config file save/open
            commands::save_config,
            commands::load_config,
            // Tool commands
            commands::parse_hex,
            commands::parse_frame_full,
            // Update commands
            update::check_for_update,
            update::install_update,
            update::skip_update,
            update::schedule_update_on_next_launch,
        ])
        .setup(|app| {
            #[cfg(feature = "simlab-suite")]
            suite::initialize(app, "iec104-master", "cloud.carldai.simlab.iec104.master")?;
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
                    #[cfg(feature = "simlab-suite")]
                    app_handle
                        .state::<AppState>()
                        .suite_storage
                        .block(error.clone());
                    log::warn!("failed to restore persisted master workspace: {error}");
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
