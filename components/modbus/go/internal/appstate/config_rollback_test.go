package appstate

import (
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"testing"
)

func TestRestoreConnectionsKeepsUnrelatedObjects(t *testing.T) {
	m := NewManager(Events{})
	defer m.CloseAll()
	req := CreateConnectionRequest{Transport: Transport{Type: project.TransportTCP, Host: "127.0.0.1", Port: 15020}, SlaveID: 1, RegisterRanges: []RegisterRange{}}
	healthy, err := m.CreateConnection(req)
	if err != nil {
		t.Fatal(err)
	}
	req.Transport.Port++
	target, err := m.CreateConnection(req)
	if err != nil {
		t.Fatal(err)
	}
	before := m.connections[healthy.ID]
	old := m.Project()
	if err = m.DeleteConnection(target.ID); err != nil {
		t.Fatal(err)
	}
	if err = m.RestoreConnections(old, []string{target.ID}); err != nil {
		t.Fatal(err)
	}
	if m.connections[healthy.ID] != before {
		t.Fatal("rollback replaced unrelated listener/simulator")
	}
	if m.connections[target.ID] == nil || m.connections[target.ID].state != Stopped {
		t.Fatal("target was not restored stopped")
	}
}
