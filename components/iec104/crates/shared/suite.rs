use std::path::PathBuf;
use tauri::AppHandle;
#[cfg(feature = "simlab-suite")]
use tauri::Manager;

#[cfg(feature = "simlab-suite")]
struct SuiteDataRoot(PathBuf);

/// Suite stores use an absolute path; standalone stores retain their old path.
pub(crate) fn store_path(app: &AppHandle, filename: &str) -> PathBuf {
    #[cfg(feature = "simlab-suite")]
    {
        return app.state::<SuiteDataRoot>().0.join(filename);
    }
    #[cfg(not(feature = "simlab-suite"))]
    {
        let _ = app;
        PathBuf::from(filename)
    }
}

pub(crate) fn ensure_component_updates_allowed() -> Result<(), String> {
    if cfg!(feature = "simlab-suite") {
        Err("套件内组件由 SimLab 整包更新，请从套件入口获取新版。".into())
    } else {
        Ok(())
    }
}

#[cfg(feature = "simlab-suite")]
fn role_data_root(base: PathBuf, role: &str) -> Result<PathBuf, String> {
    if !base.is_absolute() {
        return Err("SIMLAB_DATA_DIR 必须是绝对路径。".into());
    }
    Ok(base.join(role))
}

#[cfg(feature = "simlab-suite")]
pub(crate) fn initialize(
    app: &mut tauri::App,
    role: &str,
    expected_identifier: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if app.config().identifier != expected_identifier {
        return Err("套件构建缺少 tauri.suite.conf.json 配置。".into());
    }
    let base = match std::env::var_os("SIMLAB_DATA_DIR") {
        Some(value) => PathBuf::from(value),
        None => app.path().config_dir()?.join("SimLab"),
    };
    let root = role_data_root(base, role)?;
    std::fs::create_dir_all(&root)?;
    let webview_root = root.join("webview");
    std::fs::create_dir_all(&webview_root)?;
    app.manage(SuiteDataRoot(root));

    // The suite overlay delays initial window creation until its storage path
    // is available. Windows/Linux honor this directory. WKWebView storage on
    // macOS is isolated by the suite-only bundle identifier instead.
    let window = app
        .config()
        .app
        .windows
        .first()
        .ok_or("套件缺少主窗口配置。")?;
    tauri::WebviewWindowBuilder::from_config(app.handle(), window)?
        .data_directory(webview_root)
        .build()?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn updates_belong_to_the_selected_distribution() {
        assert_eq!(
            ensure_component_updates_allowed().is_err(),
            cfg!(feature = "simlab-suite")
        );
    }

    #[cfg(feature = "simlab-suite")]
    #[test]
    fn role_storage_is_absolute_and_separated() {
        let base = std::env::temp_dir().join("SimLab 中文 space");
        let master = role_data_root(base.clone(), "iec104-master").unwrap();
        let slave = role_data_root(base.clone(), "iec104-slave").unwrap();
        assert_eq!(master, base.join("iec104-master"));
        assert_ne!(master, slave);
        assert!(role_data_root(PathBuf::from("relative"), "iec104-master").is_err());
    }
}
