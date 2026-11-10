package suite

import (
	"crypto/sha256"
	"encoding/hex"
	"encoding/json"
	"fmt"
	"io"
	"os"
	"path/filepath"
	"runtime"
	"strings"
)

type Component struct {
	ID           string `json:"id"`
	Version      string `json:"version"`
	SourceCommit string `json:"sourceCommit"`
	Executable   string `json:"executable"`
	SHA256       string `json:"sha256"`
	SuiteBuild   bool   `json:"suiteBuild"`
}

type Manifest struct {
	SchemaVersion int         `json:"schemaVersion"`
	SuiteVersion  string      `json:"suiteVersion"`
	Platform      string      `json:"platform"`
	Architecture  string      `json:"architecture"`
	Components    []Component `json:"components"`
}

func (m Manifest) Validate(platform, architecture string) error {
	if m.SchemaVersion != 1 || m.SuiteVersion == "" {
		return fmt.Errorf("套件清单格式无效，请重新安装完整套件")
	}
	if m.Platform != platform || m.Architecture != architecture {
		return fmt.Errorf("此套件适用于 %s/%s，当前系统为 %s/%s", m.Platform, m.Architecture, platform, architecture)
	}
	seen := make(map[string]bool)
	for _, c := range m.Components {
		if _, ok := FindRole(c.ID); !ok || seen[c.ID] {
			return fmt.Errorf("套件清单包含未知或重复角色 %q", c.ID)
		}
		seen[c.ID] = true
		digest, err := hex.DecodeString(c.SHA256)
		if !c.SuiteBuild || c.Version != m.SuiteVersion || len(c.SourceCommit) != 40 || err != nil || len(digest) != sha256.Size {
			return fmt.Errorf("%s 的套件版本或校验信息不完整", c.ID)
		}
		if err := validateRelative(c.Executable); err != nil {
			return fmt.Errorf("%s 的启动路径无效: %w", c.ID, err)
		}
	}
	if len(seen) != len(Roles) {
		return fmt.Errorf("完整套件需要八个角色，当前清单包含 %d 个", len(seen))
	}
	return nil
}

func validateRelative(path string) error {
	clean := filepath.Clean(filepath.FromSlash(path))
	if path == "" || filepath.IsAbs(clean) || clean == "." || clean == ".." || strings.HasPrefix(clean, ".."+string(filepath.Separator)) || strings.Contains(path, "\\") || strings.Contains(path, ":") {
		return fmt.Errorf("路径必须位于套件内部")
	}
	return nil
}

// ResolveExecutable rejects both lexical escapes and symlinks outside the suite.
func ResolveExecutable(root, relative string) (string, error) {
	if err := validateRelative(relative); err != nil {
		return "", err
	}
	base, err := filepath.EvalSymlinks(root)
	if err != nil {
		return "", err
	}
	path, err := filepath.EvalSymlinks(filepath.Join(base, filepath.FromSlash(relative)))
	if err != nil {
		return "", fmt.Errorf("内置程序缺失，请修复完整套件: %w", err)
	}
	rel, err := filepath.Rel(base, path)
	if err != nil || rel == ".." || strings.HasPrefix(rel, ".."+string(filepath.Separator)) {
		return "", fmt.Errorf("内置程序指向了套件以外的位置")
	}
	info, err := os.Stat(path)
	if err != nil {
		return "", err
	}
	if !info.Mode().IsRegular() {
		return "", fmt.Errorf("内置程序不是有效文件")
	}
	if runtime.GOOS != "windows" && info.Mode().Perm()&0111 == 0 {
		return "", fmt.Errorf("内置程序没有执行权限，请修复完整套件")
	}
	return path, nil
}

func VerifyExecutable(path, expected string) error {
	f, err := os.Open(path)
	if err != nil {
		return err
	}
	defer f.Close()
	h := sha256.New()
	if _, err := io.Copy(h, f); err != nil {
		return err
	}
	if !strings.EqualFold(hex.EncodeToString(h.Sum(nil)), expected) {
		return fmt.Errorf("内置程序校验不匹配，请修复完整套件")
	}
	return nil
}

func LoadManifest(root string) (Manifest, error) {
	for _, relative := range []string{"suite-manifest.json", "Resources/suite-manifest.json"} {
		data, err := os.ReadFile(filepath.Join(root, relative))
		if os.IsNotExist(err) {
			continue
		}
		if err != nil {
			return Manifest{}, err
		}
		var m Manifest
		if err := json.Unmarshal(data, &m); err != nil {
			return m, fmt.Errorf("套件清单损坏: %w", err)
		}
		return m, m.Validate(runtime.GOOS, runtime.GOARCH)
	}
	return Manifest{}, fmt.Errorf("未找到完整套件清单，请从完整安装包启动 SimLab")
}

func LocateRoot(executable string) string {
	if real, err := filepath.EvalSymlinks(executable); err == nil {
		executable = real
	}
	dir := filepath.Dir(executable)
	for i := 0; i < 4; i++ {
		for _, name := range []string{"suite-manifest.json", "Resources/suite-manifest.json"} {
			if _, err := os.Stat(filepath.Join(dir, name)); err == nil {
				return dir
			}
		}
		dir = filepath.Dir(dir)
	}
	return filepath.Dir(executable)
}
