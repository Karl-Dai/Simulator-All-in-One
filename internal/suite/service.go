package suite

import (
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"runtime"
	"slices"
	"strings"
	"sync"
	"time"
)

type RoleState struct {
	Role
	Version string `json:"version"`
	Ready   bool   `json:"ready"`
	Error   string `json:"error"`
}

type Snapshot struct {
	Version      string      `json:"version"`
	Platform     string      `json:"platform"`
	Architecture string      `json:"architecture"`
	Roles        []RoleState `json:"roles"`
	Preferences  Preferences `json:"preferences"`
	Warning      string      `json:"warning"`
}

type LaunchResult struct {
	ID      string `json:"id"`
	PID     int    `json:"pid"`
	Warning string `json:"warning"`
}

type Service struct {
	root        string
	dataDir     string
	manifest    Manifest
	manifestErr error
	prefs       Preferences
	warning     string
	mu          sync.Mutex
	starting    map[string]bool
}

func New(root, dataDir string) *Service {
	m, err := LoadManifest(root)
	prefs, warning := loadPreferences(dataDir)
	return &Service{root: root, dataDir: dataDir, manifest: m, manifestErr: err, prefs: prefs, warning: warning, starting: map[string]bool{}}
}

func DefaultDataDir() (string, error) {
	if root, configured := os.LookupEnv("SIMLAB_DATA_DIR"); configured {
		if !filepath.IsAbs(root) {
			return "", fmt.Errorf("SIMLAB_DATA_DIR 必须是绝对路径")
		}
		return filepath.Clean(root), nil
	}
	root, err := os.UserConfigDir()
	if err != nil {
		return "", err
	}
	return filepath.Join(root, "SimLab"), nil
}

func (s *Service) component(id string) (Component, error) {
	if s.manifestErr != nil {
		return Component{}, s.manifestErr
	}
	for _, c := range s.manifest.Components {
		if c.ID == id {
			return c, nil
		}
	}
	return Component{}, fmt.Errorf("未找到角色 %q", id)
}

func (s *Service) Snapshot() Snapshot {
	s.mu.Lock()
	defer s.mu.Unlock()
	result := Snapshot{Version: s.manifest.SuiteVersion, Platform: runtime.GOOS, Architecture: runtime.GOARCH, Roles: []RoleState{}, Preferences: s.prefs, Warning: s.warning}
	if result.Version == "" {
		result.Version = "0.1.2"
	}
	// Copy preferences before leaving the lock; UI reads may overlap writes.
	result.Preferences.Favorites = slices.Clone(s.prefs.Favorites)
	result.Preferences.Recent = make(map[string]string, len(s.prefs.Recent))
	for k, v := range s.prefs.Recent {
		result.Preferences.Recent[k] = v
	}
	for _, role := range Roles {
		state := RoleState{Role: role}
		c, err := s.component(role.ID)
		if err == nil {
			state.Version = c.Version
			var path string
			path, err = ResolveExecutable(s.root, c.Executable)
			if err == nil {
				err = VerifyExecutable(path, c.SHA256)
			}
		}
		state.Ready = err == nil
		if err != nil {
			state.Error = err.Error()
		}
		result.Roles = append(result.Roles, state)
	}
	return result
}

func (s *Service) SetFavorite(id string, favorite bool) (Preferences, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if _, ok := FindRole(id); !ok {
		return clonePreferences(s.prefs), fmt.Errorf("未知模拟器角色")
	}
	next := s.prefs
	next.Favorites = slices.Clone(s.prefs.Favorites)
	i := slices.Index(next.Favorites, id)
	if favorite && i < 0 {
		next.Favorites = append(next.Favorites, id)
	}
	if !favorite && i >= 0 {
		next.Favorites = slices.Delete(next.Favorites, i, i+1)
	}
	if err := savePreferences(s.dataDir, next); err != nil {
		return clonePreferences(s.prefs), err
	}
	s.prefs = next
	return clonePreferences(next), nil
}

func (s *Service) SetTheme(theme string) (Preferences, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if theme != "system" && theme != "light" && theme != "dark" {
		return clonePreferences(s.prefs), fmt.Errorf("未知主题")
	}
	next := s.prefs
	next.Theme = theme
	if err := savePreferences(s.dataDir, next); err != nil {
		return clonePreferences(s.prefs), err
	}
	s.prefs = next
	return clonePreferences(next), nil
}

func (s *Service) SetLanguage(language string) (Preferences, error) {
	s.mu.Lock()
	defer s.mu.Unlock()
	if language != "auto" && language != "zh-CN" && language != "en" {
		return clonePreferences(s.prefs), fmt.Errorf("未知语言")
	}
	next := s.prefs
	next.Language = language
	if err := savePreferences(s.dataDir, next); err != nil {
		return clonePreferences(s.prefs), err
	}
	s.prefs = next
	return clonePreferences(next), nil
}

func (s *Service) Launch(id string) (LaunchResult, error) {
	if _, ok := FindRole(id); !ok {
		return LaunchResult{}, fmt.Errorf("未知模拟器角色")
	}
	s.mu.Lock()
	if s.starting[id] {
		s.mu.Unlock()
		return LaunchResult{}, fmt.Errorf("该模拟器正在启动")
	}
	s.starting[id] = true
	s.mu.Unlock()
	defer func() { s.mu.Lock(); delete(s.starting, id); s.mu.Unlock() }()
	c, err := s.component(id)
	if err != nil {
		return LaunchResult{}, err
	}
	path, err := ResolveExecutable(s.root, c.Executable)
	if err != nil {
		return LaunchResult{}, err
	}
	if err := VerifyExecutable(path, c.SHA256); err != nil {
		return LaunchResult{}, err
	}
	workDir := filepath.Join(s.dataDir, strings.ReplaceAll(id, ".", "-"))
	if err := os.MkdirAll(workDir, 0700); err != nil {
		return LaunchResult{}, fmt.Errorf("无法创建模拟器用户数据目录: %w", err)
	}
	log, err := os.OpenFile(filepath.Join(workDir, "suite-launch.log"), os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0600)
	if err != nil {
		return LaunchResult{}, fmt.Errorf("无法创建启动日志: %w", err)
	}
	defer log.Close()
	cmd := exec.Command(path)
	cmd.Dir = workDir
	cmd.Env = append(os.Environ(), "SIMLAB_DATA_DIR="+s.dataDir)
	if runtime.GOOS == "linux" {
		// Relocated WebKit helpers resolve their patched ././ prefix from here.
		// User data and logs remain in the role directory, never in this runtime.
		runtimeDir := filepath.Join(s.root, "runtime", "usr")
		info, err := os.Stat(runtimeDir)
		if err != nil || !info.IsDir() {
			return LaunchResult{}, fmt.Errorf("套件 Linux 运行资源缺失，请修复完整套件")
		}
		cmd.Dir = runtimeDir
		libraries := filepath.Join(s.root, "lib")
		if previous := os.Getenv("LD_LIBRARY_PATH"); previous != "" {
			libraries += ":" + previous
		}
		cmd.Env = append(cmd.Env, "LD_LIBRARY_PATH="+libraries)
	}
	cmd.Stdout, cmd.Stderr = log, log
	detach(cmd)
	if err := cmd.Start(); err != nil {
		return LaunchResult{}, fmt.Errorf("无法启动 %s: %w", id, err)
	}
	pid := cmd.Process.Pid
	done := make(chan error, 1)
	go func() { done <- cmd.Wait() }()
	select {
	case err := <-done:
		if err != nil {
			return LaunchResult{}, fmt.Errorf("%s 启动后退出，请查看启动日志: %w", id, err)
		}
		return LaunchResult{}, fmt.Errorf("%s 在打开窗口前退出，请查看启动日志", id)
	case <-time.After(350 * time.Millisecond):
	}
	s.mu.Lock()
	defer s.mu.Unlock()
	s.prefs.Recent[id] = time.Now().UTC().Format(time.RFC3339)
	result := LaunchResult{ID: id, PID: pid}
	if err := savePreferences(s.dataDir, s.prefs); err != nil {
		result.Warning = "应用已启动，但最近使用记录保存失败: " + err.Error()
	}
	return result, nil
}
