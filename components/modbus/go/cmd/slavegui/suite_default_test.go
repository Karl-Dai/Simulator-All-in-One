//go:build !simlab_suite

package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/Karl-Dai/ModbusSim/go/internal/suitehost"
	"github.com/wailsapp/wails/v2/pkg/options"
)

func TestStandaloneAppearanceBindingCannotReadSuiteSettings(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	if err := os.WriteFile(filepath.Join(root, "launcher.json"), []byte(`{"schemaVersion":1,"theme":"dark","language":"zh-CN"}`), 0600); err != nil {
		t.Fatal(err)
	}
	original := new(int)
	app := &options.App{Title: "standalone", Bind: []interface{}{original}}
	if err := configureSuite(app); err != nil {
		t.Fatal(err)
	}
	if app.Title != "standalone" || len(app.Bind) != 2 || app.Bind[0] != original {
		t.Fatalf("standalone configuration changed: %#v", app)
	}
	appearance, ok := app.Bind[1].(*suitehost.Appearance)
	if !ok {
		t.Fatalf("missing stable appearance binding: %T", app.Bind[1])
	}
	if _, err := appearance.ReadAppearance(); err == nil {
		t.Fatal("standalone read suite preferences")
	}
	if _, err := appearance.Theme(); err == nil {
		t.Fatal("standalone read legacy suite theme")
	}
	if _, err := os.Stat(filepath.Join(root, "modbus.slave")); !os.IsNotExist(err) {
		t.Fatalf("standalone created a suite role directory: %v", err)
	}
}
