#[derive(Debug, serde::Serialize, PartialEq)]
#[serde(rename_all = "camelCase")]
pub(crate) struct AppearanceV1 {
    schema_version: u32,
    theme: String,
    language: String,
}

fn read_appearance(root: &std::path::Path) -> Result<AppearanceV1, String> {
    if !root.is_absolute() {
        return Err("SIMLAB_DATA_DIR must be absolute".into());
    }
    let mut result = AppearanceV1 {
        schema_version: 1,
        theme: "system".into(),
        language: "en".into(),
    };
    #[cfg(feature = "simlab-suite")]
    let stored = crate::config_store::read_json(root, "launcher", "preferences")?;
    #[cfg(not(feature = "simlab-suite"))]
    let stored: Option<serde_json::Value> = None;
    let value = match stored {
        Some(value) => value,
        None => match std::fs::read(root.join("launcher.json")) {
            Ok(data) => serde_json::from_slice(&data).map_err(|e| e.to_string())?,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(result),
            Err(error) => return Err(error.to_string()),
        },
    };
    if !value.is_object()
        || value.get("schemaVersion").is_some_and(|v| v.as_u64() != Some(1))
    {
        return Err("unsupported launcher preferences version".into());
    }
    if let Some(theme @ ("light" | "dark")) = value["theme"].as_str() {
        result.theme = theme.into();
    }
    if let Some(language @ ("auto" | "zh-CN")) = value["language"].as_str() {
        result.language = language.into();
    }
    Ok(result)
}

#[tauri::command]
pub(crate) fn simlab_appearance() -> Result<AppearanceV1, String> {
    if !cfg!(feature = "simlab-suite") {
        return Err("Appearance is managed by the standalone application".into());
    }
    let root = std::env::var_os("SIMLAB_DATA_DIR")
        .map(std::path::PathBuf::from)
        .ok_or("SIMLAB_DATA_DIR is not configured")?;
    read_appearance(&root)
}

/// Legacy theme behavior remains unchanged for components not yet migrated.
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
    Ok(read_appearance(&root)?.theme)
}

#[cfg(test)]
mod appearance_tests {
    use super::*;

    #[test]
    fn shared_file_contract_and_read_only_behavior() {
        let cases: serde_json::Value = serde_json::from_str(include_str!("contracts/appearance-cases.json")).unwrap();
        let unique = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_nanos();
        let root = std::env::temp_dir().join(format!("saio-appearance-{}-{unique}", std::process::id()));
        std::fs::create_dir_all(&root).unwrap();
        let path = root.join("launcher.json");
        for case in cases["fileCases"].as_array().unwrap() {
            if let Some(contents) = case["contents"].as_str() {
                std::fs::write(&path, contents).unwrap();
            } else if path.exists() {
                std::fs::remove_file(&path).unwrap();
            }
            let got = read_appearance(&root);
            if case.get("error").is_some() {
                assert!(got.is_err(), "{}", case["name"]);
            } else {
                assert_eq!(serde_json::to_value(got.unwrap()).unwrap(), case["appearance"], "{}", case["name"]);
            }
            if let Some(contents) = case["contents"].as_str() {
                assert_eq!(std::fs::read_to_string(&path).unwrap(), contents);
            }
        }
        std::fs::remove_dir_all(root).unwrap();
        assert!(read_appearance(std::path::Path::new("relative")).is_err());
    }
}
