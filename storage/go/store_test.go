package configstore

import (
	"bytes"
	"encoding/json"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
)

func newStore(t *testing.T, root string) *Store {
	t.Helper()
	s, e := Open(root, "iec104.master")
	if e != nil {
		t.Fatal(e)
	}
	t.Cleanup(func() { s.Close() })
	return s
}
func TestSessionMigrationAndDiff(t *testing.T) {
	root := t.TempDir()
	a := newStore(t, root)
	b := newStore(t, root)
	if a.Save("workspace", nil) == nil {
		t.Fatal("save before load")
	}
	p, e := a.MigrateJSON("workspace", []byte(`{"雪":[null,true,false,-9223372036854775808,18446744073709551615,1.25,"",[],{}]}`), "old.json")
	if e != nil || !p {
		t.Fatal(p, e)
	}
	var got any
	if _, e = a.Load("workspace", &got); e != nil {
		t.Fatal(e)
	}
	if _, e = b.Load("workspace", nil); e != nil {
		t.Fatal(e)
	}
	var rootID int64
	a.db.QueryRow("SELECT node_id FROM node WHERE parent_id=-1").Scan(&rootID)
	if e = a.Save("workspace", map[string]any{"float": float64(1), "雪": []any{}}); e != nil {
		t.Fatal(e)
	}
	if e = b.Save("workspace", map[string]any{}); e != ErrConflict {
		t.Fatalf("stale writer: %v", e)
	}
	var packed []byte
	a.db.QueryRow("SELECT value FROM node WHERE node_id=0").Scan(&packed)
	leaves, e := unpackScalars(packed)
	if e != nil || len(leaves) != 1 || leaves[0].kind != 5 {
		t.Fatal("integral float type lost", leaves, e)
	}
	var count int
	a.db.QueryRow("SELECT count(*) FROM node").Scan(&count)
	if count != 2 {
		t.Fatalf("orphan nodes: %d", count)
	}
	if e = a.Save("workspace", map[string]any{}); e != nil {
		t.Fatal(e)
	}
	if _, e = a.MigrateJSON("workspace", []byte(`{"resurrect":true}`), "old.json"); e != nil {
		t.Fatal(e)
	}
	a.Load("workspace", &got)
	if len(got.(map[string]any)) != 0 {
		t.Fatal(got)
	}
	p, e = a.MigrateJSON("null", []byte(`null`), "old")
	if !p || e != nil {
		t.Fatal(p, e)
	}
	p, e = a.Load("null", &got)
	if !p || e != nil || got != nil {
		t.Fatal(p, got, e)
	}
	if _, e = a.MigrateJSON("bad", []byte(`{`), "old"); e == nil {
		t.Fatal("bad accepted")
	}
	if a.Save("bad", map[string]any{}) == nil {
		t.Fatal("bad restore allowed write")
	}
}
func TestConcurrentInitialization(t *testing.T) {
	root := t.TempDir()
	var wg sync.WaitGroup
	errs := make(chan error, 12)
	for i := 0; i < 12; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			s, e := Open(root, "launcher")
			if e == nil {
				e = s.Close()
			}
			errs <- e
		}()
	}
	wg.Wait()
	close(errs)
	for e := range errs {
		if e != nil {
			t.Fatal(e)
		}
	}
}
func TestProjectAndCorruption(t *testing.T) {
	root := t.TempDir()
	path := filepath.Join(root, "case.saioproj")
	if e := ExportProject(path, "iec104.master", "workspace", map[string]any{"n": uint64(^uint64(0))}); e != nil {
		t.Fatal(e)
	}
	var v any
	if e := ReadProject(path, "iec104.master", "workspace", &v); e != nil {
		t.Fatal(e)
	}
	if ReadProject(path, "pmu.master", "workspace", &v) == nil {
		t.Fatal("wrong role accepted")
	}
	if ExportProject(filepath.Join(root, "saio.db"), "launcher", "preferences", v) == nil {
		t.Fatal("database overwrite accepted")
	}
	os.WriteFile(filepath.Join(root, "saio.db"), []byte("broken"), 0600)
	if _, e := Open(root, "launcher"); e == nil {
		t.Fatal("corrupt accepted")
	}
}
func TestProjectRejectsSharedDatabaseNames(t *testing.T) {
	root := t.TempDir()
	store, err := Open(root, "launcher")
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()
	if _, err := store.MigrateJSON("preferences", []byte(`{"theme":"dark"}`), "legacy"); err != nil {
		t.Fatal(err)
	}
	for _, name := range []string{"SAIO.DB", "SaIo.Db-WaL", "saio.db-shm", "SAIO.DB-JOURNAL"} {
		if ExportProject(filepath.Join(root, name), "pmu.master", "workspace", nil) == nil {
			t.Fatalf("shared database name accepted: %s", name)
		}
	}
	var restored map[string]any
	if present, err := store.Load("preferences", &restored); err != nil || !present || restored["theme"] != "dark" {
		t.Fatalf("shared data changed: %v %v", restored, err)
	}
}

func TestCrossLanguage(t *testing.T) {
	if os.Getenv("SAIO_CROSS_LANGUAGE") == "" {
		t.Skip("set SAIO_CROSS_LANGUAGE=1 to compile and run Rust interoperability")
	}
	harness := filepath.Join("..", "harness", "Cargo.toml")
	run := func(args ...string) []byte {
		t.Helper()
		cmd := exec.Command("cargo", append([]string{"run", "--quiet", "--manifest-path", harness, "--"}, args...)...)
		var stderr bytes.Buffer
		cmd.Stderr = &stderr
		b, e := cmd.Output()
		if e != nil {
			t.Fatal(e, stderr.String())
		}
		return b
	}
	root := t.TempDir()
	s := newStore(t, root)
	fixture := `{"Unicode雪":"风机😀","emptyArray":[],"emptyObject":{},"nil":null,"b":true,"i":-9223372036854775808,"u":18446744073709551615,"float":1.25}`
	s.MigrateJSON("workspace", []byte(fixture), "golden")
	out := run("read", root)
	a, _ := decodeJSON([]byte(fixture))
	b, _ := decodeJSON(out)
	aa, _ := json.Marshal(a)
	bb, _ := json.Marshal(b)
	if string(aa) != string(bb) {
		t.Fatal(string(aa), string(bb))
	}
	path := filepath.Join(root, "fixture.json")
	os.WriteFile(path, []byte(strings.Replace(fixture, "1.25", "2.5", 1)), 0600)
	run("write", root, path)
	if e := s.Save("workspace", a); e != ErrConflict {
		t.Fatalf("cross-language stale writer: %v", e)
	}
	var got map[string]any
	s.Load("workspace", &got)
	if fmt.Sprint(got["float"]) != "2.5" {
		t.Fatal(got)
	}
}
func TestActualTreeSize(t *testing.T) {
	if os.Getenv("SAIO_SIZE_BENCH") == "" {
		t.Skip("set SAIO_SIZE_BENCH=1")
	}
	for _, n := range []int{1000, 10000, 100000} {
		root := t.TempDir()
		s := newStore(t, root)
		points := make([]any, n)
		for i := range points {
			points[i] = map[string]any{"ioa": i + 1, "name": fmt.Sprintf("Point %d", i+1), "type": "float", "value": float64(i) / 10, "quality": 0, "enabled": true}
		}
		value := map[string]any{"points": points}
		data, _ := json.Marshal(value)
		pretty, _ := json.MarshalIndent(value, "", "  ")
		s.Load("workspace", nil)
		if e := s.Save("workspace", value); e != nil {
			t.Fatal(e)
		}
		s.db.Exec("PRAGMA wal_checkpoint(TRUNCATE)")
		info, _ := os.Stat(filepath.Join(root, "saio.db"))
		if n >= 10000 && info.Size() >= int64(len(data)) {
			t.Fatal("packed schema exceeds compact JSON", info.Size(), len(data))
		}
		t.Logf("points=%d compact_json=%d pretty_json=%d typed_sqlite=%d", n, len(data), len(pretty), info.Size())
	}
}

func TestExportIsSingleFileAndCreatesParent(t *testing.T) {
	folder := filepath.Join(t.TempDir(), "nested")
	path := filepath.Join(folder, "case.saioproj")
	if err := ExportProject(path, "pmu.master", "workspace", map[string]any{"value": 1}); err != nil {
		t.Fatal(err)
	}
	var value any
	if err := ReadProject(path, "pmu.master", "workspace", &value); err != nil {
		t.Fatal(err)
	}
	files, err := os.ReadDir(folder)
	if err != nil || len(files) != 1 {
		t.Fatalf("project generated sidecars: %v %v", files, err)
	}
}

func TestNewDatabaseIsPrivate(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("Unix file permissions")
	}
	root := filepath.Join(t.TempDir(), "new-root")
	store, err := Open(root, "launcher")
	if err != nil {
		t.Fatal(err)
	}
	defer store.Close()
	if _, err = store.MigrateJSON("preferences", []byte(`{"schemaVersion":1}`), "test"); err != nil {
		t.Fatal(err)
	}
	directory, err := os.Stat(root)
	if err != nil || directory.Mode().Perm() != 0700 {
		t.Fatalf("root permissions: %v %v", directory, err)
	}
	for _, name := range []string{"saio.db", "saio.db-wal", "saio.db-shm"} {
		info, err := os.Stat(filepath.Join(root, name))
		if os.IsNotExist(err) {
			continue
		}
		if err != nil || info.Mode().Perm() != 0600 {
			t.Fatalf("%s permissions: %v %v", name, info, err)
		}
	}
}
