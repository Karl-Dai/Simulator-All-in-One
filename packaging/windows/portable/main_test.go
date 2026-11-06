package main

import (
	"archive/zip"
	"bytes"
	"crypto/sha256"
	"fmt"
	"os"
	"path/filepath"
	"sync"
	"testing"
)

func fixture(t *testing.T, name string) ([]byte, map[string]fileRecord) {
	t.Helper()
	var buf bytes.Buffer
	w := zip.NewWriter(&buf)
	f, err := w.Create(name)
	if err != nil {
		t.Fatal(err)
	}
	data := []byte("fixture payload")
	f.Write(data)
	w.Close()
	return buf.Bytes(), map[string]fileRecord{name: {fmt.Sprintf("%x", sha256.Sum256(data)), int64(len(data))}}
}
func TestExtractReuseAndCorruption(t *testing.T) {
	archive, index := fixture(t, "apps/example.exe")
	parent := t.TempDir()
	dir, err := prepare(parent, archive, index)
	if err != nil {
		t.Fatal(err)
	}
	second, err := prepare(parent, archive, index)
	if err != nil || second != dir {
		t.Fatalf("reuse: %s %v", second, err)
	}
	file := filepath.Join(dir, "apps/example.exe")
	os.WriteFile(file, []byte("changed payload"), 0600)
	if _, err = prepare(parent, archive, index); err != nil {
		t.Fatal(err)
	}
	if got, _ := os.ReadFile(file); string(got) != "fixture payload" {
		t.Fatal("cache was not repaired")
	}
	backups, _ := filepath.Glob(dir + ".damaged-*")
	if len(backups) != 1 {
		t.Fatal("damaged cache was not preserved")
	}
}
func TestTraversalRejected(t *testing.T) {
	for _, name := range []string{"../escape.exe", "/absolute.exe", "C:/escape.exe", "apps/../escape.exe", "apps\\escape.exe"} {
		archive, index := fixture(t, name)
		if _, err := prepare(t.TempDir(), archive, index); err == nil {
			t.Fatalf("accepted %s", name)
		}
	}
}
func TestConcurrentExtraction(t *testing.T) {
	archive, index := fixture(t, "SAIO.exe")
	parent := t.TempDir()
	var wg sync.WaitGroup
	for i := 0; i < 8; i++ {
		wg.Add(1)
		go func() {
			defer wg.Done()
			if _, err := prepare(parent, archive, index); err != nil {
				t.Error(err)
			}
		}()
	}
	wg.Wait()
}
func TestIncompleteArchiveAndBadHash(t *testing.T) {
	archive, index := fixture(t, "SAIO.exe")
	index["missing.exe"] = index["SAIO.exe"]
	if _, err := prepare(t.TempDir(), archive, index); err == nil {
		t.Fatal("accepted incomplete archive")
	}
	delete(index, "missing.exe")
	index["SAIO.exe"] = fileRecord{"bad", 15}
	if _, err := prepare(t.TempDir(), archive, index); err == nil {
		t.Fatal("accepted invalid hash")
	}
}

func TestSymlinkAndMissingRuntimeRejected(t *testing.T) {
	archive, index := fixture(t, "apps/example.exe")
	parent := t.TempDir()
	dir, err := prepare(parent, archive, index)
	if err != nil {
		t.Fatal(err)
	}
	file := filepath.Join(dir, "apps/example.exe")
	os.Remove(file)
	if _, err = prepare(parent, archive, index); err != nil {
		t.Fatal(err)
	}
	os.Remove(file)
	target := filepath.Join(t.TempDir(), "target")
	os.WriteFile(target, []byte("fixture payload"), 0600)
	if err = os.Symlink(target, file); err != nil {
		t.Skip(err)
	}
	if _, err = prepare(parent, archive, index); err != nil {
		t.Fatal(err)
	}
	if info, _ := os.Lstat(file); !info.Mode().IsRegular() {
		t.Fatal("symlink was not repaired")
	}
}

func TestUndeclaredDLLIsNotUsed(t *testing.T) {
	archive, index := fixture(t, "SAIO.exe")
	parent := t.TempDir()
	dir, err := prepare(parent, archive, index)
	if err != nil {
		t.Fatal(err)
	}
	os.WriteFile(filepath.Join(dir, "unknown.dll"), []byte("unexpected"), 0600)
	if _, err = prepare(parent, archive, index); err != nil {
		t.Fatal(err)
	}
	if _, err = os.Stat(filepath.Join(dir, "unknown.dll")); !os.IsNotExist(err) {
		t.Fatal("undeclared DLL remains in executable cache")
	}
}

func TestPortableEnvironmentReplacesStaleDataDirectory(t *testing.T) {
	env := portableEnvironment([]string{"KEEP=yes", "simlab_data_dir=old"}, "new-data")
	if len(env) != 2 || env[0] != "KEEP=yes" || env[1] != "SIMLAB_DATA_DIR=new-data" {
		t.Fatal(env)
	}
}
