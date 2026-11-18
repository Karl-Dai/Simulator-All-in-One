//go:build simlab_suite

package main

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/Karl-Dai/ModbusSim/go/internal/suitehost"
	"github.com/wailsapp/wails/v2/pkg/options"
)

func TestSuiteAppearanceBindingReadsBothInterfaces(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	if err := os.WriteFile(filepath.Join(root, "launcher.json"), []byte(`{"schemaVersion":1,"theme":"dark","language":"zh-CN"}`), 0600); err != nil {
		t.Fatal(err)
	}
	original := new(int)
	app := &options.App{Bind: []interface{}{original}}
	if err := configureSuite(app); err != nil {
		t.Fatal(err)
	}
	if len(app.Bind) != 2 || app.Bind[0] != original {
		t.Fatal("existing application binding was replaced")
	}
	appearance, ok := app.Bind[1].(*suitehost.Appearance)
	if !ok {
		t.Fatalf("missing stable appearance binding: %T", app.Bind[1])
	}
	value, err := appearance.ReadAppearance()
	if err != nil || value.Theme != "dark" || value.Language != "zh-CN" {
		t.Fatalf("suite preferences unavailable: %#v, %v", value, err)
	}
	theme, err := appearance.Theme()
	if err != nil || theme != "dark" {
		t.Fatalf("legacy theme unavailable: %q, %v", theme, err)
	}
}
