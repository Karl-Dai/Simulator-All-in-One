package main

import (
	"fmt"
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"sync"
)

type configurationStore interface {
	Load(string, any) (bool, error)
	Save(string, any) error
	MigrateJSON(string, []byte, string) (bool, error)
	Close() error
}
type configurationState struct {
	mu    sync.Mutex
	store configurationStore
	err   error
}

func (a *App) snapshot() (*project.Project, error) { return a.manager.Project(), nil }
func (a *App) change(ids []string, operation func() error) error {
	a.config.mu.Lock()
	defer a.config.mu.Unlock()
	if a.config.err != nil {
		return fmt.Errorf("configuration restore failed; no changes saved: %w", a.config.err)
	}
	if a.config.store == nil {
		return operation()
	}
	before, err := a.snapshot()
	if err != nil {
		return err
	}
	if err = operation(); err != nil {
		return err
	}
	after, err := a.snapshot()
	if err == nil {
		err = a.config.store.Save("workspace", after)
	}
	if err == nil {
		return nil
	}
	if ids != nil && len(ids) == 0 && after != nil {
		oldIDs := map[string]bool{}
		for _, c := range before.Connections {
			oldIDs[c.ID] = true
		}
		for _, c := range after.Connections {
			if !oldIDs[c.ID] {
				ids = append(ids, c.ID)
			}
		}
	}
	if ids == nil {
		seen := map[string]bool{}
		for _, c := range before.Connections {
			seen[c.ID] = true
		}
		if after != nil {
			for _, c := range after.Connections {
				seen[c.ID] = true
			}
		}
		for id := range seen {
			ids = append(ids, id)
		}
	}
	if rollbackErr := a.manager.RestoreConnections(before, ids); rollbackErr != nil {
		a.config.err = fmt.Errorf("save failed: %v; rollback failed: %w", err, rollbackErr)
		return a.config.err
	}
	return fmt.Errorf("save failed; previous configuration restored; affected connections stopped, restart manually: %w", err)
}
func changeValue[T any](a *App, ids []string, operation func() (T, error)) (T, error) {
	var result T
	err := a.change(ids, func() error { var err error; result, err = operation(); return err })
	if err != nil {
		var zero T
		return zero, err
	}
	return result, nil
}
