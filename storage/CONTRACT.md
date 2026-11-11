# Unified configuration storage contract (v1)

The database is `<absolute root>/saio.db`; role names are `launcher`, `iec104.master`, `iec104.slave`, `modbus.master`, `modbus.slave`, `opcua.master`, `opcua.server`, `pmu.master`, `pmu.sub`. JSON is only an in-memory interchange/legacy import, never a stored complete document.

Go `configstore` (canonical `storage/go`, copied to Modbus `go/internal/configstore`):
- `Open(root, role string) (*Store,error)`; `Close() error`.
- `Load(key string, dst any) (present bool,error)`; dst may be `*any`, structs or maps. Absence captures revision zero; JSON null is present.
- `Save(key string, value any) error`: requires prior successful Load/MigrateJSON for key; uses captured revision, rejects stale writes without changing it. An unchanged value is a no-op but still checks revision.
- `MigrateJSON(key string, data []byte, source string) (present bool,error)`: absent-only import, records source SHA256 atomically, captures current document's revision. Existing document wins; pass nil data when no legacy file exists (captures absence).
- `ExportProject(path, role,key string,value any) error`; `ReadProject(path,role,key string,dst any) error`: single-document SQLite file, legacy JSON accepted on read. Existing export destination is atomically replaced.
- `ReadJSON(root,role,key string) ([]byte,bool,error)` convenience read-only result encoded as in-memory JSON for shared appearance adapters.

Rust shared module `config_store` (canonical `storage/rust/config_store.rs`, copied to `crates/shared/config_store.rs`):
- `Store::open(root: &Path, role: &str) -> Result<Store,String>`.
- `load(&mut self,key: &str) -> Result<Option<serde_json::Value>,String>`.
- `save(&mut self,key: &str,value: &Value) -> Result<(),String>`.
- `migrate_json(&mut self,key:&str,data:Option<&[u8]>,source:&str) -> Result<Option<Value>,String>`.
- `export_project(path:&Path,role:&str,key:&str,value:&Value) -> Result<(),String>`.
- `read_project(path:&Path,role:&str,key:&str) -> Result<Value,String>` (SQLite or legacy JSON).
- `read_json(root:&Path,role:&str,key:&str) -> Result<Option<Value>,String>` convenience.

Documents: launcher/preferences; each application/workspace; frontend preferences may use separate keys `settings`, `presets`, `ui`. Each session must load any key before saving. A failed load leaves it unauthorized for writes. Migrations preserve legacy source files; caller must not delete them. No Delete API: save the valid empty state to prevent legacy resurrection. Do not hold the database connection on the UI thread during expensive work.

SQLite format has application_id `0x5341494f`, user_version 1. Tables document, field, node, migration are shared by both languages. `node.kind`: 0 null,1 false,2 true,3 signed integer,4 unsigned integer (8-byte big-endian BLOB),5 finite double,6 text,7 array,8 object. Root node has node_id=0,parent_id=-1,slot=0. Direct scalar children of each array/object are packed in that container’s value BLOB; nested containers retain separate SQL nodes. The BLOB starts bytes 0x53,0x01, followed by zero or more records sorted by numeric slot: signed zigzag LEB128 slot, one-byte kind, scalar payload (kind0-2 none; kind3 signed zigzag LEB128; kind4 unsigned big-endian8; kind5 IEEE754 big-endian8; kind6 unsigned LEB128 UTF-8 byte count then bytes). No subtree or whole-document serialization is stored. Empty/no-scalar containers use SQL NULL. Scalar roots remain native typed SQL values. Object slots are positive field IDs; array slots are -(ordinal+1). Unchanged paths retain node IDs. Fields are interned globally; document updates are transactional diffs, not whole document deletion. No indexes on scalar values. CAS revisions and BEGIN IMMEDIATE serialize writes safely.

Error contract: never catch a restore error and then auto-save defaults. Publish a config mutation only after its Save succeeds or restore the old in-memory state on failure under the same mutation lock. Same-role multiple instances receive a conflict, rather than overwriting newer config. Connection/listener start is separate from config restoration.
