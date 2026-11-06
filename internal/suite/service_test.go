package suite

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"os"
	"path/filepath"
	"runtime"
	"strings"
	"sync"
	"testing"
	"time"
)

func fixture(t *testing.T, body string) (string, string, Manifest) {
	t.Helper()
	root := filepath.Join(t.TempDir(), "SimLab 中文 suite")
	if err := os.MkdirAll(filepath.Join(root, "apps"), 0755); err != nil {
		t.Fatal(err)
	}
	if runtime.GOOS == "linux" {
		if err := os.MkdirAll(filepath.Join(root, "runtime", "usr"), 0755); err != nil {
			t.Fatal(err)
		}
	}
	m := Manifest{SchemaVersion: 1, SuiteVersion: "0.1.0", Platform: runtime.GOOS, Architecture: runtime.GOARCH}
	for _, role := range Roles {
		path := filepath.Join("apps", role.ID)
		data := []byte(body)
		if err := os.WriteFile(filepath.Join(root, path), data, 0755); err != nil {
			t.Fatal(err)
		}
		digest := sha256.Sum256(data)
		m.Components = append(m.Components, Component{role.ID, m.SuiteVersion, strings.Repeat("a", 40), filepath.ToSlash(path), hex.EncodeToString(digest[:]), true})
	}
	writeManifest(t, root, m)
	return root, t.TempDir(), m
}

func writeManifest(t *testing.T, root string, m Manifest) {
	t.Helper()
	data, err := json.Marshal(m)
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "suite-manifest.json"), data, 0644); err != nil {
		t.Fatal(err)
	}
}

func TestCompleteManifestAndIntegrity(t *testing.T) {
	root, data, _ := fixture(t, "program")
	s := New(root, data)
	if got := s.Snapshot(); len(got.Roles) != 8 {
		t.Fatalf("roles: %d", len(got.Roles))
	} else {
		for _, role := range got.Roles {
			if !role.Ready {
				t.Errorf("%s: %s", role.ID, role.Error)
			}
		}
	}
	if err := os.WriteFile(filepath.Join(root, "apps", "pmu.sub"), []byte("modified"), 0755); err != nil {
		t.Fatal(err)
	}
	got := s.Snapshot()
	for _, role := range got.Roles {
		if role.ID == "pmu.sub" && (role.Ready || !strings.Contains(role.Error, "校验")) {
			t.Fatalf("tampered role: %+v", role)
		}
		if role.ID != "pmu.sub" && !role.Ready {
			t.Errorf("unaffected role %s broken", role.ID)
		}
	}
}

func TestManifestRejectsIncompleteDuplicateWrongTarget(t *testing.T) {
	_, _, m := fixture(t, "program")
	cases := []struct {
		name   string
		mutate func(*Manifest)
	}{
		{"incomplete", func(m *Manifest) { m.Components = m.Components[:7] }},
		{"duplicate", func(m *Manifest) { m.Components[1].ID = m.Components[0].ID }},
		{"component-version", func(m *Manifest) { m.Components[1].Version = "99.0.0" }},
		{"standalone", func(m *Manifest) { m.Components[1].SuiteBuild = false }},
		{"escape", func(m *Manifest) { m.Components[0].Executable = "../outside" }},
		{"windows-path", func(m *Manifest) { m.Components[0].Executable = "C:\\outside.exe" }},
		{"invalid-digest", func(m *Manifest) { m.Components[0].SHA256 = "abc" }},
		{"wrong-target", func(m *Manifest) { m.Platform = "other" }},
	}
	for _, tc := range cases {
		t.Run(tc.name, func(t *testing.T) {
			next := m
			next.Components = append([]Component{}, m.Components...)
			tc.mutate(&next)
			if err := next.Validate(runtime.GOOS, runtime.GOARCH); err == nil {
				t.Fatal("invalid manifest accepted")
			}
		})
	}
}

func TestRejectsSymlinkOutsideSuite(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("symlink permissions vary on Windows")
	}
	root, _, _ := fixture(t, "program")
	outside := filepath.Join(t.TempDir(), "external")
	if err := os.WriteFile(outside, []byte("outside"), 0755); err != nil {
		t.Fatal(err)
	}
	link := filepath.Join(root, "apps", "outside")
	if err := os.Symlink(outside, link); err != nil {
		t.Fatal(err)
	}
	if _, err := ResolveExecutable(root, "apps/outside"); err == nil {
		t.Fatal("external symlink accepted")
	}
}

func TestPreferencesConcurrencyAndReload(t *testing.T) {
	root, data, _ := fixture(t, "program")
	s := New(root, data)
	var wg sync.WaitGroup
	for _, role := range Roles {
		wg.Add(1)
		go func(id string) {
			defer wg.Done()
			if _, err := s.SetFavorite(id, true); err != nil {
				t.Error(err)
			}
			_ = s.Snapshot()
		}(role.ID)
	}
	wg.Wait()
	if _, err := s.SetTheme("dark"); err != nil {
		t.Fatal(err)
	}
	got := New(root, data).Snapshot().Preferences
	if len(got.Favorites) != 8 || got.Theme != "dark" {
		t.Fatalf("preferences: %+v", got)
	}
	got.Favorites[0] = "mutated"
	if s.Snapshot().Preferences.Favorites[0] == "mutated" {
		t.Fatal("shared preference slice")
	}
}

func TestSetLanguagePersistedAndNormalized(t *testing.T) {
	root, data, _ := fixture(t, "program")
	s := New(root, data)
	if got := s.Snapshot().Preferences.Language; got != "en" {
		t.Fatalf("new installation must default to English: %q", got)
	}
	for _, language := range []string{"zh-CN", "auto", "en"} {
		if _, err := s.SetLanguage(language); err != nil {
			t.Fatal(err)
		}
		if got := New(root, data).Snapshot().Preferences.Language; got != language {
			t.Fatalf("language preference changed on reload: want %q, got %q", language, got)
		}
	}
	if _, err := s.SetLanguage("fr"); err == nil {
		t.Fatal("expected invalid language error")
	}
	legacy := `{"schemaVersion":1,"theme":"light","favorites":[],"recent":{}}`
	if err := os.WriteFile(filepath.Join(data, "launcher.json"), []byte(legacy), 0600); err != nil {
		t.Fatal(err)
	}
	if got := New(root, data).Snapshot().Preferences.Language; got != "en" {
		t.Fatalf("legacy language normalized: %q", got)
	}
}

func TestCorruptPreferencesPreserved(t *testing.T) {
	root, data, _ := fixture(t, "program")
	if err := os.WriteFile(filepath.Join(data, "launcher.json"), []byte("broken"), 0600); err != nil {
		t.Fatal(err)
	}
	got := New(root, data).Snapshot()
	if got.Warning == "" || got.Preferences.Theme != "system" {
		t.Fatalf("recovery: %+v", got)
	}
	files, _ := filepath.Glob(filepath.Join(data, "launcher.json.corrupt-*"))
	if len(files) != 1 {
		t.Fatalf("backups: %v", files)
	}
}

func TestLaunchUsesBundleAndDataEnvironment(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("POSIX helper; Windows process code is cross compiled separately")
	}
	root, data, _ := fixture(t, "#!/bin/sh\nprintf '%s\\n%s' \"$SIMLAB_DATA_DIR\" \"$PWD\" > \"$SIMLAB_TEST_CAPTURE\"\nsleep 1\n")
	capture := filepath.Join(t.TempDir(), "capture")
	t.Setenv("SIMLAB_TEST_CAPTURE", capture)
	s := New(root, data)
	result, err := s.Launch("modbus.master")
	if err != nil {
		t.Fatal(err)
	}
	if result.PID <= 0 {
		t.Fatal("missing pid")
	}
	var content []byte
	for i := 0; i < 20; i++ {
		content, err = os.ReadFile(capture)
		if err == nil {
			break
		}
		time.Sleep(10 * time.Millisecond)
	}
	work := filepath.Join(data, "modbus-master")
	if runtime.GOOS == "linux" {
		work = filepath.Join(root, "runtime", "usr")
	}
	workingDir, err := filepath.EvalSymlinks(work)
	if err != nil {
		t.Fatal(err)
	}
	expected := data + "\n" + workingDir
	if string(content) != expected {
		t.Fatalf("wrong launch environment: %q; want %q", content, expected)
	}
	if s.Snapshot().Preferences.Recent["modbus.master"] == "" {
		t.Fatal("recent launch missing")
	}
	if _, err := s.Launch("../external"); err == nil {
		t.Fatal("unknown role accepted")
	}
}

func TestEarlyFailureIsNotRecordedAsSuccess(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("POSIX helper")
	}
	root, data, _ := fixture(t, "#!/bin/sh\nexit 7\n")
	s := New(root, data)
	if _, err := s.Launch("modbus.slave"); err == nil {
		t.Fatal("early failure accepted")
	}
	if len(s.Snapshot().Preferences.Recent) != 0 {
		t.Fatal("failed launch recorded as successful")
	}
}

func TestEarlyNormalExitIsNotTreatedAsOpenWindow(t *testing.T) {
	if runtime.GOOS == "windows" {
		t.Skip("POSIX helper")
	}
	root, data, _ := fixture(t, "#!/bin/sh\nexit 0\n")
	s := New(root, data)
	if _, err := s.Launch("pmu.sub"); err == nil {
		t.Fatal("early normal exit accepted as open window")
	}
	if len(s.Snapshot().Preferences.Recent) != 0 {
		t.Fatal("exited launch recorded")
	}
}

func TestLinuxMissingRuntimeCannotLaunch(t *testing.T) {
	if runtime.GOOS != "linux" {
		t.Skip("Linux runtime contract")
	}
	root, data, _ := fixture(t, "#!/bin/sh\nsleep 1\n")
	if err := os.RemoveAll(filepath.Join(root, "runtime")); err != nil {
		t.Fatal(err)
	}
	if _, err := New(root, data).Launch("modbus.master"); err == nil {
		t.Fatal("missing Linux runtime accepted")
	}
}

func TestDataRootOverrideMustBeAbsolute(t *testing.T) {
	root := t.TempDir()
	t.Setenv("SIMLAB_DATA_DIR", root)
	if got, err := DefaultDataDir(); err != nil || got != root {
		t.Fatalf("absolute data root: %q %v", got, err)
	}
	t.Setenv("SIMLAB_DATA_DIR", "relative-path")
	if _, err := DefaultDataDir(); err == nil {
		t.Fatal("relative data root accepted")
	}
}
