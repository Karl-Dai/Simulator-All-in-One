// Package configstore persists suite configuration as typed SQLite records.
package configstore

import (
	"bytes"
	"context"
	"crypto/sha256"
	"database/sql"
	_ "embed"
	"encoding/binary"
	"encoding/hex"
	"encoding/json"
	"errors"
	"fmt"
	"io"
	"math"
	"net/url"
	"os"
	"path/filepath"
	"reflect"
	"sort"
	"strconv"
	"strings"
	"sync"
	"unicode/utf8"

	_ "modernc.org/sqlite"
)

//go:embed schema.sql
var schema string

const applicationID = 1396787535

var ErrConflict = errors.New("configuration changed in another window; reload before saving")

type Store struct {
	db        *sql.DB
	role      string
	mu        sync.Mutex
	revisions map[string]int64
}
type cell struct {
	id, parent, slot int64
	kind             int
	value            any
}
type queryer interface {
	QueryContext(context.Context, string, ...any) (*sql.Rows, error)
	QueryRowContext(context.Context, string, ...any) *sql.Row
	ExecContext(context.Context, string, ...any) (sql.Result, error)
}

func validRole(role string) bool {
	switch role {
	case "launcher", "iec104.master", "iec104.slave", "modbus.master", "modbus.slave", "opcua.master", "opcua.server", "pmu.master", "pmu.sub":
		return true
	}
	return false
}
func Open(root, role string) (*Store, error) {
	if !filepath.IsAbs(root) || !validRole(role) {
		return nil, fmt.Errorf("invalid configuration root or role")
	}
	if err := os.MkdirAll(root, 0700); err != nil {
		return nil, err
	}
	return openFile(filepath.Join(root, "saio.db"), role, false)
}
func openFile(path, role string, readonly bool) (*Store, error) {
	if !validRole(role) {
		return nil, fmt.Errorf("invalid configuration role")
	}
	if !readonly {
		file, err := os.OpenFile(path, os.O_RDWR|os.O_CREATE, 0600)
		if err != nil {
			return nil, err
		}
		if err = file.Close(); err != nil {
			return nil, err
		}
	}
	dsn := path
	if readonly {
		dsn = (&url.URL{Scheme: "file", Path: filepath.ToSlash(path), RawQuery: "mode=ro"}).String()
	}
	db, err := sql.Open("sqlite", dsn)
	if err != nil {
		return nil, err
	}
	db.SetMaxOpenConns(1)
	fail := func(e error) (*Store, error) { db.Close(); return nil, e }
	if _, err = db.Exec("PRAGMA busy_timeout=5000"); err != nil {
		return fail(err)
	}
	if _, err = db.Exec("PRAGMA foreign_keys=ON"); err != nil {
		return fail(err)
	}
	if readonly {
		if err = validate(db, false); err != nil {
			return fail(err)
		}
	} else {
		conn, e := db.Conn(context.Background())
		if e != nil {
			return fail(e)
		}
		if _, e = conn.ExecContext(context.Background(), "BEGIN IMMEDIATE"); e == nil {
			e = validate(conn, true)
			if e == nil {
				_, e = conn.ExecContext(context.Background(), "COMMIT")
			} else {
				conn.ExecContext(context.Background(), "ROLLBACK")
			}
		}
		conn.Close()
		if e != nil {
			return fail(e)
		}
		if _, err = db.Exec("PRAGMA journal_mode=WAL"); err != nil {
			return fail(err)
		}
		if _, err = db.Exec("PRAGMA synchronous=FULL"); err != nil {
			return fail(err)
		}
	}
	return &Store{db: db, role: role, revisions: map[string]int64{}}, nil
}
func validate(q queryer, create bool) error {
	var id, version, count int
	ctx := context.Background()
	if err := q.QueryRowContext(ctx, "PRAGMA application_id").Scan(&id); err != nil {
		return err
	}
	if err := q.QueryRowContext(ctx, "PRAGMA user_version").Scan(&version); err != nil {
		return err
	}
	if id == applicationID && version == 1 {
		return nil
	}
	if create && id == 0 && version == 0 {
		if err := q.QueryRowContext(ctx, "SELECT count(*) FROM sqlite_master WHERE name NOT LIKE 'sqlite_%'").Scan(&count); err != nil {
			return err
		}
		if count == 0 {
			_, err := q.ExecContext(ctx, schema)
			return err
		}
	}
	return fmt.Errorf("unsupported or unrelated configuration database (application=%d version=%d)", id, version)
}
func (s *Store) Close() error { return s.db.Close() }
func normalize(v any) (any, error) {
	b, e := json.Marshal(v)
	if e != nil {
		return nil, e
	}
	out, err := decodeJSON(b)
	if err != nil {
		return nil, err
	}
	return restoreFloatKinds(reflect.ValueOf(v), out), nil
}

// encoding/json emits integral floats as integers. Restore their source type.
func restoreFloatKinds(v reflect.Value, out any) any {
	for v.IsValid() && (v.Kind() == reflect.Pointer || v.Kind() == reflect.Interface) {
		if v.IsNil() {
			return out
		}
		v = v.Elem()
	}
	if !v.IsValid() {
		return out
	}
	switch v.Kind() {
	case reflect.Float32, reflect.Float64:
		return v.Float()
	case reflect.Map:
		if m, ok := out.(map[string]any); ok && v.Type().Key().Kind() == reflect.String {
			it := v.MapRange()
			for it.Next() {
				key := it.Key().String()
				if old, yes := m[key]; yes {
					m[key] = restoreFloatKinds(it.Value(), old)
				}
			}
		}
	case reflect.Slice, reflect.Array:
		if list, ok := out.([]any); ok {
			for i := range list {
				list[i] = restoreFloatKinds(v.Index(i), list[i])
			}
		}
	case reflect.Struct:
		if m, ok := out.(map[string]any); ok {
			for i := 0; i < v.NumField(); i++ {
				f := v.Type().Field(i)
				if f.PkgPath != "" {
					continue
				}
				name := strings.Split(f.Tag.Get("json"), ",")[0]
				if name == "-" {
					continue
				}
				if name == "" {
					name = f.Name
				}
				if old, yes := m[name]; yes {
					m[name] = restoreFloatKinds(v.Field(i), old)
				}
			}
		}
	}
	return out
}
func decodeJSON(b []byte) (any, error) {
	d := json.NewDecoder(bytes.NewReader(b))
	d.UseNumber()
	var v any
	if e := d.Decode(&v); e != nil {
		return nil, e
	}
	var extra any
	if e := d.Decode(&extra); e != io.EOF {
		return nil, fmt.Errorf("configuration has trailing content")
	}
	return v, nil
}
func assign(dst, v any) error {
	switch target := dst.(type) {
	case *any:
		*target = v
		return nil
	case *map[string]any:
		if m, ok := v.(map[string]any); ok {
			*target = m
			return nil
		}
	case *[]any:
		if list, ok := v.([]any); ok {
			*target = list
			return nil
		}
	}
	if dst == nil {
		return nil
	}
	b, e := json.Marshal(v)
	if e != nil {
		return e
	}
	d := json.NewDecoder(bytes.NewReader(b))
	d.UseNumber()
	return d.Decode(dst)
}
func (s *Store) Load(key string, dst any) (bool, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	delete(s.revisions, key)
	tx, e := s.db.BeginTx(context.Background(), &sql.TxOptions{ReadOnly: true})
	if e != nil {
		return false, e
	}
	defer tx.Rollback()
	v, _, rev, _, e := read(tx, s.role, key)
	if e != nil {
		delete(s.revisions, key)
		return false, e
	}
	if rev > 0 {
		if e = assign(dst, v); e != nil {
			delete(s.revisions, key)
			return false, e
		}
	}
	if e = tx.Commit(); e != nil {
		delete(s.revisions, key)
		return false, e
	}
	s.revisions[key] = rev
	return rev > 0, nil
}
func read(q queryer, role, key string) (any, int64, int64, map[int64]cell, error) {
	ctx := context.Background()
	var id, rev int64
	e := q.QueryRowContext(ctx, "SELECT id,revision FROM document WHERE role=? AND key=?", role, key).Scan(&id, &rev)
	if e == sql.ErrNoRows {
		return nil, 0, 0, nil, nil
	}
	if e != nil {
		return nil, 0, 0, nil, e
	}
	rows, e := q.QueryContext(ctx, "SELECT node_id,parent_id,slot,kind,value FROM node WHERE document_id=?", id)
	if e != nil {
		return nil, 0, 0, nil, e
	}
	cells := map[int64]cell{}
	children := map[int64][]cell{}
	for rows.Next() {
		var c cell
		if e = rows.Scan(&c.id, &c.parent, &c.slot, &c.kind, &c.value); e != nil {
			rows.Close()
			return nil, 0, 0, nil, e
		}
		cells[c.id] = c
		if c.id != 0 {
			children[c.parent] = append(children[c.parent], c)
		}
	}
	e = rows.Err()
	rows.Close()
	if e != nil {
		return nil, 0, 0, nil, e
	}
	sqlCells := cells
	cells = make(map[int64]cell, len(sqlCells))
	for id, c := range sqlCells {
		cells[id] = c
	}
	virtual := int64(-2)
	for id, c := range sqlCells {
		if c.kind == 7 || c.kind == 8 {
			if c.value != nil {
				packed, ok := c.value.([]byte)
				if !ok {
					return nil, 0, 0, nil, fmt.Errorf("invalid packed container")
				}
				leaves, err := unpackScalars(packed)
				if err != nil {
					return nil, 0, 0, nil, err
				}
				for _, leaf := range leaves {
					leaf.id = virtual
					leaf.parent = id
					virtual--
					cells[leaf.id] = leaf
					children[id] = append(children[id], leaf)
				}
				c.value = nil
				cells[id] = c
			}
		}
	}
	fields, e := readFields(q)
	if e != nil {
		return nil, 0, 0, nil, e
	}
	visited := map[int64]bool{}
	var restore func(int64, int) (any, error)
	restore = func(n int64, depth int) (any, error) {
		if depth > 256 || visited[n] {
			return nil, fmt.Errorf("invalid configuration node graph")
		}
		visited[n] = true
		c, ok := cells[n]
		if !ok {
			return nil, fmt.Errorf("missing configuration node")
		}
		if (c.kind <= 2 || c.kind == 7 || c.kind == 8) && c.value != nil {
			return nil, fmt.Errorf("invalid null/container scalar")
		}
		switch c.kind {
		case 0:
			return nil, nil
		case 1:
			return false, nil
		case 2:
			return true, nil
		case 3:
			v, ok := c.value.(int64)
			if !ok {
				return nil, fmt.Errorf("invalid signed integer")
			}
			return json.Number(strconv.FormatInt(v, 10)), nil
		case 4:
			v, ok := c.value.([]byte)
			if !ok || len(v) != 8 {
				return nil, fmt.Errorf("invalid unsigned integer")
			}
			return json.Number(strconv.FormatUint(binary.BigEndian.Uint64(v), 10)), nil
		case 5:
			v, ok := c.value.(float64)
			if !ok {
				if iv, yes := c.value.(int64); yes {
					v = float64(iv)
				} else {
					return nil, fmt.Errorf("invalid floating point value")
				}
			}
			if math.IsNaN(v) || math.IsInf(v, 0) {
				return nil, fmt.Errorf("nonfinite configuration value")
			}
			return v, nil
		case 6:
			v, ok := c.value.(string)
			if !ok {
				return nil, fmt.Errorf("invalid text value")
			}
			return v, nil
		case 7:
			list := children[n]
			sort.Slice(list, func(i, j int) bool { return list[i].slot > list[j].slot })
			out := make([]any, len(list))
			for i, ch := range list {
				if ch.slot != -int64(i)-1 {
					return nil, fmt.Errorf("invalid array ordering")
				}
				v, e := restore(ch.id, depth+1)
				if e != nil {
					return nil, e
				}
				out[i] = v
			}
			return out, nil
		case 8:
			out := map[string]any{}
			for _, ch := range children[n] {
				name, ok := fields[ch.slot]
				if !ok || ch.slot <= 0 {
					return nil, fmt.Errorf("invalid object field")
				}
				if _, dup := out[name]; dup {
					return nil, fmt.Errorf("duplicate object field")
				}
				v, e := restore(ch.id, depth+1)
				if e != nil {
					return nil, e
				}
				out[name] = v
			}
			return out, nil
		}
		return nil, fmt.Errorf("unknown configuration type")
	}
	root, ok := cells[0]
	if !ok || root.parent != -1 || root.slot != 0 {
		return nil, 0, 0, nil, fmt.Errorf("invalid configuration root")
	}
	v, e := restore(0, 0)
	if e == nil && len(visited) != len(cells) {
		e = fmt.Errorf("orphan configuration records")
	}
	return v, id, rev, sqlCells, e
}
func readFields(q queryer) (map[int64]string, error) {
	rows, e := q.QueryContext(context.Background(), "SELECT id,name FROM field")
	if e != nil {
		return nil, e
	}
	defer rows.Close()
	out := map[int64]string{}
	for rows.Next() {
		var id int64
		var n string
		if e = rows.Scan(&id, &n); e != nil {
			return nil, e
		}
		out[id] = n
	}
	return out, rows.Err()
}
func (s *Store) Save(key string, value any) error {
	s.mu.Lock()
	defer s.mu.Unlock()
	expected, ok := s.revisions[key]
	if !ok {
		return fmt.Errorf("configuration must be loaded before saving")
	}
	v, e := normalize(value)
	if e != nil {
		return e
	}
	rev, e := s.write(key, v, expected, "", "")
	if e == nil {
		s.revisions[key] = rev
	}
	return e
}
func (s *Store) write(key string, v any, expected int64, source, digest string) (int64, error) {
	ctx := context.Background()
	conn, e := s.db.Conn(ctx)
	if e != nil {
		return 0, e
	}
	defer conn.Close()
	if _, e = conn.ExecContext(ctx, "BEGIN IMMEDIATE"); e != nil {
		return 0, e
	}
	defer conn.ExecContext(ctx, "ROLLBACK")
	_, id, rev, old, e := read(conn, s.role, key)
	if e != nil {
		return 0, e
	}
	if rev != expected {
		return 0, ErrConflict
	}
	if rev == 0 {
		result, e := conn.ExecContext(ctx, "INSERT INTO document(role,key,revision) VALUES(?,?,1)", s.role, key)
		if e != nil {
			return 0, e
		}
		id, e = result.LastInsertId()
		if e != nil {
			return 0, e
		}
	}
	allFields, e := readFields(conn)
	if e != nil {
		return 0, e
	}
	fieldIDs := map[string]int64{}
	for id, name := range allFields {
		fieldIDs[name] = id
	}
	nextID := int64(0)
	edges := map[[2]int64]int64{}
	for id, c := range old {
		if id > nextID {
			nextID = id
		}
		edges[[2]int64{c.parent, c.slot}] = id
	}
	nodes := map[int64]cell{}
	var flatten func(any, int64, int64, int64, int) error
	flatten = func(value any, id, parent, slot int64, depth int) error {
		if depth > 256 {
			return fmt.Errorf("configuration nesting exceeds 256")
		}
		c := cell{id: id, parent: parent, slot: slot}
		var packed []cell
		child := func(value any, slot int64) error {
			switch value.(type) {
			case []any, map[string]any:
			default:
				leaf, e := scalarCell(value)
				if e != nil {
					return e
				}
				leaf.slot = slot
				packed = append(packed, leaf)
				return nil
			}
			cid, ok := edges[[2]int64{id, slot}]
			if !ok {
				nextID++
				cid = nextID
			}
			return flatten(value, cid, id, slot, depth+1)
		}
		switch x := value.(type) {
		case nil:
			c.kind = 0
		case bool:
			if x {
				c.kind = 2
			} else {
				c.kind = 1
			}
		case string:
			c.kind = 6
			c.value = x
		case json.Number:
			raw := string(x)
			if strings.ContainsAny(raw, ".eE") || raw == "-0" {
				f, e := strconv.ParseFloat(raw, 64)
				if e != nil || math.IsInf(f, 0) || math.IsNaN(f) {
					return fmt.Errorf("invalid number %s", raw)
				}
				c.kind = 5
				c.value = f
			} else if i, e := strconv.ParseInt(raw, 10, 64); e == nil {
				c.kind = 3
				c.value = i
			} else {
				u, e := strconv.ParseUint(raw, 10, 64)
				if e != nil {
					return e
				}
				c.kind = 4
				b := make([]byte, 8)
				binary.BigEndian.PutUint64(b, u)
				c.value = b
			}
		case float64:
			if math.IsNaN(x) || math.IsInf(x, 0) {
				return fmt.Errorf("nonfinite configuration value")
			}
			c.kind = 5
			c.value = x
		case []any:
			c.kind = 7
			for i, val := range x {
				if e := child(val, -int64(i)-1); e != nil {
					return e
				}
			}
		case map[string]any:
			c.kind = 8
			names := make([]string, 0, len(x))
			for n := range x {
				names = append(names, n)
			}
			sort.Strings(names)
			for _, name := range names {
				fid, ok := fieldIDs[name]
				if !ok {
					if _, e := conn.ExecContext(ctx, "INSERT OR IGNORE INTO field(name) VALUES(?)", name); e != nil {
						return e
					}
					if e := conn.QueryRowContext(ctx, "SELECT id FROM field WHERE name=?", name).Scan(&fid); e != nil {
						return e
					}
					fieldIDs[name] = fid
				}
				if e := child(x[name], fid); e != nil {
					return e
				}
			}
		default:
			return fmt.Errorf("unsupported configuration value %T", value)
		}
		if len(packed) > 0 {
			c.value = packScalars(packed)
		}
		nodes[id] = c
		return nil
	}
	if e = flatten(v, 0, -1, 0, 0); e != nil {
		return 0, e
	}
	changed := rev == 0
	for nid := range old {
		if _, ok := nodes[nid]; !ok {
			if _, e = conn.ExecContext(ctx, "DELETE FROM node WHERE document_id=? AND node_id=?", id, nid); e != nil {
				return 0, e
			}
			changed = true
		}
	}
	ids := make([]int64, 0, len(nodes))
	for nid := range nodes {
		ids = append(ids, nid)
	}
	sort.Slice(ids, func(i, j int) bool { return ids[i] < ids[j] })
	for _, nid := range ids {
		c := nodes[nid]
		if prev, ok := old[nid]; ok && reflect.DeepEqual(prev, c) {
			continue
		}
		if _, e = conn.ExecContext(ctx, "INSERT INTO node(document_id,node_id,parent_id,slot,kind,value) VALUES(?,?,?,?,?,?) ON CONFLICT(document_id,node_id) DO UPDATE SET parent_id=excluded.parent_id,slot=excluded.slot,kind=excluded.kind,value=excluded.value", id, nid, c.parent, c.slot, c.kind, c.value); e != nil {
			return 0, e
		}
		changed = true
	}
	if rev == 0 {
		rev = 1
	} else if changed {
		rev++
		if _, e = conn.ExecContext(ctx, "UPDATE document SET revision=? WHERE id=?", rev, id); e != nil {
			return 0, e
		}
	}
	if source != "" {
		if _, e = conn.ExecContext(ctx, "INSERT INTO migration(role,key,source,sha256) VALUES(?,?,?,?)", s.role, key, source, digest); e != nil {
			return 0, e
		}
	}
	if _, e = conn.ExecContext(ctx, "COMMIT"); e != nil {
		return 0, e
	}
	return rev, nil
}

// MigrateJSON imports a retained legacy file only while this document is absent.
// A nil payload captures absence without creating a document.
func (s *Store) MigrateJSON(key string, data []byte, source string) (bool, error) {
	var existing any
	present, err := s.Load(key, &existing)
	if err != nil || present || data == nil {
		return present, err
	}
	value, err := decodeJSON(data)
	if err != nil {
		s.mu.Lock()
		delete(s.revisions, key)
		s.mu.Unlock()
		return false, err
	}
	digest := sha256.Sum256(data)
	s.mu.Lock()
	rev, err := s.write(key, value, 0, source, hex.EncodeToString(digest[:]))
	if err == nil {
		s.revisions[key] = rev
	} else {
		delete(s.revisions, key)
	}
	s.mu.Unlock()
	if errors.Is(err, ErrConflict) {
		return s.Load(key, nil)
	}
	return err == nil, err
}

// ReadJSON reads the current document without retaining a write session.
func ReadJSON(root, role, key string) ([]byte, bool, error) {
	s, e := Open(root, role)
	if e != nil {
		return nil, false, e
	}
	defer s.Close()
	var value any
	present, e := s.Load(key, &value)
	if e != nil || !present {
		return nil, present, e
	}
	data, e := json.Marshal(value)
	return data, true, e
}

// ExportProject writes one validated role/document as a portable SQLite file.
func ExportProject(path, role, key string, value any) error {
	if !validRole(role) || key == "" {
		return fmt.Errorf("invalid project role or key")
	}
	if name := strings.ToLower(filepath.Base(path)); name == "saio.db" || name == "saio.db-wal" || name == "saio.db-shm" || name == "saio.db-journal" {
		return fmt.Errorf("project export cannot replace the shared configuration database")
	}
	if err := os.MkdirAll(filepath.Dir(path), 0700); err != nil {
		return err
	}
	temp, e := os.CreateTemp(filepath.Dir(path), ".saio-project-*")
	if e != nil {
		return e
	}
	name := temp.Name()
	temp.Close()
	defer os.Remove(name)
	s, e := openFile(name, role, false)
	if e != nil {
		return e
	}
	_, e = s.Load(key, nil)
	if e == nil {
		e = s.Save(key, value)
	}
	if e == nil {
		_, e = s.db.Exec("PRAGMA wal_checkpoint(TRUNCATE)")
	}
	if e == nil {
		_, e = s.db.Exec("PRAGMA journal_mode=DELETE")
	}
	closeErr := s.Close()
	if e != nil {
		return e
	}
	if closeErr != nil {
		return closeErr
	}
	return os.Rename(name, path)
}

// ReadProject accepts the selected SQLite document or a legacy JSON document.
func ReadProject(path, role, key string, dst any) error {
	if !validRole(role) || key == "" {
		return fmt.Errorf("invalid project role or key")
	}
	data, e := os.ReadFile(path)
	if e != nil {
		return e
	}
	if !bytes.HasPrefix(data, []byte("SQLite format 3\x00")) {
		value, e := decodeJSON(data)
		if e != nil {
			return e
		}
		return assign(dst, value)
	}
	s, e := openFile(path, role, true)
	if e != nil {
		return e
	}
	defer s.Close()
	var count int
	if e = s.db.QueryRow("SELECT count(*) FROM document").Scan(&count); e != nil {
		return e
	}
	if count != 1 {
		return fmt.Errorf("project must contain exactly one document")
	}
	var value any
	present, e := s.Load(key, &value)
	if e != nil {
		return e
	}
	if !present {
		return fmt.Errorf("project role or document does not match")
	}
	return assign(dst, value)
}

func scalarCell(value any) (cell, error) {
	c := cell{}
	switch x := value.(type) {
	case nil:
		c.kind = 0
	case bool:
		if x {
			c.kind = 2
		} else {
			c.kind = 1
		}
	case string:
		c.kind = 6
		c.value = x
	case json.Number:
		raw := string(x)
		if strings.ContainsAny(raw, ".eE") || raw == "-0" {
			f, e := strconv.ParseFloat(raw, 64)
			if e != nil || math.IsInf(f, 0) || math.IsNaN(f) {
				return cell{}, fmt.Errorf("invalid number %s", raw)
			}
			c.kind = 5
			c.value = f
		} else if i, e := strconv.ParseInt(raw, 10, 64); e == nil {
			c.kind = 3
			c.value = i
		} else {
			u, e := strconv.ParseUint(raw, 10, 64)
			if e != nil {
				return cell{}, e
			}
			c.kind = 4
			b := make([]byte, 8)
			binary.BigEndian.PutUint64(b, u)
			c.value = b
		}
	case float64:
		if math.IsNaN(x) || math.IsInf(x, 0) {
			return cell{}, fmt.Errorf("nonfinite configuration value")
		}
		c.kind = 5
		c.value = x
	default:
		return cell{}, fmt.Errorf("invalid packed scalar %T", value)
	}
	return c, nil
}
func packScalars(cells []cell) []byte {
	sort.Slice(cells, func(i, j int) bool { return cells[i].slot < cells[j].slot })
	out := []byte{'S', 1}
	var scratch [10]byte
	for _, c := range cells {
		n := binary.PutVarint(scratch[:], c.slot)
		out = append(out, scratch[:n]...)
		out = append(out, byte(c.kind))
		switch c.kind {
		case 3:
			n = binary.PutVarint(scratch[:], c.value.(int64))
			out = append(out, scratch[:n]...)
		case 4:
			out = append(out, c.value.([]byte)...)
		case 5:
			var b [8]byte
			binary.BigEndian.PutUint64(b[:], math.Float64bits(c.value.(float64)))
			out = append(out, b[:]...)
		case 6:
			v := []byte(c.value.(string))
			n = binary.PutUvarint(scratch[:], uint64(len(v)))
			out = append(out, scratch[:n]...)
			out = append(out, v...)
		}
	}
	return out
}
func unpackScalars(data []byte) ([]cell, error) {
	fail := func() ([]cell, error) { return nil, fmt.Errorf("invalid packed scalar record") }
	if len(data) < 2 || data[0] != 'S' || data[1] != 1 {
		return fail()
	}
	data = data[2:]
	var out []cell
	seen := map[int64]bool{}
	for len(data) > 0 {
		slot, n := binary.Varint(data)
		if n <= 0 || slot == 0 || seen[slot] {
			return fail()
		}
		seen[slot] = true
		data = data[n:]
		if len(data) == 0 {
			return fail()
		}
		c := cell{slot: slot, kind: int(data[0])}
		data = data[1:]
		switch c.kind {
		case 0, 1, 2:
		case 3:
			v, n := binary.Varint(data)
			if n <= 0 {
				return fail()
			}
			c.value = v
			data = data[n:]
		case 4:
			if len(data) < 8 {
				return fail()
			}
			c.value = append([]byte{}, data[:8]...)
			data = data[8:]
		case 5:
			if len(data) < 8 {
				return fail()
			}
			c.value = math.Float64frombits(binary.BigEndian.Uint64(data))
			data = data[8:]
		case 6:
			size, n := binary.Uvarint(data)
			if n <= 0 || size > uint64(len(data)-n) {
				return fail()
			}
			data = data[n:]
			if !utf8.Valid(data[:int(size)]) {
				return fail()
			}
			c.value = string(data[:int(size)])
			data = data[int(size):]
		default:
			return fail()
		}
		out = append(out, c)
	}
	return out, nil
}
