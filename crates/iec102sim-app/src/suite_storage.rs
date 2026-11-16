use crate::config_store::Store;
use serde::Serialize;
use serde_json::Value;
use std::path::Path;
use std::sync::Mutex;

#[derive(Default)]
struct Inner {
    store: Option<Store>,
    initialized: bool,
    blocked: Option<String>,
    error: Option<String>,
    write_errors: std::collections::BTreeMap<String, String>,
}

/// 本角色共享的配置存储；每次读写都经过同一把锁与同一版本号。
#[derive(Default)]
pub struct SuiteStorage(Mutex<Inner>);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
#[allow(dead_code)]
pub struct PersistenceStatus {
    pub enabled: bool,
    pub error: Option<String>,
}

#[allow(dead_code)]
impl SuiteStorage {
    pub fn initialize(&self, root: &Path, role: &str) -> Result<(), String> {
        let mut inner = self.0.lock().map_err(|e| e.to_string())?;
        inner.initialized = true;
        match Store::open(root, role) {
            Ok(store) => {
                inner.store = Some(store);
                inner.blocked = None;
                Ok(())
            }
            Err(error) => {
                inner.blocked = Some(error.clone());
                inner.error = Some(error.clone());
                Err(error)
            }
        }
    }

    pub fn block(&self, error: String) {
        if let Ok(mut inner) = self.0.lock() {
            inner.initialized = true;
            inner.blocked = Some(error.clone());
            inner.error = Some(error);
        }
    }

    pub fn ensure_writable(&self) -> Result<(), String> {
        let inner = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(error) = &inner.blocked {
            return Err(format!("配置恢复失败，已阻止自动保存: {error}"));
        }
        if inner.store.is_none() {
            #[cfg(test)]
            if !inner.initialized {
                return Ok(());
            }
            return Err("配置存储尚未初始化".into());
        }
        Ok(())
    }

    pub fn migrate(
        &self,
        key: &str,
        data: Option<&[u8]>,
        source: &str,
    ) -> Result<Option<Value>, String> {
        let mut inner = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(error) = &inner.blocked {
            return Err(error.clone());
        }
        let result = inner
            .store
            .as_mut()
            .ok_or("配置存储尚未初始化")?
            .migrate_json(key, data, source);
        if let Err(error) = &result {
            inner.blocked = Some(error.clone());
            inner.error = Some(error.clone());
        }
        result
    }

    pub fn load(&self, key: &str) -> Result<Option<Value>, String> {
        let mut inner = self.0.lock().map_err(|e| e.to_string())?;
        if let Some(error) = &inner.blocked {
            return Err(error.clone());
        }
        let result = inner.store.as_mut().ok_or("配置存储尚未初始化")?.load(key);
        if let Err(error) = &result {
            inner.blocked = Some(error.clone());
            inner.error = Some(error.clone());
        }
        result
    }

    pub fn save(&self, key: &str, value: &Value) -> Result<(), String> {
        self.ensure_writable()?;
        let mut inner = self.0.lock().map_err(|e| e.to_string())?;
        #[cfg(test)]
        if !inner.initialized {
            return Ok(());
        }
        let result = inner
            .store
            .as_mut()
            .ok_or("配置存储尚未初始化")?
            .save(key, value);
        match &result {
            Ok(()) => {
                inner.write_errors.remove(key);
            }
            Err(error) => {
                inner.write_errors.insert(key.into(), error.clone());
            }
        }
        result.map_err(|e| format!("配置未保存: {e}"))
    }

    pub fn load_preferences(&self, legacy: Value) -> Result<Value, String> {
        validate_preferences(&legacy)?;
        let data = serde_json::to_vec(&legacy).map_err(|e| e.to_string())?;
        let value = self
            .migrate("ui", Some(&data), "webview-localStorage-v1")?
            .unwrap_or_else(|| serde_json::json!({}));
        if let Err(error) = validate_preferences(&value) {
            self.block(error.clone());
            return Err(error);
        }
        Ok(value)
    }

    pub fn save_preferences(&self, values: Value) -> Result<(), String> {
        validate_preferences(&values)?;
        self.save("ui", &values)
    }

    pub fn status(&self) -> Result<PersistenceStatus, String> {
        let inner = self.0.lock().map_err(|e| e.to_string())?;
        Ok(PersistenceStatus {
            enabled: inner.store.is_some() && inner.blocked.is_none(),
            error: inner.error.clone().or_else(|| inner.write_errors.values().next().cloned()),
        })
    }
}

fn validate_preferences(value: &Value) -> Result<(), String> {
    let map = value.as_object().ok_or("界面配置必须是对象")?;
    for (key, item) in map {
        match item.get("kind").and_then(Value::as_str) {
            Some("text") if item.get("value").is_some_and(Value::is_string) => {}
            Some("json")
                if item
                    .get("value")
                    .is_some_and(|v| v.is_object() || v.is_array()) => {}
            _ => return Err(format!("界面配置格式无效: {key}")),
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn preferences_survive_a_restart() {
        let dir = tempfile::tempdir().unwrap();
        let storage = SuiteStorage::default();
        storage.initialize(dir.path(), "iec102.slave").unwrap();
        let legacy = serde_json::json!({
            "iec102.slave:form:v1": {"kind":"text","value":"{\"host\":\"10.0.0.9\",\"port\":2401}"}
        });
        let restored = storage.load_preferences(legacy).unwrap();
        assert_eq!(restored["iec102.slave:form:v1"]["value"], "{\"host\":\"10.0.0.9\",\"port\":2401}");
        let mut values = restored.as_object().unwrap().clone();
        values.insert(
            "iec102.slave:points:v1".into(),
            serde_json::json!({"kind":"json","value":[{"ioa":7,"kind":"single","value":true}]}),
        );
        storage.save_preferences(Value::Object(values)).unwrap();
        // 重新打开同一目录，模拟重启。
        let reopened = SuiteStorage::default();
        reopened.initialize(dir.path(), "iec102.slave").unwrap();
        let again = reopened.load_preferences(serde_json::json!({})).unwrap();
        assert_eq!(again["iec102.slave:points:v1"]["value"][0]["ioa"], 7);
        assert_eq!(again["iec102.slave:form:v1"]["value"], "{\"host\":\"10.0.0.9\",\"port\":2401}");
    }

    #[test]
    fn unreadable_store_blocks_writes() {
        let storage = SuiteStorage::default();
        storage.block("数据库不可读".into());
        assert!(storage.save_preferences(serde_json::json!({})).is_err());
        assert!(storage.load_preferences(serde_json::json!({})).is_err());
    }
}

#[cfg(test)]
mod theme_tests {
    #[test]
    fn launcher_theme_light_is_read_from_legacy_file() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(
            dir.path().join("launcher.json"),
            r#"{"schemaVersion":1,"theme":"light","language":"zh-CN"}"#,
        )
        .unwrap();
        std::env::set_var("SIMLAB_DATA_DIR", dir.path());
        let theme = crate::simlab_preferences::simlab_theme().unwrap();
        assert_eq!(theme, "light");
    }
}
