#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_close.rs"]
mod config_close;

#[path = "../../shared/simlab_preferences.rs"]
mod simlab_preferences;

#[cfg(feature = "simlab-suite")]
#[path = "../../shared/config_store.rs"]
mod config_store;

#[cfg(feature = "simlab-suite")]
use pmusim_sub::suite;
use pmusim_sub::{commands, state::AppState};

#[cfg(feature = "simlab-suite")]
use pmusim_sub::suite_storage;

fn main() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_log::Builder::new().build())
        .plugin(tauri_plugin_store::Builder::new().build())
        .plugin(tauri_plugin_dialog::init())
        .manage(AppState::default());
    #[cfg(feature = "simlab-suite")]
    let builder = builder.manage(suite_storage::SuiteStorage::new("pmu.sub"));
    #[cfg(feature = "simlab-suite")]
    let builder = config_close::protect(builder);
    let builder = builder.invoke_handler(tauri::generate_handler![
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
        #[cfg(feature = "simlab-suite")]
        suite::preset_store_path,
        commands::start_substation,
        commands::stop_substation,
        commands::update_config,
        commands::update_gen,
        commands::fire_trigger,
        commands::poll_events,
        commands::open_url,
    ]);
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
