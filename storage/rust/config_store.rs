//! Shared, versioned configuration storage. Values are typed SQLite records.
use rusqlite::{params, Connection, OptionalExtension};
use serde_json::{Map, Number, Value};
use std::{
    collections::{BTreeMap, HashMap, HashSet},
    path::Path,
    time::Duration,
};
const APPLICATION_ID: i64 = 1396787535;
const SCHEMA: &str = include_str!("config_schema.sql");
const CONFLICT: &str = "configuration changed in another window; reload before saving";
#[derive(Clone, PartialEq)]
struct Cell {
    id: i64,
    parent: i64,
    slot: i64,
    kind: i64,
    value: rusqlite::types::Value,
}
pub struct Store {
    db: Connection,
    role: String,
    revisions: HashMap<String, i64>,
}
fn valid_role(role: &str) -> bool {
    matches!(
        role,
        "launcher"
            | "iec104.master"
            | "iec104.slave"
            | "modbus.master"
            | "modbus.slave"
            | "opcua.master"
            | "opcua.server"
            | "pmu.master"
            | "pmu.sub"
    )
}
fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}
fn create_private_directory(path: &Path) -> Result<(), String> {
    let mut builder = std::fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path).map_err(err)
}
fn validate(db: &Connection, create: bool) -> Result<(), String> {
    let id: i64 = db
        .pragma_query_value(None, "application_id", |r| r.get(0))
        .map_err(err)?;
    let version: i64 = db
        .pragma_query_value(None, "user_version", |r| r.get(0))
        .map_err(err)?;
    if id == APPLICATION_ID && version == 1 {
        return Ok(());
    }
    let count: i64 = db
        .query_row(
            "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'",
            [],
            |r| r.get(0),
        )
        .map_err(err)?;
    if create && id == 0 && version == 0 && count == 0 {
        return db.execute_batch(SCHEMA).map_err(err);
    }
    Err(format!(
        "unsupported or unrelated configuration database (application={id} version={version})"
    ))
}
impl Store {
    pub fn open(root: &Path, role: &str) -> Result<Self, String> {
        if !root.is_absolute() || !valid_role(role) {
            return Err("invalid configuration root or role".into());
        }
        create_private_directory(root)?;
        Self::open_file(&root.join("saio.db"), role, false)
    }
    fn open_file(path: &Path, role: &str, readonly: bool) -> Result<Self, String> {
        if !valid_role(role) {
            return Err("invalid configuration role".into());
        }
        if !readonly {
            let mut options = std::fs::OpenOptions::new();
            options.read(true).write(true).create(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                options.mode(0o600);
            }
            drop(options.open(path).map_err(err)?);
        }
        let db = if readonly {
            Connection::open_with_flags(path, rusqlite::OpenFlags::SQLITE_OPEN_READ_ONLY)
        } else {
            Connection::open(path)
        }
        .map_err(err)?;
        db.busy_timeout(Duration::from_secs(5)).map_err(err)?;
        db.pragma_update(None, "foreign_keys", true).map_err(err)?;
        if readonly {
            validate(&db, false)?
        } else {
            db.execute_batch("BEGIN IMMEDIATE").map_err(err)?;
            if let Err(e) = validate(&db, true) {
                let _ = db.execute_batch("ROLLBACK");
                return Err(e);
            }
            db.execute_batch("COMMIT; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL;")
                .map_err(err)?;
        }
        Ok(Self {
            db,
            role: role.into(),
            revisions: HashMap::new(),
        })
    }
    pub fn load(&mut self, key: &str) -> Result<Option<Value>, String> {
        self.revisions.remove(key);
        let tx = self.db.unchecked_transaction().map_err(err)?;
        let (value, _, rev, _) = read(&tx, &self.role, key)?;
        tx.commit().map_err(err)?;
        self.revisions.insert(key.into(), rev);
        Ok(value)
    }
    pub fn save(&mut self, key: &str, value: &Value) -> Result<(), String> {
        let expected = *self
            .revisions
            .get(key)
            .ok_or("configuration must be loaded before saving")?;
        let rev = self.write(key, value, expected, None)?;
        self.revisions.insert(key.into(), rev);
        Ok(())
    }
    pub fn migrate_json(
        &mut self,
        key: &str,
        data: Option<&[u8]>,
        source: &str,
    ) -> Result<Option<Value>, String> {
        let existing = self.load(key)?;
        if existing.is_some() || data.is_none() {
            return Ok(existing);
        }
        let data = data.unwrap();
        let value: Value = match serde_json::from_slice(data) {
            Ok(v) => v,
            Err(e) => {
                self.revisions.remove(key);
                return Err(err(e));
            }
        };
        // SHA-256 comes from the shared dependency, never a legacy-file deletion.
        use sha2::Digest;
        let digest = format!("{:x}", sha2::Sha256::digest(data));
        match self.write(key, &value, 0, Some((source, &digest))) {
            Ok(rev) => {
                self.revisions.insert(key.into(), rev);
                Ok(Some(value))
            }
            Err(e) if e == CONFLICT => self.load(key),
            Err(e) => {
                self.revisions.remove(key);
                Err(e)
            },
        }
    }
    fn write(
        &mut self,
        key: &str,
        value: &Value,
        expected: i64,
        migration: Option<(&str, &str)>,
    ) -> Result<i64, String> {
        let tx = self
            .db
            .transaction_with_behavior(rusqlite::TransactionBehavior::Immediate)
            .map_err(err)?;
        let (_, mut doc, mut rev, old) = read(&tx, &self.role, key)?;
        if rev != expected {
            return Err(CONFLICT.into());
        }
        if rev == 0 {
            tx.execute(
                "INSERT INTO document(role,key,revision) VALUES(?,?,1)",
                params![self.role, key],
            )
            .map_err(err)?;
            doc = tx.last_insert_rowid();
        }
        let mut fields = fields(&tx)?
            .into_iter()
            .map(|(id, name)| (name, id))
            .collect::<HashMap<_, _>>();
        let edges = old
            .values()
            .map(|c| ((c.parent, c.slot), c.id))
            .collect::<HashMap<_, _>>();
        let mut next = old.keys().copied().max().unwrap_or(0);
        let mut nodes = BTreeMap::new();
        flatten(
            &tx,
            value,
            0,
            -1,
            0,
            0,
            &edges,
            &mut next,
            &mut fields,
            &mut nodes,
        )?;
        let mut changed = rev == 0;
        for id in old.keys() {
            if !nodes.contains_key(id) {
                tx.execute(
                    "DELETE FROM node WHERE document_id=? AND node_id=?",
                    params![doc, id],
                )
                .map_err(err)?;
                changed = true;
            }
        }
        {
            let mut statement=tx.prepare_cached("INSERT INTO node(document_id,node_id,parent_id,slot,kind,value) VALUES(?,?,?,?,?,?) ON CONFLICT(document_id,node_id) DO UPDATE SET parent_id=excluded.parent_id,slot=excluded.slot,kind=excluded.kind,value=excluded.value").map_err(err)?;
            for (id, c) in nodes {
                if old.get(&id) == Some(&c) {
                    continue;
                }
                statement
                    .execute(params![doc, id, c.parent, c.slot, c.kind, c.value])
                    .map_err(err)?;
                changed = true;
            }
        }
        if rev == 0 {
            rev = 1
        } else if changed {
            rev += 1;
            tx.execute(
                "UPDATE document SET revision=? WHERE id=?",
                params![rev, doc],
            )
            .map_err(err)?;
        }
        if let Some((source, digest)) = migration {
            tx.execute(
                "INSERT INTO migration(role,key,source,sha256) VALUES(?,?,?,?)",
                params![self.role, key, source, digest],
            )
            .map_err(err)?;
        }
        tx.commit().map_err(err)?;
        Ok(rev)
    }
}
fn fields(db: &Connection) -> Result<HashMap<i64, String>, String> {
    let mut stmt = db.prepare("SELECT id,name FROM field").map_err(err)?;
    let rows = stmt
        .query_map([], |r| Ok((r.get(0)?, r.get(1)?)))
        .map_err(err)?;
    rows.collect::<Result<_, _>>().map_err(err)
}
fn read(
    db: &Connection,
    role: &str,
    key: &str,
) -> Result<(Option<Value>, i64, i64, BTreeMap<i64, Cell>), String> {
    let header: Option<(i64, i64)> = db
        .query_row(
            "SELECT id,revision FROM document WHERE role=? AND key=?",
            params![role, key],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()
        .map_err(err)?;
    let Some((doc, rev)) = header else {
        return Ok((None, 0, 0, BTreeMap::new()));
    };
    let mut stmt = db
        .prepare("SELECT node_id,parent_id,slot,kind,value FROM node WHERE document_id=?")
        .map_err(err)?;
    let rows = stmt
        .query_map([doc], |r| {
            Ok(Cell {
                id: r.get(0)?,
                parent: r.get(1)?,
                slot: r.get(2)?,
                kind: r.get(3)?,
                value: r.get(4)?,
            })
        })
        .map_err(err)?;
    let mut nodes = BTreeMap::new();
    let mut children: HashMap<i64, Vec<i64>> = HashMap::new();
    for row in rows {
        let c = row.map_err(err)?;
        if c.id != 0 {
            children.entry(c.parent).or_default().push(c.id);
        }
        nodes.insert(c.id, c);
    }
    let sql_nodes = nodes.clone();
    let mut virtual_id = -2i64;
    for (id, c) in &sql_nodes {
        if c.kind == 7 || c.kind == 8 {
            match &c.value {
                rusqlite::types::Value::Null => {}
                rusqlite::types::Value::Blob(data) => {
                    for mut leaf in unpack_scalars(data)? {
                        leaf.id = virtual_id;
                        leaf.parent = *id;
                        virtual_id -= 1;
                        children.entry(*id).or_default().push(leaf.id);
                        nodes.insert(leaf.id, leaf);
                    }
                    nodes.get_mut(id).unwrap().value = rusqlite::types::Value::Null;
                }
                _ => return Err("invalid packed container".into()),
            }
        }
    }
    let root = nodes.get(&0).ok_or("missing configuration root")?;
    if root.parent != -1 || root.slot != 0 {
        return Err("invalid configuration root".into());
    }
    let mut seen = HashSet::new();
    let value = restore(0, 0, &nodes, &children, &fields(db)?, &mut seen)?;
    if seen.len() != nodes.len() {
        return Err("orphan configuration records".into());
    }
    Ok((Some(value), doc, rev, sql_nodes))
}
fn restore(
    id: i64,
    depth: usize,
    nodes: &BTreeMap<i64, Cell>,
    children: &HashMap<i64, Vec<i64>>,
    fields: &HashMap<i64, String>,
    seen: &mut HashSet<i64>,
) -> Result<Value, String> {
    use rusqlite::types::Value as Sql;
    if depth > 256 || !seen.insert(id) {
        return Err("invalid configuration node graph".into());
    }
    let c = nodes.get(&id).ok_or("missing configuration node")?;
    let value = match (&c.kind, &c.value) {
        (0, Sql::Null) => Value::Null,
        (1, Sql::Null) => Value::Bool(false),
        (2, Sql::Null) => Value::Bool(true),
        (3, Sql::Integer(v)) => Value::Number((*v).into()),
        (4, Sql::Blob(v)) if v.len() == 8 => {
            Value::Number(u64::from_be_bytes(v.as_slice().try_into().unwrap()).into())
        }
        (5, Sql::Real(v)) => {
            Value::Number(Number::from_f64(*v).ok_or("nonfinite configuration value")?)
        }
        (5, Sql::Integer(v)) => Value::Number(Number::from_f64(*v as f64).unwrap()),
        (6, Sql::Text(v)) => Value::String(v.clone()),
        (7, Sql::Null) => {
            let mut ids = children.get(&id).cloned().unwrap_or_default();
            ids.sort_by_key(|n| -nodes[n].slot);
            let mut out = Vec::new();
            for (i, n) in ids.iter().enumerate() {
                if nodes[n].slot != -(i as i64) - 1 {
                    return Err("invalid array ordering".into());
                }
                out.push(restore(*n, depth + 1, nodes, children, fields, seen)?);
            }
            Value::Array(out)
        }
        (8, Sql::Null) => {
            let mut out = Map::new();
            for n in children.get(&id).into_iter().flatten() {
                let slot = nodes[n].slot;
                let name = fields
                    .get(&slot)
                    .filter(|_| slot > 0)
                    .ok_or("invalid object field")?;
                if out.contains_key(name) {
                    return Err("duplicate object field".into());
                }
                out.insert(
                    name.clone(),
                    restore(*n, depth + 1, nodes, children, fields, seen)?,
                );
            }
            Value::Object(out)
        }
        _ => return Err("invalid typed configuration value".into()),
    };
    Ok(value)
}
#[allow(clippy::too_many_arguments)]
fn flatten(
    db: &Connection,
    value: &Value,
    id: i64,
    parent: i64,
    slot: i64,
    depth: usize,
    edges: &HashMap<(i64, i64), i64>,
    next: &mut i64,
    fields: &mut HashMap<String, i64>,
    nodes: &mut BTreeMap<i64, Cell>,
) -> Result<(), String> {
    use rusqlite::types::Value as Sql;
    if depth > 256 {
        return Err("configuration nesting exceeds 256".into());
    }
    let (kind, scalar) = make_scalar(value)?;
    nodes.insert(
        id,
        Cell {
            id,
            parent,
            slot,
            kind,
            value: scalar,
        },
    );
    let mut packed = Vec::new();
    let mut child =
        |v: &Value, slot: i64, fields: &mut HashMap<String, i64>| -> Result<(), String> {
            if !value_is_container(v) {
                let (kind, value) = make_scalar(v)?;
                packed.push(Cell {
                    id: 0,
                    parent: id,
                    slot,
                    kind,
                    value,
                });
                return Ok(());
            }
            let cid = if let Some(cid) = edges.get(&(id, slot)) {
                *cid
            } else {
                *next += 1;
                *next
            };
            flatten(db, v, cid, id, slot, depth + 1, edges, next, fields, nodes)
        };
    match value {
        Value::Array(list) => {
            for (i, v) in list.iter().enumerate() {
                child(v, -(i as i64) - 1, fields)?
            }
        }
        Value::Object(map) => {
            for (name, v) in map {
                let fid = if let Some(fid) = fields.get(name) {
                    *fid
                } else {
                    db.execute("INSERT OR IGNORE INTO field(name) VALUES(?)", [name])
                        .map_err(err)?;
                    let fid = db
                        .query_row("SELECT id FROM field WHERE name=?", [name], |r| r.get(0))
                        .map_err(err)?;
                    fields.insert(name.clone(), fid);
                    fid
                };
                child(v, fid, fields)?
            }
        }
        _ => {}
    }
    if !packed.is_empty() {
        nodes.get_mut(&id).unwrap().value = Sql::Blob(pack_scalars(&mut packed));
    }
    Ok(())
}
pub fn read_json(root: &Path, role: &str, key: &str) -> Result<Option<Value>, String> {
    Store::open(root, role)?.load(key)
}
pub fn export_project(path: &Path, role: &str, key: &str, value: &Value) -> Result<(), String> {
    if !valid_role(role) || key.is_empty() {
        return Err("invalid project role or key".into());
    }
    if path.file_name().and_then(|v| v.to_str()).is_some_and(|v| matches!(v.to_ascii_lowercase().as_str(), "saio.db" | "saio.db-wal" | "saio.db-shm" | "saio.db-journal")) {
        return Err("project export cannot replace the shared configuration database".into());
    }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or_else(|| Path::new("."));
    create_private_directory(parent)?;
    let temp = tempfile::NamedTempFile::new_in(parent).map_err(err)?;
    {
        let mut store = Store::open_file(temp.path(), role, false)?;
        store.load(key)?;
        store.save(key, value)?;
        store
            .db
            .execute_batch("PRAGMA wal_checkpoint(TRUNCATE); PRAGMA journal_mode=DELETE;")
            .map_err(err)?;
    }
    temp.persist(path).map_err(err)?;
    Ok(())
}
pub fn read_project(path: &Path, role: &str, key: &str) -> Result<Value, String> {
    if !valid_role(role) || key.is_empty() {
        return Err("invalid project role or key".into());
    }
    let data = std::fs::read(path).map_err(err)?;
    if !data.starts_with(b"SQLite format 3\0") {
        return serde_json::from_slice(&data).map_err(err);
    }
    let mut store = Store::open_file(path, role, true)?;
    let count: i64 = store
        .db
        .query_row("SELECT count(*) FROM document", [], |r| r.get(0))
        .map_err(err)?;
    if count != 1 {
        return Err("project must contain exactly one document".into());
    }
    store
        .load(key)?
        .ok_or_else(|| "project role or document does not match".into())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    #[test]
    fn new_database_and_journals_are_private() {
        use std::os::unix::fs::PermissionsExt;
        let base = tempfile::tempdir().unwrap();
        let root = base.path().join("new-root");
        let mut store = Store::open(&root,"launcher").unwrap();
        store.migrate_json("preferences",Some(br#"{"schemaVersion":1}"#),"test").unwrap();
        assert_eq!(std::fs::metadata(&root).unwrap().permissions().mode() & 0o777,0o700);
        for name in ["saio.db","saio.db-wal","saio.db-shm"] {
            if let Ok(info) = std::fs::metadata(root.join(name)) {
                assert_eq!(info.permissions().mode() & 0o777,0o600,"{name}");
            }
        }
    }

    #[test]
    fn project_is_single_file_without_journal_sidecars() {
        let root = tempfile::tempdir().unwrap();
        let folder = root.path().join("nested");
        let path = folder.join("case.saioproj");
        export_project(&path,"pmu.master","workspace",&serde_json::json!({"value":1})).unwrap();
        read_project(&path,"pmu.master","workspace").unwrap();
        assert_eq!(std::fs::read_dir(&folder).unwrap().count(),1);
    }

    #[test]
    fn failed_migration_revokes_write_and_export_preserves_database() {
        let root = tempfile::tempdir().unwrap();
        let mut store = Store::open(root.path(), "launcher").unwrap();
        store.db.execute_batch("CREATE TRIGGER abort_migration BEFORE INSERT ON migration BEGIN SELECT RAISE(ABORT,'failed migration'); END;").unwrap();
        assert!(store.migrate_json("preferences", Some(br#"{"theme":"dark"}"#), "legacy").is_err());
        assert!(store.save("preferences", &serde_json::json!({"theme":"default"})).is_err());
        assert_eq!(store.db.query_row("SELECT count(*) FROM document", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        assert_eq!(store.db.query_row("SELECT count(*) FROM migration", [], |r| r.get::<_,i64>(0)).unwrap(), 0);
        for name in ["saio.db", "SAIO.DB", "SaIo.Db-WaL", "saio.db-shm", "saio.db-journal"] {
            assert!(export_project(&root.path().join(name), "pmu.master", "workspace", &Value::Null).is_err());
        }
        store.db.execute_batch("DROP TRIGGER abort_migration").unwrap();
        store.migrate_json("preferences", Some(br#"{"theme":"dark"}"#), "legacy").unwrap();
        let mut reopened = Store::open(root.path(), "launcher").unwrap();
        assert_eq!(reopened.load("preferences").unwrap().unwrap()["theme"], "dark");
    }

    #[test]
    fn migration_cas_types_and_project() {
        let root = tempfile::tempdir().unwrap();
        let mut a = Store::open(root.path(), "pmu.master").unwrap();
        let mut b = Store::open(root.path(), "pmu.master").unwrap();
        assert!(a.save("workspace", &Value::Null).is_err());
        let value = serde_json::json!({"雪":[null,true,false,i64::MIN,u64::MAX,1.0,[],{}]});
        let data = serde_json::to_vec(&value).unwrap();
        assert_eq!(
            a.migrate_json("workspace", Some(&data), "legacy").unwrap(),
            Some(value.clone())
        );
        assert_eq!(b.load("workspace").unwrap(), Some(value));
        a.save("workspace", &serde_json::json!({})).unwrap();
        assert_eq!(b.save("workspace", &Value::Null).unwrap_err(), CONFLICT);
        assert_eq!(
            a.migrate_json("workspace", Some(b"bad"), "legacy").unwrap(),
            Some(serde_json::json!({}))
        );
        assert!(a.migrate_json("bad", Some(b"{"), "legacy").is_err());
        assert!(a.save("bad", &Value::Null).is_err());
        let path = root.path().join("case.saioproj");
        export_project(
            &path,
            "pmu.master",
            "workspace",
            &serde_json::json!({"max":u64::MAX}),
        )
        .unwrap();
        assert_eq!(
            read_project(&path, "pmu.master", "workspace").unwrap(),
            serde_json::json!({"max":u64::MAX})
        );
        assert!(read_project(&path, "pmu.sub", "workspace").is_err());
    }
}

fn value_is_container(v: &Value) -> bool {
    matches!(v, Value::Array(_) | Value::Object(_))
}
fn make_scalar(value: &Value) -> Result<(i64, rusqlite::types::Value), String> {
    use rusqlite::types::Value as Sql;
    let result = match value {
        Value::Null => (0, Sql::Null),
        Value::Bool(false) => (1, Sql::Null),
        Value::Bool(true) => (2, Sql::Null),
        Value::String(v) => (6, Sql::Text(v.clone())),
        Value::Number(v) => {
            if let Some(i) = v.as_i64() {
                (3, Sql::Integer(i))
            } else if let Some(u) = v.as_u64() {
                (4, Sql::Blob(u.to_be_bytes().to_vec()))
            } else {
                (5, Sql::Real(v.as_f64().ok_or("invalid number")?))
            }
        }
        Value::Array(_) => (7, Sql::Null),
        Value::Object(_) => (8, Sql::Null),
    };
    Ok(result)
}
fn put_var(out: &mut Vec<u8>, mut value: u64) {
    while value >= 128 {
        out.push((value as u8) | 128);
        value >>= 7;
    }
    out.push(value as u8);
}
fn take_var(data: &mut &[u8]) -> Result<u64, String> {
    let mut value = 0u64;
    for i in 0..10 {
        let b = *data.first().ok_or("truncated packed scalar")?;
        *data = &data[1..];
        if i == 9 && b > 1 {
            return Err("overflow in packed scalar".into());
        }
        value |= ((b & 127) as u64) << (i * 7);
        if b < 128 {
            return Ok(value);
        }
    }
    Err("invalid packed scalar varint".into())
}
fn pack_scalars(cells: &mut [Cell]) -> Vec<u8> {
    use rusqlite::types::Value as Sql;
    cells.sort_by_key(|c| c.slot);
    let mut out = vec![b'S', 1];
    for c in cells {
        put_var(&mut out, ((c.slot << 1) ^ (c.slot >> 63)) as u64);
        out.push(c.kind as u8);
        match &c.value {
            Sql::Null => {}
            Sql::Integer(v) => put_var(&mut out, ((v << 1) ^ (v >> 63)) as u64),
            Sql::Real(v) => out.extend_from_slice(&v.to_bits().to_be_bytes()),
            Sql::Text(v) => {
                put_var(&mut out, v.len() as u64);
                out.extend_from_slice(v.as_bytes());
            }
            Sql::Blob(v) => out.extend_from_slice(v),
        }
    }
    out
}
fn unpack_scalars(mut data: &[u8]) -> Result<Vec<Cell>, String> {
    use rusqlite::types::Value as Sql;
    if !data.starts_with(&[b'S', 1]) {
        return Err("invalid packed scalar record".into());
    }
    data = &data[2..];
    let mut out = Vec::new();
    let mut seen = HashSet::new();
    while !data.is_empty() {
        let n = take_var(&mut data)?;
        let slot = ((n >> 1) as i64) ^ -((n & 1) as i64);
        if slot == 0 || !seen.insert(slot) {
            return Err("invalid packed scalar slot".into());
        }
        let kind = *data.first().ok_or("missing packed scalar kind")? as i64;
        data = &data[1..];
        let value = match kind {
            0..=2 => Sql::Null,
            3 => {
                let n = take_var(&mut data)?;
                Sql::Integer(((n >> 1) as i64) ^ -((n & 1) as i64))
            }
            4 | 5 => {
                if data.len() < 8 {
                    return Err("truncated packed scalar".into());
                }
                let b = &data[..8];
                let value = if kind == 4 {
                    Sql::Blob(b.to_vec())
                } else {
                    Sql::Real(f64::from_bits(u64::from_be_bytes(b.try_into().unwrap())))
                };
                data = &data[8..];
                value
            }
            6 => {
                let len = usize::try_from(take_var(&mut data)?).map_err(err)?;
                if len > data.len() {
                    return Err("truncated packed text".into());
                }
                let value = Sql::Text(std::str::from_utf8(&data[..len]).map_err(err)?.into());
                data = &data[len..];
                value
            }
            _ => return Err("invalid packed scalar kind".into()),
        };
        out.push(Cell {
            id: 0,
            parent: 0,
            slot,
            kind,
            value,
        });
    }
    Ok(out)
}
