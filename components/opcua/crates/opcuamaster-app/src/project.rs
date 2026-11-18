use std::collections::HashMap;
use std::fs;
#[cfg(not(feature = "simlab-suite"))]
use std::fs::OpenOptions;
#[cfg(not(feature = "simlab-suite"))]
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use opcuasim_core::client::OpcUaConnection;
use opcuasim_core::config::{
    ConnectionConfig, ConnectionProjectEntry, MonitoredNodeConfig, ProjectFile,
};
use opcuasim_core::node::{AccessMode, MonitoredNode};
use opcuasim_core::polling::PollingManager;
use opcuasim_core::subscription::SubscriptionManager;
use serde::Serialize;
use uuid::Uuid;

use crate::state::{AppState, ConnectionEntry};

#[derive(Default)]
pub struct Persistence {
    path: Option<PathBuf>,
    error: Option<String>,
    // Never overwrite a failed restore unless its original bytes were backed up.
    blocked: bool,
}

#[derive(Serialize)]
pub struct PersistenceStatus {
    pub enabled: bool,
    pub error: Option<String>,
}

pub fn snapshot(state: &AppState) -> Result<ProjectFile, String> {
    let conns = state.connections.read().map_err(|e| e.to_string())?;
    let groups = state.groups.read().map_err(|e| e.to_string())?;
    let mut project = ProjectFile::new_master();
    project.connections = conns
        .values()
        .map(|entry| {
            let c = &entry.connection.config;
            ConnectionProjectEntry {
                id: c.id.clone(),
                name: entry.name.clone(),
                endpoint_url: c.endpoint_url.clone(),
                security_policy: c.security_policy.clone(),
                security_mode: c.security_mode.clone(),
                auth: c.auth.clone(),
                timeout_ms: c.timeout_ms,
                monitored_nodes: entry
                    .pending_subscriptions
                    .iter()
                    .chain(&entry.pending_polling)
                    .map(|node| MonitoredNodeConfig {
                        node_id: node.node_id.clone(),
                        display_name: node.display_name.clone(),
                        access_mode: node.access_mode.clone(),
                        group_id: node.group_id.clone(),
                        data_type: node.data_type.clone(),
                        browse_path: node.browse_path.clone(),
                        filter: node.filter,
                    })
                    .collect(),
            }
        })
        .collect();
    project.connections.sort_by(|a, b| a.id.cmp(&b.id));
    project.groups = groups.clone();
    Ok(project)
}

pub fn read(path: &Path) -> Result<ProjectFile, String> {
    #[cfg(feature = "simlab-suite")]
    let project: ProjectFile = serde_json::from_value(crate::config_store::read_project(
        path,
        "opcua.master",
        "workspace",
    )?)
    .map_err(|e| e.to_string())?;
    #[cfg(not(feature = "simlab-suite"))]
    let project = ProjectFile::from_json(&fs::read_to_string(path).map_err(|e| e.to_string())?)
        .map_err(|e| e.to_string())?;
    validate(&project)?;
    Ok(project)
}

pub fn validate(project: &ProjectFile) -> Result<(), String> {
    #[cfg(feature = "simlab-suite")]
    {
        if project.version != "0.1.0" { return Err(format!("Unsupported OPC UA master project version: {}", project.version)); }
        let _: ProjectFile = serde_json::from_value(serde_json::to_value(project).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    }
    if project.project_type != "OpcUaMaster" {
        return Err("Not an OPC UA master project".into());
    }
    for conn in &project.connections {
        for node in &conn.monitored_nodes {
            let valid_interval = match node.access_mode {
                AccessMode::Subscription { interval_ms } => {
                    interval_ms.is_finite() && interval_ms > 0.0
                }
                AccessMode::Polling { interval_ms } => interval_ms > 0,
            };
            if !valid_interval {
                return Err(format!("Invalid monitoring interval for {}", node.node_id));
            }
        }
    }
    Ok(())
}

/// Called only at startup or after the old runtimes have been stopped.
pub fn apply(state: &AppState, project: ProjectFile) -> Result<(), String> {
    let mut restored = HashMap::new();
    for ce in project.connections {
        let id = if ce.id.is_empty() || restored.contains_key(&ce.id) {
            Uuid::new_v4().to_string()
        } else {
            ce.id
        };
        let config = ConnectionConfig {
            id: id.clone(),
            name: ce.name.clone(),
            endpoint_url: ce.endpoint_url,
            security_policy: ce.security_policy,
            security_mode: ce.security_mode,
            auth: ce.auth,
            timeout_ms: ce.timeout_ms,
        };
        let connection = Arc::new(OpcUaConnection::new(config));
        let polling_mgr = Arc::new(PollingManager::new(connection.get_session_holder()));
        let nodes = ce.monitored_nodes.into_iter().map(|saved| {
            let mut node = MonitoredNode::new(
                saved.node_id,
                saved.display_name,
                saved.browse_path,
                if saved.data_type.is_empty() {
                    "Unknown".into()
                } else {
                    saved.data_type
                },
            );
            node.access_mode = saved.access_mode;
            node.group_id = saved.group_id;
            node.filter = saved.filter;
            node
        });
        let (pending_subscriptions, pending_polling) =
            nodes.partition(|n| matches!(n.access_mode, AccessMode::Subscription { .. }));
        restored.insert(
            id,
            ConnectionEntry {
                name: ce.name,
                connection,
                subscription_mgr: SubscriptionManager::new(),
                polling_mgr,
                pending_subscriptions,
                pending_polling,
            },
        );
    }
    let mut conns = state.connections.write().map_err(|e| e.to_string())?;
    let mut groups = state.groups.write().map_err(|e| e.to_string())?;
    *conns = restored;
    *groups = project.groups;
    Ok(())
}

/// Write beside the destination and rename only after all bytes reach disk.
pub fn write_atomic(path: &Path, project: &ProjectFile) -> Result<(), String> {
    #[cfg(feature = "simlab-suite")]
    { std::fs::create_dir_all(path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."))).map_err(|e| e.to_string())?; }
    #[cfg(feature = "simlab-suite")]
    return crate::config_store::export_project(
        path,
        "opcua.master",
        "workspace",
        &serde_json::to_value(project).map_err(|e| e.to_string())?,
    );
    #[cfg(not(feature = "simlab-suite"))]
    {
        let json = project.to_json().map_err(|e| e.to_string())?;
        let parent = path
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        let tmp = parent.join(format!(".opcuamaster-{}.tmp", Uuid::new_v4()));
        let result = (|| -> std::io::Result<()> {
            let mut options = OpenOptions::new();
            options.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            let mut file = options.open(&tmp)?;
            file.write_all(json.as_bytes())?;
            file.sync_all()?;
            drop(file);
            fs::rename(&tmp, path)
        })();
        if result.is_err() {
            let _ = fs::remove_file(&tmp);
        }
        result.map_err(|e| e.to_string())
    }
}

impl AppState {
    #[cfg(feature = "simlab-suite")]
    pub fn initialize_suite_persistence(&self, root: &Path, legacy: &Path) -> Result<(), String> {
        let result = (|| {
            self.suite_storage.initialize(root, "opcua.master")?;
            let stored = self.suite_storage.load("workspace")?;
            let value = if stored.is_some() {
                stored
            } else {
                let data = match fs::read(legacy) {
                    Ok(data) => {
                        let project = ProjectFile::from_json(
                            std::str::from_utf8(&data).map_err(|e| e.to_string())?,
                        )
                        .map_err(|e| e.to_string())?;
                        validate(&project)?;
                        Some(data)
                    }
                    Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
                    Err(e) => return Err(e.to_string()),
                };
                self.suite_storage.migrate(
                    "workspace",
                    data.as_deref(),
                    &legacy.to_string_lossy(),
                )?
            };
            if let Some(value) = value {
                let project: ProjectFile =
                    serde_json::from_value(value).map_err(|e| e.to_string())?;
                validate(&project)?;
                apply(self, project)?;
            }
            Ok(())
        })();
        if let Err(error) = &result {
            self.suite_storage.block(error.clone());
        }
        result
    }

    #[cfg(any(not(feature = "simlab-suite"), test))]
    pub fn initialize_persistence(&self, path: PathBuf) -> Result<(), String> {
        let mut persistence = self.persistence.lock().map_err(|e| e.to_string())?;
        persistence.path = Some(path.clone());
        let result = match fs::metadata(&path) {
            Ok(_) => read(&path).and_then(|project| apply(self, project)),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(()),
            Err(e) => Err(e.to_string()),
        };
        if let Err(error) = &result {
            let backup =
                path.with_file_name(format!("last-session.corrupt-{}.opcuaproj", Uuid::new_v4()));
            persistence.blocked = fs::copy(&path, &backup).is_err();
            persistence.error = Some(format!("Automatic restore failed: {error}"));
        }
        result
    }

    pub fn autosave(&self) -> Result<(), String> {
        #[cfg(feature = "simlab-suite")]
        {
            #[cfg(test)]
            if self
                .persistence
                .lock()
                .map_err(|e| e.to_string())?
                .path
                .is_some()
            {
                return self.autosave_legacy();
            }
            let project = snapshot(self)?;
            validate(&project)?;
            return self.suite_storage.save("workspace", &serde_json::to_value(project).map_err(|e| e.to_string())?);
        }
        #[cfg(not(feature = "simlab-suite"))]
        self.autosave_legacy()
    }

    #[cfg(any(not(feature = "simlab-suite"), test))]
    fn autosave_legacy(&self) -> Result<(), String> {
        // Serializes writes and takes each snapshot under the same lock, so an
        // older concurrent mutation cannot overwrite a newer snapshot.
        let mut persistence = self.persistence.lock().map_err(|e| e.to_string())?;
        let Some(path) = persistence.path.as_ref() else {
            return Ok(());
        };
        if persistence.blocked {
            return Err(
                "Automatic save disabled because the failed restore could not be backed up".into(),
            );
        }
        let result = snapshot(self).and_then(|project| write_atomic(path, &project));
        persistence.error = result.as_ref().err().cloned();
        result.map_err(|e| format!("Automatic save failed: {e}"))
    }

    pub fn persistence_status(&self) -> Result<PersistenceStatus, String> {
        #[cfg(feature = "simlab-suite")]
        {
            #[cfg(test)]
            if self
                .persistence
                .lock()
                .map_err(|e| e.to_string())?
                .path
                .is_some()
            {
                let p = self.persistence.lock().map_err(|e| e.to_string())?;
                return Ok(PersistenceStatus {
                    enabled: !p.blocked,
                    error: p.error.clone(),
                });
            }
            let status = self.suite_storage.status()?;
            return Ok(PersistenceStatus {
                enabled: status.enabled,
                error: status.error,
            });
        }
        #[cfg(not(feature = "simlab-suite"))]
        {
            let p = self.persistence.lock().map_err(|e| e.to_string())?;
            Ok(PersistenceStatus {
                enabled: p.path.is_some() && !p.blocked,
                error: p.error.clone(),
            })
        }
    }

    pub fn report_persistence_error(&self, error: String) {
        #[cfg(feature = "simlab-suite")]
        self.suite_storage.block(error.clone());
        if let Ok(mut p) = self.persistence.lock() {
            p.error = Some(error);
        }
    }
}

/// Retains the exact runtime handles while a configuration commit is pending.
pub struct ConfigMutation<'a> {
    state: &'a AppState,
    _guard: std::sync::MutexGuard<'a, ()>,
    #[cfg(feature = "simlab-suite")]
    connections: Option<HashMap<String, ConnectionEntry>>,
    #[cfg(feature = "simlab-suite")]
    groups: Option<Vec<opcuasim_core::node::NodeGroup>>,
}
impl AppState {
    pub fn begin_config_mutation(&self) -> Result<ConfigMutation<'_>, String> {
        let guard = self.config_mutation.lock().map_err(|e| e.to_string())?;
        #[cfg(feature = "simlab-suite")]
        self.suite_storage.ensure_writable()?;
        Ok(ConfigMutation {
            state: self,
            _guard: guard,
            #[cfg(feature = "simlab-suite")]
            connections: Some(self.connections.read().map_err(|e| e.to_string())?.clone()),
            #[cfg(feature = "simlab-suite")]
            groups: Some(self.groups.read().map_err(|e| e.to_string())?.clone()),
        })
    }
}
impl ConfigMutation<'_> {
    pub fn commit(mut self) -> Result<(), String> {
        self.state.autosave()?;
        #[cfg(feature = "simlab-suite")]
        {
            self.connections = None;
            self.groups = None;
        }
        Ok(())
    }
}
#[cfg(feature = "simlab-suite")]
impl Drop for ConfigMutation<'_> {
    fn drop(&mut self) {
        if let Some(connections) = self.connections.take() {
            if let Ok(mut current) = self.state.connections.write() {
                *current = connections;
            }
        }
        if let Some(groups) = self.groups.take() {
            if let Ok(mut current) = self.state.groups.write() {
                *current = groups;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn test_dir() -> PathBuf {
        let dir = std::env::temp_dir().join(format!("opcua-project-{}", Uuid::new_v4()));
        fs::create_dir_all(&dir).unwrap();
        dir
    }

    #[test]
    fn legacy_project_restores_without_new_fields() {
        let dir = test_dir();
        let path = dir.join("legacy.opcuaproj");
        fs::write(&path, r#"{"type":"OpcUaMaster","version":"0.1.0","connections":[{"name":"Legacy","endpoint_url":"opc.tcp://localhost:4840","security_policy":"None","security_mode":"None","auth":"Anonymous","timeout_ms":5000,"monitored_nodes":[{"node_id":"ns=2;s=Temperature","display_name":"Temperature","access_mode":{"Subscription":{"interval_ms":1000}},"group_id":null}]}],"groups":[]}"#).unwrap();
        let state = AppState::new();
        state.initialize_persistence(path).unwrap();
        let conns = state.connections.read().unwrap();
        let entry = conns.values().next().unwrap();
        assert!(!entry.connection.config.id.is_empty());
        assert_eq!(entry.pending_subscriptions.len(), 1);
        assert_eq!(entry.pending_subscriptions[0].data_type, "Unknown");
        assert!(entry.pending_subscriptions[0].filter.is_none());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn corrupted_restore_is_reported_and_original_is_backed_up() {
        let dir = test_dir();
        let path = dir.join("last-session.opcuaproj");
        fs::write(&path, "{broken").unwrap();
        let state = AppState::new();
        assert!(state.initialize_persistence(path.clone()).is_err());
        assert!(state.persistence_status().unwrap().error.is_some());
        assert_eq!(fs::read_to_string(&path).unwrap(), "{broken");
        let backup = fs::read_dir(&dir)
            .unwrap()
            .map(|e| e.unwrap().path())
            .find(|p| {
                p.file_name()
                    .unwrap()
                    .to_string_lossy()
                    .starts_with("last-session.corrupt-")
            })
            .unwrap();
        assert_eq!(fs::read_to_string(backup).unwrap(), "{broken");
        state.autosave().unwrap();
        assert!(read(&path).is_ok());
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn failed_atomic_replace_leaves_existing_destination_and_reports_error() {
        let dir = test_dir();
        let path = dir.join("destination");
        fs::create_dir(&path).unwrap();
        fs::write(path.join("keep"), "original").unwrap();
        let state = AppState::new();
        assert!(state.initialize_persistence(path.clone()).is_err());
        assert!(state.autosave().is_err());
        assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "original");
        assert!(fs::read_dir(&dir).unwrap().all(|e| !e
            .unwrap()
            .file_name()
            .to_string_lossy()
            .ends_with(".tmp")));
        assert!(write_atomic(&path, &ProjectFile::new_master()).is_err());
        assert_eq!(fs::read_to_string(path.join("keep")).unwrap(), "original");
        fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn successful_atomic_writes_are_private_and_replace_previous_snapshot() {
        let dir = test_dir();
        let path = dir.join("last-session.opcuaproj");
        write_atomic(&path, &ProjectFile::new_master()).unwrap();
        write_atomic(&path, &ProjectFile::new_master()).unwrap();
        assert!(read(&path).is_ok());
        assert_eq!(fs::read_dir(&dir).unwrap().count(), 1);
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                fs::metadata(&path).unwrap().permissions().mode() & 0o777,
                0o600
            );
        }
        fs::remove_dir_all(dir).unwrap();
    }
}
