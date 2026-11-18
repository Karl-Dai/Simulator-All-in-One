//! Writable paths for the bundled SimLab applications. Never use the install tree.
use std::path::{Component, Path, PathBuf};

fn resolve(root: Option<PathBuf>, config: Option<PathBuf>, role: &str) -> std::io::Result<PathBuf> {
    let root = root
        .or_else(|| config.map(|p| p.join("SimLab")))
        .ok_or_else(|| {
            std::io::Error::new(
                std::io::ErrorKind::NotFound,
                "No user configuration directory",
            )
        })?;
    let mut components = Path::new(role).components();
    if !root.is_absolute()
        || !matches!(components.next(), Some(Component::Normal(_)))
        || components.next().is_some()
    {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "Invalid SimLab data path",
        ));
    }
    Ok(root.join(role))
}

pub fn role_dir(role: &str) -> std::io::Result<PathBuf> {
    let dir = resolve(
        std::env::var_os("SIMLAB_DATA_DIR").map(PathBuf::from),
        dirs::config_dir(),
        role,
    )?;
    std::fs::create_dir_all(&dir)?;
    Ok(dir)
}

pub fn master_pki_dir() -> std::io::Result<PathBuf> {
    Ok(role_dir("opcua-master")?.join("pki"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configured_root_and_roles_are_isolated() {
        let root = std::env::temp_dir().join("simlab-path-test");
        let master = resolve(Some(root.clone()), None, "opcua-master").unwrap();
        let server = resolve(Some(root.clone()), None, "opcua-server").unwrap();
        assert_eq!(master, root.join("opcua-master"));
        assert_ne!(master, server);
        assert!(master.is_absolute());
        assert!(resolve(Some(root.clone()), None, "..").is_err());
        assert!(resolve(Some(root), None, "master/other").is_err());
    }

    #[test]
    fn fallback_never_uses_the_working_directory() {
        let config = std::env::temp_dir().join("user-config");
        assert_eq!(
            resolve(None, Some(config.clone()), "opcua-master").unwrap(),
            config.join("SimLab/opcua-master")
        );
        assert!(resolve(
            Some(PathBuf::from("relative")),
            Some(config),
            "opcua-master"
        )
        .is_err());
        assert!(resolve(None, None, "opcua-master").is_err());
    }
}
