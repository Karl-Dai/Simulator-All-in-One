//! Presets for the bundled application live outside the installation directory.
use std::path::PathBuf;
use tauri::Manager;

fn preset_path(root: PathBuf) -> Result<PathBuf, String> {
    if !root.is_absolute() {
        return Err("SIMLAB_DATA_DIR must be an absolute path".into());
    }
    Ok(root.join("pmu-sub").join("presets.json"))
}

#[tauri::command]
pub fn preset_store_path(app: tauri::AppHandle) -> Result<String, String> {
    let root = match std::env::var_os("SIMLAB_DATA_DIR") {
        Some(path) => PathBuf::from(path),
        None => app
            .path()
            .config_dir()
            .map_err(|e| e.to_string())?
            .join("SimLab"),
    };
    let path = preset_path(root)?;
    std::fs::create_dir_all(path.parent().expect("preset parent")).map_err(|e| e.to_string())?;
    Ok(path.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn presets_are_role_scoped_and_never_relative_to_the_executable() {
        let root = std::env::temp_dir().join("simlab-test");
        assert_eq!(
            preset_path(root.clone()).unwrap(),
            root.join("pmu-sub/presets.json")
        );
        assert!(preset_path(PathBuf::from("relative")).is_err());
    }
}
