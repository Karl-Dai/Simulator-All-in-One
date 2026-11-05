package suitehost

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"
)

func TestReadAppearanceContract(t *testing.T) {
	data, err := os.ReadFile("contracts/appearance-cases.json")
	if err != nil {
		t.Fatal(err)
	}
	var cases struct {
		FileCases []struct {
			Name       string       `json:"name"`
			Contents   *string      `json:"contents"`
			Appearance AppearanceV1 `json:"appearance"`
			Error      string       `json:"error"`
		} `json:"fileCases"`
	}
	if err := json.Unmarshal(data, &cases); err != nil {
		t.Fatal(err)
	}
	for _, c := range cases.FileCases {
		t.Run(c.Name, func(t *testing.T) {
			root := t.TempDir()
			t.Setenv("SIMLAB_DATA_DIR", root)
			path := filepath.Join(root, "launcher.json")
			if c.Contents != nil {
				if err := os.WriteFile(path, []byte(*c.Contents), 0600); err != nil {
					t.Fatal(err)
				}
			}
			got, err := NewAppearance(true).ReadAppearance()
			if c.Error != "" {
				if err == nil {
					t.Fatalf("expected %s", c.Error)
				}
			} else if err != nil || got != c.Appearance {
				t.Fatalf("got %+v, %v; want %+v", got, err, c.Appearance)
			}
			if c.Contents != nil {
				after, err := os.ReadFile(path)
				if err != nil || string(after) != *c.Contents {
					t.Fatal("reader modified preferences")
				}
			}
		})
	}
	t.Setenv("SIMLAB_DATA_DIR", "relative")
	if _, err := NewAppearance(true).ReadAppearance(); err == nil {
		t.Fatal("relative root accepted")
	}
}

func TestStandaloneAppearanceRejectsSuitePreferences(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	path := filepath.Join(root, "launcher.json")
	contents := []byte(`{"schemaVersion":1,"theme":"dark","language":"zh-CN"}`)
	if err := os.WriteFile(path, contents, 0600); err != nil {
		t.Fatal(err)
	}
	standalone := NewAppearance(false)
	if _, err := standalone.ReadAppearance(); err == nil {
		t.Fatal("standalone binding exposed suite appearance")
	}
	if _, err := standalone.Theme(); err == nil {
		t.Fatal("standalone binding exposed legacy suite theme")
	}
	if after, err := os.ReadFile(path); err != nil || string(after) != string(contents) {
		t.Fatal("standalone binding modified suite preferences")
	}
}

func TestAppearanceReadsSuitePreferencesWithoutExposingOtherSettings(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	appearance := NewAppearance(true)
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
