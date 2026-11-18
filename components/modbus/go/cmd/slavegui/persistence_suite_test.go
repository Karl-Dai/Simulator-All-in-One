//go:build simlab_suite

package main

import (
	"context"
	"encoding/json"
	"github.com/Karl-Dai/ModbusSim/go/internal/appstate"
	"github.com/Karl-Dai/ModbusSim/go/internal/configstore"
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"path/filepath"
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
func TestSuiteSlaveRestartAndLegacyImport(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	first := persistedApp(t)
	info, err := first.CreateConnection(appstate.CreateConnectionRequest{Transport: project.Transport{Type: project.TransportTCP, Host: "127.0.0.1", Port: 15020}, SlaveID: 1, RegisterRanges: []appstate.RegisterRange{}})
	if err != nil {
		t.Fatal(err)
	}
	if _, err = first.AddDevice(appstate.AddDeviceRequest{ConnectionID: info.ID, SlaveID: 2, Name: "saved"}); err != nil {
		t.Fatal(err)
	}
	restarted := persistedApp(t)
	rows := restarted.ListConnections()
	if len(rows) != 1 || rows[0].State != appstate.Stopped {
		t.Fatalf("restart: %+v", rows)
	}
	devices, err := restarted.ListDevices(info.ID)
	if err != nil || len(devices) != 2 {
		t.Fatalf("devices: %+v %v", devices, err)
	}
	legacy := filepath.Join(root, "old.modbusproj")
	if err = project.SaveFile(legacy, first.manager.Project()); err != nil {
		t.Fatal(err)
	}
	if _, err = first.LoadProject(legacy); err != nil {
		t.Fatal(err)
	}
	export := filepath.Join(root, "slave.saioproj")
	if err = first.SaveProject(export); err != nil {
		t.Fatal(err)
	}
	var saved project.Project
	// The master role cannot import the slave role's SQLite document.
	if err = configstore.ReadProject(export, "modbus.master", "workspace", &saved); err == nil {
		t.Fatal("cross-role project accepted")
	}
}

func TestSuiteLegacyV1AndInvalidImportPreserveWorkspace(t *testing.T) {
	t.Setenv("SIMLAB_DATA_DIR", t.TempDir())
	a := persistedApp(t)
	legacy := filepath.Join("..", "..", "internal", "project", "testdata", "v1-slave-legacy-blocks.json")
	if _, err := a.LoadProject(legacy); err != nil {
		t.Fatal(err)
	}
	before := a.manager.Project()
	if len(before.Connections) == 0 || len(before.Connections[0].Devices) == 0 {
		t.Fatal("fixture did not restore devices")
	}
	original, err := json.Marshal(before)
	if err != nil {
		t.Fatal(err)
	}
	before.Connections[0].Devices = append(before.Connections[0].Devices, before.Connections[0].Devices[0])
	invalid := filepath.Join(t.TempDir(), "duplicate.modbusproj")
	if err = project.SaveFile(invalid, before); err != nil {
		t.Fatal(err)
	}
	if _, err = a.LoadProject(invalid); err == nil {
		t.Fatal("duplicate slave id accepted")
	}
	current, _ := json.Marshal(a.manager.Project())
	if string(current) != string(original) {
		t.Fatal("failed import replaced live workspace")
	}
	restarted := persistedApp(t)
	saved, _ := json.Marshal(restarted.manager.Project())
	if string(saved) != string(original) {
		t.Fatal("failed import changed durable workspace")
	}
}
