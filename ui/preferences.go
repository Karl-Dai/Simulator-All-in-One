package suitehost

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// Appearance exposes read-only display preferences to the suite webview.
type Appearance struct {
	suiteMode bool
	reader    func(string, string, string) ([]byte, bool, error)
}

// Both build modes expose stable bindings, but only a suite build may read its settings.
func NewAppearance(suiteMode bool, readers ...func(string, string, string) ([]byte, bool, error)) *Appearance {
	a := &Appearance{suiteMode: suiteMode}
	if len(readers) > 0 {
		a.reader = readers[0]
	}
	return a
}
func (a *Appearance) read(root string) ([]byte, error) {
	if a.reader != nil {
		data, present, err := a.reader(root, "launcher", "preferences")
		if err != nil {
			return nil, err
		}
		if present {
			return data, nil
		}
	}
	data, err := os.ReadFile(filepath.Join(root, "launcher.json"))
	if os.IsNotExist(err) {
		return nil, nil
	}
	return data, err
}

// AppearanceV1 deliberately excludes private launcher and protocol state.
type AppearanceV1 struct {
	SchemaVersion int    `json:"schemaVersion"`
	Theme         string `json:"theme"`
	Language      string `json:"language"`
}

func (a *Appearance) ReadAppearance() (AppearanceV1, error) {
	result := AppearanceV1{SchemaVersion: 1, Theme: "system", Language: "en"}
	if !a.suiteMode {
		return result, fmt.Errorf("appearance is managed by the standalone application")
	}
	root := os.Getenv("SIMLAB_DATA_DIR")
	if !filepath.IsAbs(root) {
		return result, fmt.Errorf("SIMLAB_DATA_DIR must be absolute")
	}
	data, err := a.read(root)
	if err == nil && data == nil {
		return result, nil
	}
	if err != nil {
		return result, err
	}
	var fields map[string]json.RawMessage
	if err := json.Unmarshal(data, &fields); err != nil {
		return result, err
	}
	if fields == nil {
		return result, fmt.Errorf("launcher preferences must be an object")
	}
	if raw, exists := fields["schemaVersion"]; exists {
		var version int
		if err := json.Unmarshal(raw, &version); err != nil || version != 1 {
			return result, fmt.Errorf("unsupported launcher preferences version")
		}
	}
	var theme, language string
	_ = json.Unmarshal(fields["theme"], &theme)
	_ = json.Unmarshal(fields["language"], &language)
	if theme == "light" || theme == "dark" {
		result.Theme = theme
	}
	if language == "auto" || language == "zh-CN" {
		result.Language = language
	}
	return result, nil
}

func (a *Appearance) Theme() (string, error) {
	if !a.suiteMode {
		return "", fmt.Errorf("appearance is managed by the standalone application")
	}
	root := os.Getenv("SIMLAB_DATA_DIR")
	if !filepath.IsAbs(root) {
		return "", fmt.Errorf("SIMLAB_DATA_DIR must be absolute")
	}
	data, err := a.read(root)
	if err == nil && data == nil {
		return "system", nil
	}
	if err != nil {
		return "", err
	}
	var value struct {
		Theme string `json:"theme"`
	}
	if err := json.Unmarshal(data, &value); err != nil {
		return "", err
	}
	if value.Theme == "light" || value.Theme == "dark" {
		return value.Theme, nil
	}
	return "system", nil
}
