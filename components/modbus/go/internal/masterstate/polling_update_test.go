package masterstate

import "testing"

func TestUpdateRunningScanGroupPreservesTargetAndAppliesRange(t *testing.T) {
	h := serve(t, 0)
	m, id := newTestManager(t, h, Events{})
	sid := uint8(7)
	g := group()
	g.SlaveID = &sid
	g, err := m.AddScanGroup(id, g)
	if err != nil {
		t.Fatal(err)
	}
	if err = m.StartPolling(id, g.ID); err != nil {
		t.Fatal(err)
	}
	eventually(t, func() bool { r, _ := m.GetPollData(id, g.ID); return r != nil && len(r.Values) == 3 })
	g.Name = "updated"
	g.StartAddress = 10
	g.Quantity = 6
	g.IntervalMS = 100
	if err = m.UpdateScanGroup(id, g); err != nil {
		t.Fatal(err)
	}
	eventually(t, func() bool {
		r, _ := m.GetPollData(id, g.ID)
		return r != nil && r.StartAddress == 10 && len(r.Values) == 6
	})
	groups, err := m.ListScanGroups(id)
	if err != nil {
		t.Fatal(err)
	}
	if len(groups) != 1 || groups[0].ID != g.ID || !groups[0].IsPolling || groups[0].SlaveID == nil || *groups[0].SlaveID != 7 {
		t.Fatalf("group identity or active polling changed: %+v", groups)
	}
	if err = m.StopPolling(id, g.ID); err != nil {
		t.Fatal(err)
	}
	g.Quantity = 2
	if err = m.UpdateScanGroup(id, g); err != nil {
		t.Fatal(err)
	}
	groups, _ = m.ListScanGroups(id)
	if groups[0].IsPolling {
		t.Fatal("editing a stopped group started polling")
	}
}
