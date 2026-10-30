package suitehost

import (
	"os"
	"path/filepath"
	"testing"
)

func TestAppearanceReadsSuitePreferencesWithoutExposingOtherSettings(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	appearance := &Appearance{}
	for _, theme := range []string{"system", "light", "dark", "unknown"} {
		if err := os.WriteFile(filepath.Join(root, "launcher.json"), []byte(`{"theme":"`+theme+`","favorites":["pmu.sub"]}`), 0600); err != nil {
			t.Fatal(err)
		}
		want := theme
		if theme == "unknown" {
			want = "system"
		}
		got, err := appearance.Theme()
		if err != nil || got != want {
			t.Fatalf("theme %s: got %q, %v", theme, got, err)
		}
	}
	if err := os.Remove(filepath.Join(root, "launcher.json")); err != nil {
		t.Fatal(err)
	}
	if got, err := appearance.Theme(); err != nil || got != "system" {
		t.Fatalf("missing settings: %q, %v", got, err)
	}
	if err := os.WriteFile(filepath.Join(root, "launcher.json"), []byte(`{invalid`), 0600); err != nil {
		t.Fatal(err)
	}
	if _, err := appearance.Theme(); err == nil {
		t.Fatal("corrupt preferences must report an error")
	}
	t.Setenv("SIMLAB_DATA_DIR", "relative")
	if _, err := appearance.Theme(); err == nil {
		t.Fatal("relative data directory accepted")
	}
}
