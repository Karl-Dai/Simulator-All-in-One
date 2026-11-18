//go:build simlab_suite

package main

import (
	"encoding/json"
	"fmt"
	"github.com/Karl-Dai/Simulator-All-in-One/storage/go"
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"github.com/Karl-Dai/ModbusSim/go/internal/suitehost"
	"io"
	"os"
	"path/filepath"
)

const projectFileExtension = ".saioproj"
const storageRole = "modbus.master"

func (a *App) openPersistence() {
	if a.config.store != nil {
		_ = a.config.store.Close()
		a.config.store = nil
	}
	dir, err := suitehost.DataDir("master")
	if err != nil {
		a.config.err = err
		return
	}
	store, err := configstore.Open(filepath.Dir(dir), storageRole)
	if err != nil {
		a.config.err = err
		return
	}
	a.config.store = store
	var saved project.Project
	present, err := store.Load("workspace", &saved)
	if err == nil && present {
		_, err = a.manager.ApplyProject(&saved)
	}
	a.config.err = err
}
func (a *App) initializePersistence() {
	a.config.mu.Lock()
	defer a.config.mu.Unlock()
	a.openPersistence()
}
func (a *App) LoadPreferences(legacy map[string]any) (map[string]any, error) {
	a.config.mu.Lock()
	defer a.config.mu.Unlock()
	if a.config.err != nil || a.config.store == nil {
		a.openPersistence()
	}
	if a.config.err != nil {
		return nil, a.config.err
	}
	data, err := json.Marshal(legacy)
	if err != nil {
		return nil, err
	}
	if _, err = a.config.store.MigrateJSON("ui", data, "webview-localStorage-v1"); err != nil {
		a.config.err = err
		return nil, err
	}
	var values map[string]any
	if _, err = a.config.store.Load("ui", &values); err != nil {
		a.config.err = err
		return nil, err
	}
	if values == nil {
		values = map[string]any{}
	}
	return values, nil
}
func (a *App) SavePreferences(values map[string]any) error {
	a.config.mu.Lock()
	defer a.config.mu.Unlock()
	if a.config.err != nil {
		return a.config.err
	}
	if a.config.store == nil {
		return fmt.Errorf("configuration is not initialized")
	}
	return a.config.store.Save("ui", values)
}
func (a *App) readProject(path string) (int, error) {
	file, err := os.Open(path)
	if err != nil {
		return 0, err
	}
	header := make([]byte, 16)
	_, readErr := io.ReadFull(file, header)
	_ = file.Close()
	if readErr != nil && readErr != io.ErrUnexpectedEOF && readErr != io.EOF {
		return 0, readErr
	}
	var saved *project.Project
	if string(header) == "SQLite format 3\x00" {
		saved = &project.Project{}
		err = configstore.ReadProject(path, storageRole, "workspace", saved)
	} else {
		saved, err = project.LoadFile(path)
	}
	if err != nil {
		return 0, err
	}
	return changeValue(a, nil, func() (int, error) { return a.manager.ApplyProject(saved) })
}
func (a *App) writeProject(path string) error {
	a.config.mu.Lock()
	defer a.config.mu.Unlock()
	if a.config.err != nil {
		return a.config.err
	}
	if filepath.Ext(path) != ".saioproj" {
		return fmt.Errorf("export requires a .saioproj file; legacy imports are kept unchanged")
	}
	saved, err := a.snapshot()
	if err != nil {
		return err
	}
	return configstore.ExportProject(path, storageRole, "workspace", saved)
}
