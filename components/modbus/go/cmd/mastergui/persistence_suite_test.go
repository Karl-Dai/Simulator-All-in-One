//go:build simlab_suite

package main

import (
	"context"
	"github.com/Karl-Dai/Simulator-All-in-One/storage/go"
	"github.com/Karl-Dai/ModbusSim/go/internal/masterstate"
	"os"
	"path/filepath"
	"strings"
	"testing"
)

func persistedApp(t *testing.T) *App {
	t.Helper()
	a := newApp()
	a.initializePersistence()
	if a.config.err != nil {
		t.Fatal(a.config.err)
	}
	t.Cleanup(func() { a.shutdown(context.Background()) })
	return a
}
func TestSuiteWorkspaceRestartConflictAndProject(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	first := persistedApp(t)
	stale := persistedApp(t)
	info, err := first.CreateConnection(masterstate.DefaultSettings())
	if err != nil {
		t.Fatal(err)
	}
	if _, err = stale.CreateConnection(masterstate.DefaultSettings()); err == nil || !strings.Contains(err.Error(), "restored") {
		t.Fatalf("stale write: %v", err)
	}
	if len(stale.ListConnections()) != 0 {
		t.Fatal("stale mutation remained in memory")
	}
	restarted := persistedApp(t)
	rows := restarted.ListConnections()
	if len(rows) != 1 || rows[0].ID != info.ID || rows[0].State != "Disconnected" {
		t.Fatalf("restart: %+v", rows)
	}
	path := filepath.Join(root, "master.saioproj")
	if err = first.SaveProject(path); err != nil {
		t.Fatal(err)
	}
	bytes, err := os.ReadFile(path)
	if err != nil || !strings.HasPrefix(string(bytes), "SQLite format 3") {
		t.Fatalf("export is not SQLite: %v", err)
	}
	if _, err = first.LoadProject(path); err != nil {
		t.Fatal(err)
	}
	if err = first.SaveProject(filepath.Join(root, "legacy.modbusproj")); err == nil {
		t.Fatal("overwrote a legacy extension")
	}
}
func TestSuiteRestoreErrorBlocksDefaultWrites(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	path := filepath.Join(root, "saio.db")
	original := []byte("broken database")
	if err := os.WriteFile(path, original, 0600); err != nil {
		t.Fatal(err)
	}
	a := newApp()
	a.initializePersistence()
	defer a.shutdown(context.Background())
	if a.config.err == nil {
		t.Fatal("accepted broken database")
	}
	if _, err := a.CreateConnection(masterstate.DefaultSettings()); err == nil {
		t.Fatal("accepted mutation after failed restore")
	}
	bytes, _ := os.ReadFile(path)
	if string(bytes) != string(original) {
		t.Fatal("overwrote corrupt data with defaults")
	}
}

func TestSuiteImportsLegacyV1UsingExistingConverter(t *testing.T) {
	t.Setenv("SIMLAB_DATA_DIR", t.TempDir())
	a := persistedApp(t)
	fixture := filepath.Join("..", "..", "internal", "project", "testdata", "v1-master-saved.json")
	bytes, err := os.ReadFile(fixture)
	if err != nil {
		t.Fatal(err)
	}
	path := filepath.Join(t.TempDir(), "legacy.modbusproj")
	if err = os.WriteFile(path, bytes, 0600); err != nil {
		t.Fatal(err)
	}
	if _, err = a.LoadProject(path); err != nil {
		t.Fatal(err)
	}
	if len(a.ListConnections()) == 0 {
		t.Fatal("v1 connections were lost")
	}
	retained, _ := os.ReadFile(path)
	if string(retained) != string(bytes) {
		t.Fatal("legacy source changed")
	}
}

func TestSuiteRejectsUnsupportedSQLiteProjectVersion(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	a := persistedApp(t)
	if _, err := a.CreateConnection(masterstate.DefaultSettings()); err != nil {
		t.Fatal(err)
	}
	saved, err := a.manager.Project()
	if err != nil {
		t.Fatal(err)
	}
	saved.Version = 99
	path := filepath.Join(root, "future.saioproj")
	if err = configstore.ExportProject(path, "modbus.master", "workspace", saved); err != nil {
		t.Fatal(err)
	}
	if _, err = a.LoadProject(path); err == nil || !strings.Contains(err.Error(), "unsupported project version") {
		t.Fatalf("version error: %v", err)
	}
	if len(a.ListConnections()) != 1 {
		t.Fatal("unsupported project replaced workspace")
	}
}
