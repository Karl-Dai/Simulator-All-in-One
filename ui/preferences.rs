/// Read only the suite appearance preference, never expose the full settings.
#[tauri::command]
pub(crate) fn simlab_theme() -> Result<String, String> {
    if !cfg!(feature = "simlab-suite") {
        return Err("Appearance is managed by the standalone application".into());
    }
    let root = std::env::var_os("SIMLAB_DATA_DIR")
        .map(std::path::PathBuf::from)
        .ok_or("SIMLAB_DATA_DIR is not configured")?;
    if !root.is_absolute() {
        return Err("SIMLAB_DATA_DIR must be absolute".into());
    }
    let data = match std::fs::read(root.join("launcher.json")) {
        Ok(data) => data,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok("system".into()),
        Err(error) => return Err(error.to_string()),
    };
    let value: serde_json::Value = serde_json::from_slice(&data).map_err(|e| e.to_string())?;
    match value["theme"].as_str() {
        Some("light") => Ok("light".into()),
        Some("dark") => Ok("dark".into()),
        _ => Ok("system".into()),
    }
}
