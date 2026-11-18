//! Suite configuration documents. No network state is restored by this module.
use serde_json::Value;
use std::collections::HashSet;
use std::{
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};
use tauri::{Manager, State};
#[path = "config_store.rs"]
mod config_store;

#[derive(Clone)]
pub struct SuiteStorage {
    role: &'static str,
    store: Arc<Mutex<Option<config_store::Store>>>,
    loaded: Arc<Mutex<HashSet<String>>>,
}
impl SuiteStorage {
    pub fn new(role: &'static str) -> Self {
        Self {
            role,
            store: Arc::new(Mutex::new(None)),
            loaded: Arc::new(Mutex::new(HashSet::new())),
        }
    }
    fn with_store<T>(
        &self,
        root: &Path,
        operation: impl FnOnce(&mut config_store::Store) -> Result<T, String>,
    ) -> Result<T, String> {
        let mut guard = self
            .store
            .lock()
            .map_err(|_| "Configuration lock poisoned".to_string())?;
        if guard.is_none() {
            *guard = Some(config_store::Store::open(root, self.role)?);
        }
        operation(guard.as_mut().expect("opened store"))
    }
    fn remember_load<T>(&self, key: &str, result: Result<T, String>) -> Result<T, String> {
        let mut loaded = self
            .loaded
            .lock()
            .map_err(|_| "Configuration lock poisoned".to_string())?;
        if result.is_ok() {
            loaded.insert(key.into());
        } else {
            loaded.remove(key);
        }
        result
    }
    fn save(&self, root: &Path, key: &str, value: &Value) -> Result<(), String> {
        if key != "ui" {
            validate_document(self.role, key, value)?;
        }
        if !self
            .loaded
            .lock()
            .map_err(|_| "Configuration lock poisoned".to_string())?
            .contains(key)
        {
            return Err("Configuration must be restored successfully before saving".into());
        }
        self.with_store(root, |store| store.save(key, value))
    }
}
fn root(app: &tauri::AppHandle) -> Result<PathBuf, String> {
    match std::env::var_os("SIMLAB_DATA_DIR") {
        Some(root) => Ok(PathBuf::from(root)),
        None => Ok(app
            .path()
            .config_dir()
            .map_err(|e| e.to_string())?
            .join("SimLab")),
    }
}
fn key_allowed(key: &str) -> Result<(), String> {
    match key {
        "workspace" | "settings" | "presets" => Ok(()),
        _ => Err("Invalid configuration document".into()),
    }
}
fn validate_document(role: &str, key: &str, value: &Value) -> Result<(), String> {
    let object = value
        .as_object()
        .ok_or_else(|| format!("Invalid {key}: expected an object"))?;
    if key == "presets" {
        for value in object.values() {
            validate_document(role, "workspace", value)?;
        }
    } else if key == "settings" {
        for field in ["freq_offset_hz", "rocof_hz_s"] {
            if let Some(value) = object.get(field) {
                if !value.as_f64().is_some_and(f64::is_finite) {
                    return Err(format!("Invalid generator setting {field}"));
                }
            }
        }
    } else if key == "workspace" {
        if let Some(protocol) = object.get("protocol") {
            if !matches!(protocol.as_str(), Some("V2" | "V3")) {
                return Err("Invalid PMU protocol".into());
            }
        }
        if role == "pmu.master" {
            for field in [
                "connIp",
                "connMgmtPort",
                "connDataPort",
                "rateHz",
                "heartbeatSecs",
                "rawPeriod",
            ] {
                if object.get(field).is_some_and(|v| !v.is_string()) {
                    return Err(format!("Invalid PMU setting {field}"));
                }
            }
        } else if let Some(phasors) = object.get("phasors") {
            if !phasors.is_array()
                || phasors.as_array().unwrap().iter().any(|p| {
                    !p["magnitude"].as_f64().is_some_and(f64::is_finite)
                        || !p["phase_deg"].as_f64().is_some_and(f64::is_finite)
                })
            {
                return Err("Invalid PMU phasor definitions".into());
            }
        }
    }
    Ok(())
}

fn load_document(
    storage: &SuiteStorage,
    root: &Path,
    key: &str,
    legacy: Option<Value>,
) -> Result<Option<Value>, String> {
    let result = storage.with_store(root, |store| {
        if let Some(value) = store.load(key)? {
            validate_document(storage.role, key, &value)?;
            return Ok(Some(value));
        }
        let bytes = if key == "presets" && storage.role == "pmu.sub" {
            let path = root.join("pmu-sub/presets.json");
            match std::fs::read(&path) {
                Ok(data) => Some(data),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
                Err(error) => return Err(format!("{}: {error}", path.display())),
            }
        } else {
            legacy
                .map(|value| serde_json::to_vec(&value))
                .transpose()
                .map_err(|e| e.to_string())?
        };
        if let Some(bytes) = bytes.as_deref() {
            let value: Value = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
            validate_document(storage.role, key, &value)?;
        }
        store.migrate_json(key, bytes.as_deref(), "pmu-legacy-v1")
    });
    storage.remember_load(key, result)
}
#[tauri::command]
pub async fn simlab_load_preferences(
    app: tauri::AppHandle,
    storage: State<'_, SuiteStorage>,
    legacy: Value,
) -> Result<Value, String> {
    let root = root(&app)?;
    let storage = storage.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !legacy.is_object() {
            return Err("Preferences must be an object".into());
        }
        let bytes = serde_json::to_vec(&legacy).map_err(|e| e.to_string())?;
        let result = storage.with_store(&root, |store| {
            let value = store
                .migrate_json("ui", Some(&bytes), "webview-localStorage-v1")?
                .unwrap_or_else(|| serde_json::json!({}));
            if !value.is_object() {
                return Err("Invalid saved preferences: expected an object".into());
            }
            Ok(value)
        });
        storage.remember_load("ui", result)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn simlab_save_preferences(
    app: tauri::AppHandle,
    storage: State<'_, SuiteStorage>,
    values: Value,
) -> Result<(), String> {
    let root = root(&app)?;
    let storage = storage.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !values.is_object() {
            return Err("Preferences must be an object".into());
        }
        storage.save(&root, "ui", &values)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn simlab_load_document(
    app: tauri::AppHandle,
    storage: State<'_, SuiteStorage>,
    key: String,
    legacy: Option<Value>,
) -> Result<Option<Value>, String> {
    key_allowed(&key)?;
    let root = root(&app)?;
    let storage = storage.inner().clone();
    tauri::async_runtime::spawn_blocking(move || load_document(&storage, &root, &key, legacy))
        .await
        .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn simlab_save_document(
    app: tauri::AppHandle,
    storage: State<'_, SuiteStorage>,
    key: String,
    value: Value,
) -> Result<(), String> {
    key_allowed(&key)?;
    let root = root(&app)?;
    let storage = storage.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        if !value.is_object() {
            return Err("Configuration must be an object".into());
        }
        storage.save(&root, &key, &value)
    })
    .await
    .map_err(|e| e.to_string())?
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn workspace_restart_and_conflict_keep_prior_revision() {
        let dir = tempfile::tempdir().unwrap();
        let first = SuiteStorage::new("pmu.master");
        let stale = SuiteStorage::new("pmu.master");
        assert!(first.save(dir.path(), "workspace", &json!({})).is_err());
        assert_eq!(
            load_document(&first, dir.path(), "workspace", None).unwrap(),
            None
        );
        assert_eq!(
            load_document(&stale, dir.path(), "workspace", None).unwrap(),
            None
        );
        let config = json!({"protocol":"V3","connIp":"127.0.0.1","heartbeatSecs":"9"});
        first.save(dir.path(), "workspace", &config).unwrap();
        assert!(stale
            .save(dir.path(), "workspace", &json!({"protocol":"V2"}))
            .is_err());
        let restarted = SuiteStorage::new("pmu.master");
        assert_eq!(
            load_document(&restarted, dir.path(), "workspace", None).unwrap(),
            Some(config)
        );
        let other = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&other, dir.path(), "workspace", None).unwrap(),
            None
        );
    }
    #[test]
    fn invalid_phasor_draft_never_replaces_a_restorable_workspace() {
        let dir = tempfile::tempdir().unwrap();
        let store = SuiteStorage::new("pmu.sub");
        load_document(&store, dir.path(), "workspace", None).unwrap();
        let valid = json!({"protocol":"V3","phasors":[{"magnitude":1000.0,"phase_deg":0.0}]});
        store.save(dir.path(), "workspace", &valid).unwrap();
        let invalid = json!({"protocol":"V3","phasors":[{"magnitude":"","phase_deg":0.0}]});
        assert!(store.save(dir.path(), "workspace", &invalid).is_err());
        let restarted = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&restarted, dir.path(), "workspace", None).unwrap(),
            Some(valid)
        );
        let fixed = json!({"protocol":"V3","phasors":[{"magnitude":123.0,"phase_deg":0.0}]});
        store.save(dir.path(), "workspace", &fixed).unwrap();
        let restarted = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&restarted, dir.path(), "workspace", None).unwrap(),
            Some(fixed)
        );
    }
    #[test]
    fn presets_migrate_once_and_empty_never_resurrects_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pmu-sub/presets.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        let presets =
            json!({"Station A":{"protocol":"V3","phasors":[{"magnitude":123.5,"phase_deg":0.0}]}});
        std::fs::write(&path, serde_json::to_vec(&presets).unwrap()).unwrap();
        let store = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&store, dir.path(), "presets", None).unwrap(),
            Some(presets)
        );
        store.save(dir.path(), "presets", &json!({})).unwrap();
        std::fs::write(&path, b"broken legacy source").unwrap();
        let restarted = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&restarted, dir.path(), "presets", None).unwrap(),
            Some(json!({}))
        );
        assert_eq!(std::fs::read(&path).unwrap(), b"broken legacy source");
    }
    #[test]
    fn failed_legacy_restore_blocks_defaults_and_preserves_source() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("pmu-sub/presets.json");
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, b"broken").unwrap();
        let store = SuiteStorage::new("pmu.sub");
        assert!(load_document(&store, dir.path(), "presets", None).is_err());
        assert!(store.save(dir.path(), "presets", &json!({})).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"broken");
    }
}

#[tauri::command]
pub async fn simlab_export_document(
    storage: State<'_, SuiteStorage>,
    path: String,
    key: String,
    value: Value,
) -> Result<(), String> {
    key_allowed(&key)?;
    if Path::new(&path).extension().and_then(|s| s.to_str()) != Some("saioproj") {
        return Err("Export requires a .saioproj file".into());
    }
    if !value.is_object() {
        return Err("Configuration must be an object".into());
    }
    let role = storage.role;
    validate_document(role, &key, &value)?;
    tauri::async_runtime::spawn_blocking(move || {
        config_store::export_project(Path::new(&path), role, &key, &value)
    })
    .await
    .map_err(|e| e.to_string())?
}
#[tauri::command]
pub async fn simlab_import_document(
    app: tauri::AppHandle,
    storage: State<'_, SuiteStorage>,
    path: String,
    key: String,
) -> Result<Value, String> {
    key_allowed(&key)?;
    let root = root(&app)?;
    let storage = storage.inner().clone();
    tauri::async_runtime::spawn_blocking(move || {
        import_document(&storage, &root, Path::new(&path), &key)
    })
    .await
    .map_err(|e| e.to_string())?
}

fn import_document(
    storage: &SuiteStorage,
    root: &Path,
    path: &Path,
    key: &str,
) -> Result<Value, String> {
    let value = config_store::read_project(path, storage.role, key)?;
    validate_document(storage.role, key, &value)?;
    storage.save(root, key, &value)?;
    Ok(value)
}
#[cfg(test)]
mod project_tests {
    use super::*;
    use serde_json::json;
    #[test]
    fn selected_document_projects_import_before_publishing_and_keep_legacy() {
        let dir = tempfile::tempdir().unwrap();
        let store = SuiteStorage::new("pmu.sub");
        load_document(&store, dir.path(), "presets", None).unwrap();
        let value = json!({"Station A":{"protocol":"V3","stn":"saved"}});
        let project = dir.path().join("presets.saioproj");
        config_store::export_project(&project, "pmu.sub", "presets", &value).unwrap();
        assert_eq!(
            import_document(&store, dir.path(), &project, "presets").unwrap(),
            value
        );
        assert!(import_document(&store, dir.path(), &project, "workspace").is_err());
        let legacy = dir.path().join("presets.json");
        let bytes = serde_json::to_vec(&value).unwrap();
        std::fs::write(&legacy, &bytes).unwrap();
        assert_eq!(
            import_document(&store, dir.path(), &legacy, "presets").unwrap(),
            value
        );
        assert_eq!(std::fs::read(&legacy).unwrap(), bytes);
        let restarted = SuiteStorage::new("pmu.sub");
        assert_eq!(
            load_document(&restarted, dir.path(), "presets", None).unwrap(),
            Some(value)
        );
    }
}
