use crate::state::AppState;
use opcuasim_core::server::models::ServerProjectFile;
use std::path::Path;

pub fn snapshot(state: &AppState) -> Result<ServerProjectFile, String> {
    Ok(ServerProjectFile {
        server_config: state.config.read().map_err(|e| e.to_string())?.clone(),
        folders: state.folders.read().map_err(|e| e.to_string())?.clone(),
        nodes: state.nodes.read().map_err(|e| e.to_string())?.clone(),
        ..ServerProjectFile::default()
    })
}

fn validate(project: &ServerProjectFile) -> Result<(), String> {
    if project.project_type != "OpcUaServer" || project.version != "0.1.0" {
        return Err("Not a supported OPC UA server project".into());
    }
    // A round trip rejects non-finite configuration fields before persistence.
    let _: ServerProjectFile =
        serde_json::from_value(serde_json::to_value(project).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    Ok(())
}

fn apply(state: &AppState, project: ServerProjectFile) -> Result<(), String> {
    let mut config = state.config.write().map_err(|e| e.to_string())?;
    let mut folders = state.folders.write().map_err(|e| e.to_string())?;
    let mut nodes = state.nodes.write().map_err(|e| e.to_string())?;
    *config = project.server_config;
    *folders = project.folders;
    *nodes = project.nodes;
    Ok(())
}

pub fn mutate<T>(
    state: &AppState,
    edit: impl FnOnce(&mut ServerProjectFile) -> Result<T, String>,
) -> Result<T, String> {
    let _guard = state.config_mutation.lock().map_err(|e| e.to_string())?;
    #[cfg(feature = "simlab-suite")]
    state.suite_storage.ensure_writable()?;
    let mut candidate = snapshot(state)?;
    let result = edit(&mut candidate)?;
    validate(&candidate)?;
    #[cfg(feature = "simlab-suite")]
    state.suite_storage.save(
        "workspace",
        &serde_json::to_value(&candidate).map_err(|e| e.to_string())?,
    )?;
    apply(state, candidate)?;
    Ok(result)
}

#[cfg(feature = "simlab-suite")]
pub fn initialize(state: &AppState, root: &Path) -> Result<(), String> {
    let result: Result<(), String> = (|| {
        state.suite_storage.initialize(root, "opcua.server")?;
        if let Some(value) = state.suite_storage.load("workspace")? {
            let project: ServerProjectFile =
                serde_json::from_value(value).map_err(|e| e.to_string())?;
            validate(&project)?;
            apply(state, project)?;
        }
        Ok(())
    })();
    if let Err(error) = &result {
        state.suite_storage.block(error.clone());
    }
    result
}

pub fn save(state: &AppState, path: &Path) -> Result<(), String> {
    let _guard = state.config_mutation.lock().map_err(|e| e.to_string())?;
    let project = snapshot(state)?;
    #[cfg(feature = "simlab-suite")]
    return crate::config_store::export_project(
        path,
        "opcua.server",
        "workspace",
        &serde_json::to_value(&project).map_err(|e| e.to_string())?,
    );
    #[cfg(not(feature = "simlab-suite"))]
    std::fs::write(
        path,
        serde_json::to_string_pretty(&project).map_err(|e| e.to_string())?,
    )
    .map_err(|e| e.to_string())
}

pub fn load(state: &AppState, path: &Path) -> Result<(), String> {
    #[cfg(feature = "simlab-suite")]
    let project: ServerProjectFile = serde_json::from_value(crate::config_store::read_project(
        path,
        "opcua.server",
        "workspace",
    )?)
    .map_err(|e| e.to_string())?;
    #[cfg(not(feature = "simlab-suite"))]
    let project: ServerProjectFile =
        serde_json::from_slice(&std::fs::read(path).map_err(|e| e.to_string())?)
            .map_err(|e| e.to_string())?;
    validate(&project)?;
    mutate(state, |candidate| {
        *candidate = project;
        Ok(())
    })
}

#[cfg(all(test, feature = "simlab-suite"))]
mod tests {
    use super::*;
    use crate::commands::{
        add_folder_impl, add_node_impl, refresh_status_impl, AddFolderRequest, AddNodeRequest,
    };
    use opcuasim_core::server::models::{DataType, SimulationMode};
    #[tokio::test]
    async fn workspace_autosaves_definitions_and_restores_stopped() {
        let root = tempfile::tempdir().unwrap();
        let state = AppState::new();
        initialize(&state, root.path()).unwrap();
        add_folder_impl(
            &state,
            AddFolderRequest {
                node_id: "Folder".into(),
                display_name: "Saved folder".into(),
                parent_id: "Objects".into(),
            },
        )
        .await
        .unwrap();
        add_node_impl(
            &state,
            AddNodeRequest {
                node_id: "Value".into(),
                display_name: "Saved value".into(),
                parent_id: "Folder".into(),
                data_type: DataType::Double,
                writable: true,
                simulation: SimulationMode::Static {
                    value: "123".into(),
                },
                eu_range_low: 0.0,
                eu_range_high: 100.0,
            },
        )
        .await
        .unwrap();
        mutate(&state, |p| {
            p.server_config.endpoint_url = "opc.tcp://127.0.0.1:4859".into();
            Ok(())
        })
        .unwrap();
        let restored = AppState::new();
        initialize(&restored, root.path()).unwrap();
        let status = refresh_status_impl(&restored).await.unwrap();
        assert_eq!(status.state, "Stopped");
        assert_eq!(status.node_count, 1);
        assert_eq!(status.folder_count, 1);
        assert_eq!(status.endpoint_url, "opc.tcp://127.0.0.1:4859");
        mutate(&restored, |p| {
            p.nodes.clear();
            p.folders.clear();
            Ok(())
        })
        .unwrap();
        let empty = AppState::new();
        initialize(&empty, root.path()).unwrap();
        assert!(snapshot(&empty).unwrap().nodes.is_empty());
    }
    #[test]
    fn stale_project_change_is_not_published_and_export_import_is_scoped() {
        let root = tempfile::tempdir().unwrap();
        let first = AppState::new();
        initialize(&first, root.path()).unwrap();
        let stale = AppState::new();
        initialize(&stale, root.path()).unwrap();
        mutate(&first, |p| {
            p.server_config.endpoint_url = "opc.tcp://127.0.0.1:4860".into();
            Ok(())
        })
        .unwrap();
        assert!(mutate(&stale, |p| {
            p.server_config.endpoint_url = "unsaved".into();
            Ok(())
        })
        .is_err());
        assert_ne!(
            snapshot(&stale).unwrap().server_config.endpoint_url,
            "unsaved"
        );
        let file = root.path().join("server.saioproj");
        save(&first, &file).unwrap();
        assert!(std::fs::read(&file)
            .unwrap()
            .starts_with(b"SQLite format 3\0"));
        let other = tempfile::tempdir().unwrap();
        let imported = AppState::new();
        initialize(&imported, other.path()).unwrap();
        load(&imported, &file).unwrap();
        assert_eq!(
            snapshot(&imported).unwrap().server_config.endpoint_url,
            "opc.tcp://127.0.0.1:4860"
        );
        assert!(crate::config_store::read_project(&file, "opcua.master", "workspace").is_err());
        let legacy = root.path().join("legacy.opcuaproj");
        std::fs::write(
            &legacy,
            serde_json::to_vec(&snapshot(&first).unwrap()).unwrap(),
        )
        .unwrap();
        load(&imported, &legacy).unwrap();
    }
    #[test]
    fn corrupt_workspace_blocks_changes_without_writing_defaults() {
        let root = tempfile::tempdir().unwrap();
        std::fs::write(root.path().join("saio.db"), b"broken").unwrap();
        let state = AppState::new();
        assert!(initialize(&state, root.path()).is_err());
        assert!(mutate(&state, |p| {
            p.nodes.clear();
            Ok(())
        })
        .is_err());
        assert_eq!(
            std::fs::read(root.path().join("saio.db")).unwrap(),
            b"broken"
        );
    }
}
