package suitehost

import (
	"os"
	"path/filepath"
	"testing"

	"github.com/wailsapp/wails/v2/pkg/options"
)

func TestDataDirSeparatesRolesAndRejectsRelativeRoot(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	master, err := DataDir("master")
	if err != nil {
		t.Fatal(err)
	}
	slave, err := DataDir("slave")
	if err != nil {
		t.Fatal(err)
	}
	if master != filepath.Join(root, "modbus.master") || slave != filepath.Join(root, "modbus.slave") {
		t.Fatalf("unexpected suite paths: %q, %q", master, slave)
	}
	if _, err := DataDir("../master"); err == nil {
		t.Fatal("accepted a role that escapes its data root")
	}
	t.Setenv("SIMLAB_DATA_DIR", "relative-data")
	if _, err := DataDir("master"); err == nil {
		t.Fatal("accepted a working-directory-dependent override")
	}
}

func TestConfigureFailsForUnwritableDataRoot(t *testing.T) {
	file := filepath.Join(t.TempDir(), "file")
	if err := os.WriteFile(file, []byte("not a directory"), 0600); err != nil {
		t.Fatal(err)
	}
	t.Setenv("SIMLAB_DATA_DIR", file)
	if err := Configure(&options.App{}, "master"); err == nil {
		t.Fatal("accepted a data root that cannot contain a profile")
	}
}
