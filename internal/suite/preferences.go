package suite

import (
	"encoding/json"
	"fmt"
	"os"
	"path/filepath"
	"time"
)

type Preferences struct {
	SchemaVersion int               `json:"schemaVersion"`
	Theme         string            `json:"theme"`
	Language      string            `json:"language"`
	Favorites     []string          `json:"favorites"`
	Recent        map[string]string `json:"recent"`
}

func defaultPreferences() Preferences {
	return Preferences{1, "system", "en", []string{}, map[string]string{}}
}

func clonePreferences(p Preferences) Preferences {
	p.Favorites = append([]string{}, p.Favorites...)
	recent := make(map[string]string, len(p.Recent))
	for id, used := range p.Recent {
		recent[id] = used
	}
	p.Recent = recent
	return p
}

func loadPreferences(root string) (Preferences, string) {
	p := defaultPreferences()
	path := filepath.Join(root, "launcher.json")
	data, err := os.ReadFile(path)
	if os.IsNotExist(err) {
		return p, ""
	}
	if err != nil {
		return p, "无法读取入口偏好: " + err.Error()
	}
	if err = json.Unmarshal(data, &p); err != nil || p.SchemaVersion != 1 {
		backup := path + ".corrupt-" + time.Now().Format("20060102-150405.000000000")
		if err := os.Rename(path, backup); err != nil {
			return defaultPreferences(), "入口偏好损坏，保留原文件: " + err.Error()
		}
		return defaultPreferences(), "入口偏好损坏，已保留备份并恢复默认设置"
	}
	if p.Recent == nil {
		p.Recent = map[string]string{}
	}
	if p.Favorites == nil {
		p.Favorites = []string{}
	}
	if p.Theme != "system" && p.Theme != "light" && p.Theme != "dark" {
		p.Theme = "system"
	}
	if p.Language != "auto" && p.Language != "zh-CN" && p.Language != "en" {
		p.Language = "en"
	}
	return p, ""
}

func savePreferences(root string, p Preferences) error {
	if err := os.MkdirAll(root, 0700); err != nil {
		return err
	}
	data, err := json.MarshalIndent(p, "", "  ")
	if err != nil {
		return err
	}
	f, err := os.CreateTemp(root, ".launcher-*.json")
	if err != nil {
		return err
	}
	name := f.Name()
	defer os.Remove(name)
	if err = f.Chmod(0600); err == nil {
		_, err = f.Write(append(data, '\n'))
	}
	if err == nil {
		err = f.Sync()
	}
	closeErr := f.Close()
	if err != nil {
		return err
	}
	if closeErr != nil {
		return closeErr
	}
	if err = os.Rename(name, filepath.Join(root, "launcher.json")); err != nil {
		return fmt.Errorf("保存入口偏好失败: %w", err)
	}
	return nil
}
