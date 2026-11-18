package masterstate

import (
	"path/filepath"
	"testing"
)

func TestMultipleSlavePollingAndProjectTargets(t *testing.T) {
	h := serve(t, 0)
	m, id := newTestManager(t, h, Events{})
	seven := uint8(7)
	for _, v := range []struct {
		sid   *uint8
		value uint16
	}{{nil, 101}, {&seven, 707}} {
		if err := m.WriteSingleRegisterSlave(id, v.sid, 0, v.value); err != nil {
			t.Fatal(err)
		}
	}
	g1, err := m.AddScanGroup(id, ScanGroup{Name: "default", Function: "read_holding_registers", Quantity: 1, IntervalMS: 20, Enabled: true})
	if err != nil {
		t.Fatal(err)
	}
	g7, err := m.AddScanGroup(id, ScanGroup{Name: "seven", Function: "read_holding_registers", Quantity: 1, IntervalMS: 20, Enabled: true, SlaveID: &seven})
	if err != nil {
		t.Fatal(err)
	}
	for _, g := range []ScanGroup{g1, g7} {
		if err = m.StartPolling(id, g.ID); err != nil {
			t.Fatal(err)
		}
	}
	eventually(t, func() bool {
		a, _ := m.GetPollData(id, g1.ID)
		b, _ := m.GetPollData(id, g7.ID)
		return a != nil && b != nil && len(a.Values) == 1 && len(b.Values) == 1 && a.Values[0].RawValue == 101 && b.Values[0].RawValue == 707
	})
	path := filepath.Join(t.TempDir(), "multi.modbusproj")
	if err = m.SaveProject(path); err != nil {
		t.Fatal(err)
	}
	if _, err = m.LoadProject(path); err != nil {
		t.Fatal(err)
	}
	gs, err := m.ListScanGroups(id)
	if err != nil {
		t.Fatal(err)
	}
	if len(gs) != 2 || gs[0].SlaveID != nil || gs[1].SlaveID == nil || *gs[1].SlaveID != 7 {
		t.Fatalf("targets changed after reload: %+v", gs)
	}
	settings, err := m.GetSettings(id)
	if err != nil {
		t.Fatal(err)
	}
	settings.SlaveID = 7
	if err = m.UpdateConnection(id, settings); err != nil {
		t.Fatal(err)
	}
	if err = m.Connect(id); err != nil {
		t.Fatal(err)
	}
	result, err := m.RefreshScanGroup(id, g1.ID)
	if err != nil || len(result.Values) != 1 || result.Values[0].RawValue != 707 {
		t.Fatalf("group no longer follows connection default: %+v %v", result, err)
	}
}
