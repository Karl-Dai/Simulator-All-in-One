//! 套件运行时的数据目录初始化。
//!
//! 套件模式下每个角色使用 `SIMLAB_DATA_DIR/<role>`，独立模式沿用应用配置目录。
use std::path::PathBuf;
use tauri::{AppHandle, Manager};

struct SuiteDataRoot(PathBuf);

/// 返回组件配置文件所在路径。未初始化时退回相对路径。
#[allow(dead_code)]
pub(crate) fn store_path(app: &AppHandle, filename: &str) -> PathBuf {
    match app.try_state::<SuiteDataRoot>() {
        Some(root) => root.0.join(filename),
        None => PathBuf::from(filename),
    }
}

#[cfg(feature = "simlab-suite")]
pub(crate) fn initialize(
    app: &mut tauri::App,
    role: &str,
    expected_identifier: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    if app.config().identifier != expected_identifier {
        return Err("套件构建缺少 tauri.suite.conf.json 配置。".into());
    }
    let base = match std::env::var_os("SIMLAB_DATA_DIR") {
        Some(value) => PathBuf::from(value),
        None => app.path().config_dir()?.join("SimLab"),
    };
    if !base.is_absolute() {
        return Err("SIMLAB_DATA_DIR 必须是绝对路径。".into());
    }
    let root = base.join(role);
    std::fs::create_dir_all(&root)?;
    app.manage(SuiteDataRoot(root.clone()));
    Ok(root)
}

#[cfg(not(feature = "simlab-suite"))]
pub(crate) fn initialize(
    app: &mut tauri::App,
    _role: &str,
    _expected_identifier: &str,
) -> Result<PathBuf, Box<dyn std::error::Error>> {
    let root = app.path().app_config_dir()?;
    std::fs::create_dir_all(&root)?;
    app.manage(SuiteDataRoot(root.clone()));
    Ok(root)
}

#[cfg(test)]
mod tests {
    #[test]
    fn suite_roles_use_separate_directories() {
        let base = std::env::temp_dir().join("SimLab 中文 space");
        assert_ne!(base.join("iec101.master"), base.join("iec101.slave"));
    }
}
