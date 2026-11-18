// Package suitehost isolates the embedded SimLab build from standalone apps.
package suitehost

import (
	"embed"
	"fmt"
	"os"
	"path/filepath"
	"runtime"

	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/linux"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

//go:embed icons/*.png
var roleIcons embed.FS

// DataDir never resolves against the launcher's working directory. A relative
// override is rejected instead of silently creating data beside a payload.
func DataDir(role string) (string, error) {
	if role != "master" && role != "slave" {
		return "", fmt.Errorf("unsupported Modbus suite role %q", role)
	}
	root := os.Getenv("SIMLAB_DATA_DIR")
	if root == "" {
		config, err := os.UserConfigDir()
		if err != nil {
			return "", fmt.Errorf("resolve SimLab data directory: %w", err)
		}
		root = filepath.Join(config, "SimLab")
	}
	if !filepath.IsAbs(root) {
		return "", fmt.Errorf("SIMLAB_DATA_DIR must be an absolute path")
	}
	return filepath.Join(root, "modbus."+role), nil
}

// Configure is called only by the simlab_suite build-tagged entry point.
func Configure(app *options.App, role string) error {
	dir, err := DataDir(role)
	if err != nil {
		return err
	}
	if err := os.MkdirAll(dir, 0700); err != nil {
		return fmt.Errorf("create Modbus suite data directory: %w", err)
	}
	identity := "cloud.carldai.simlab.modbus." + role
	if role == "master" {
		app.Title = "Modbus Master — SAIO"
	} else {
		app.Title = "Modbus Slave — SAIO"
	}
	switch runtime.GOOS {
	case "windows":
		if app.Windows == nil {
			app.Windows = &windows.Options{}
		}
		app.Windows.WebviewUserDataPath = filepath.Join(dir, "webview")
	case "linux":
		// WebKitGTK's default context follows GLib's XDG locations. Set these
		// before Wails initializes GTK/WebKit and keep the two roles separate.
		for name, leaf := range map[string]string{
			"XDG_DATA_HOME": "data", "XDG_CONFIG_HOME": "config", "XDG_CACHE_HOME": "cache",
		} {
			path := filepath.Join(dir, leaf)
			if err := os.MkdirAll(path, 0700); err != nil {
				return fmt.Errorf("create %s: %w", name, err)
			}
			if err := os.Setenv(name, path); err != nil {
				return fmt.Errorf("set %s: %w", name, err)
			}
		}
		if app.Linux == nil {
			// Preserve Wails' default GPU policy when setting ProgramName.
			app.Linux = &linux.Options{WebviewGpuPolicy: linux.WebviewGpuPolicyNever}
		}
		app.Linux.ProgramName = identity
		app.Linux.Icon, err = roleIcons.ReadFile("icons/modbus." + role + ".png")
		if err != nil {
			return fmt.Errorf("read suite window icon: %w", err)
		}
	}
	// WKWebView on macOS uses the suite-specific CFBundleIdentifier written
	// by suite.sh, rather than the standalone app's website-data namespace.
	return nil
}
