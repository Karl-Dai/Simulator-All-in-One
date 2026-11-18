package masterstate

import "testing"

func TestRestoreConnectionsKeepsUnrelatedLiveConnection(t *testing.T) {
	h := serve(t, 0)
	m, healthyID := newTestManager(t, h, Events{})
	healthy, _ := m.get(healthyID)
	target, err := m.CreateConnection(DefaultSettings())
	if err != nil {
		t.Fatal(err)
	}
	old, err := m.Project()
	if err != nil {
		t.Fatal(err)
	}
	changed := DefaultSettings()
	changed.SlaveID = 7
	if err = m.UpdateConnection(target.ID, changed); err != nil {
		t.Fatal(err)
	}
	if err = m.RestoreConnections(old, []string{target.ID}); err != nil {
		t.Fatal(err)
	}
	after, _ := m.get(healthyID)
	if after != healthy {
		t.Fatal("rollback replaced unrelated live engine")
	}
	healthy.mu.Lock()
	state := healthy.state
	healthy.mu.Unlock()
	if state != "Connected" {
		t.Fatalf("healthy state = %s", state)
	}
	restored, _ := m.GetSettings(target.ID)
	if restored.SlaveID != 1 {
		t.Fatalf("rollback slave id = %d", restored.SlaveID)
	}
}
