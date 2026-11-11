package suite

import (
	"encoding/json"
	"fmt"
	configstore "github.com/Karl-Dai/Simulator-All-in-One/storage/go"
	"os"
	"path/filepath"
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

func openPreferences(root string) (Preferences, *configstore.Store, error) {
	p := defaultPreferences()
	store, err := configstore.Open(root, "launcher")
	if err != nil {
		return p, nil, err
	}
	fail := func(err error) (Preferences, *configstore.Store, error) {
		store.Close()
		return defaultPreferences(), nil, err
	}
	var restored *Preferences
	present, err := store.Load("preferences", &restored)
	if err != nil {
		return fail(err)
	}
	if !present {
		data, readErr := os.ReadFile(filepath.Join(root, "launcher.json"))
		if readErr != nil && !os.IsNotExist(readErr) {
			return fail(readErr)
		}
		if data != nil {
			var legacy Preferences
			if err = json.Unmarshal(data, &legacy); err != nil || legacy.SchemaVersion != 1 {
				if err == nil {
					err = fmt.Errorf("unsupported launcher preferences version")
				}
				return fail(err)
			}
		}
		if _, err = store.MigrateJSON("preferences", data, "launcher.json"); err != nil {
			return fail(err)
		}
		if present, err = store.Load("preferences", &restored); err != nil {
			return fail(err)
		}
	}
	if present {
		if restored == nil || restored.SchemaVersion != 1 {
			return fail(fmt.Errorf("invalid launcher preferences or unsupported version"))
		}
		p = *restored
	}
	if p.SchemaVersion != 1 {
		return fail(fmt.Errorf("unsupported launcher preferences version"))
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
	return p, store, nil
}
func loadPreferences(root string) (Preferences, string) {
	p, store, err := openPreferences(root)
	if store != nil {
		store.Close()
	}
	if err != nil {
		return p, "无法恢复入口偏好，自动保存已停止: " + err.Error()
	}
	return p, ""
}
func (s *Service) savePreferences(p Preferences) error {
	if s.prefsErr != nil {
		return fmt.Errorf("无法恢复入口偏好，自动保存已停止: %w", s.prefsErr)
	}
	if s.prefsStore == nil {
		return fmt.Errorf("configuration store is unavailable")
	}
	return s.prefsStore.Save("preferences", p)
}
