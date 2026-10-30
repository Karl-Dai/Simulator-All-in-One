package suitehost

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
)

// Appearance exposes only a read-only theme to the suite webview.
type Appearance struct{}

func (*Appearance) Theme() (string, error) {
	root := os.Getenv("SIMLAB_DATA_DIR")
	if !filepath.IsAbs(root) {
		return "", fmt.Errorf("SIMLAB_DATA_DIR must be absolute")
	}
	data, err := os.ReadFile(filepath.Join(root, "launcher.json"))
	if os.IsNotExist(err) {
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
