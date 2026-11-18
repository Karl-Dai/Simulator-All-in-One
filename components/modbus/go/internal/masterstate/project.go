package masterstate

import (
	"fmt"
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"strconv"
	"strings"
)

func (m *Manager) Project() (*project.Project, error) {
	m.workspace.Lock()
	defer m.workspace.Unlock()
	m.mu.Lock()
	if m.closed {
		m.mu.Unlock()
		return nil, fmt.Errorf("workspace closed")
	}
	list := []*connection{}
	for _, id := range m.order {
		list = append(list, m.connections[id])
	}
	m.mu.Unlock()
	p := project.New(project.TypeMaster)
	for _, c := range list {
		c.mu.Lock()
		s := clone(c.settings)
		groups := clone(c.groups)
		c.mu.Unlock()
		pg := []*project.ScanGroup{}
		for _, g := range groups {
			f, err := readFunction(g.Function)
			if err != nil {
				return nil, err
			}
			sid := s.SlaveID
			if g.SlaveID != nil {
				sid = *g.SlaveID
			}
			pg = append(pg, &project.ScanGroup{UseDefaultSlaveID: g.SlaveID == nil, ID: g.ID, Name: g.Name, SlaveID: sid, FunctionCode: f.FCByte(), StartAddress: g.StartAddress, Count: g.Quantity, IntervalMS: g.IntervalMS, Enabled: g.Enabled})
		}
		p.Connections = append(p.Connections, s.project(c.id, pg))
	}
	return p, nil
}
func (m *Manager) SaveProject(path string) error {
	p, err := m.Project()
	if err != nil {
		return err
	}
	return project.SaveFile(path, p)
}
func (m *Manager) LoadProject(path string) (int, error) {
	p, err := project.LoadFile(path)
	if err != nil {
		return 0, err
	}
	return m.ApplyProject(p)
}

// ApplyProject restores configurations stopped; it never reconnects or starts polling.
func (m *Manager) ApplyProject(p *project.Project) (int, error) {
	if err := p.Validate(); err != nil {
		return 0, err
	}
	if p.Type != project.TypeMaster {
		return 0, fmt.Errorf("project is not a master project")
	}
	loaded := map[string]*connection{}
	order := []string{}
	next := uint64(1)
	total := 0
	for _, pc := range p.Connections {
		s := Settings{Transport: pc.Transport, SlaveID: pc.DefaultSlaveID, TimeoutMS: pc.TimeoutMS, Requests: pc.Requests, Reconnect: pc.Reconnect, Socks5: pc.Socks5}
		if err := s.validate(); err != nil {
			return 0, fmt.Errorf("connection %s: %w", pc.ID, err)
		}
		if strings.TrimSpace(pc.ID) == "" || loaded[pc.ID] != nil {
			return 0, fmt.Errorf("invalid or duplicate connection ID")
		}
		c := m.newConnection(pc.ID, s)
		seen := map[string]bool{}
		for _, pg := range pc.ScanGroups {
			fn, err := functionName(pg.FunctionCode)
			if err != nil {
				return 0, err
			}
			gid := pg.ID
			if gid == "" {
				gid = "group_" + strconv.FormatUint(m.sequence.Add(1), 10)
			}
			if seen[gid] {
				return 0, fmt.Errorf("duplicate scan group ID %s", gid)
			}
			seen[gid] = true
			if n, err := strconv.ParseUint(strings.TrimPrefix(gid, "group_"), 10, 64); err == nil {
				for old := m.sequence.Load(); n > old; old = m.sequence.Load() {
					if m.sequence.CompareAndSwap(old, n) {
						break
					}
				}
			}
			sid := pg.SlaveID
			g := ScanGroup{ID: gid, Name: pg.Name, Function: fn, StartAddress: pg.StartAddress, Quantity: pg.Count, IntervalMS: pg.IntervalMS, Enabled: pg.Enabled, SlaveID: &sid}
			if pg.UseDefaultSlaveID {
				g.SlaveID = nil
			}
			if _, err := g.engine(); err != nil {
				return 0, err
			}
			c.groups = append(c.groups, g)
			total++
		}
		loaded[pc.ID] = c
		order = append(order, pc.ID)
		if n, err := strconv.ParseUint(strings.TrimPrefix(pc.ID, "master_"), 10, 64); err == nil && n >= next {
			next = n + 1
		}
	}
	m.workspace.Lock()
	defer m.workspace.Unlock()
	m.mu.Lock()
	if m.closed {
		m.mu.Unlock()
		return 0, fmt.Errorf("workspace closed")
	}
	m.replacing = true
	old := []*connection{}
	for _, c := range m.connections {
		old = append(old, c)
	}
	m.mu.Unlock()
	for _, c := range old {
		c.cancel()
	}
	for _, c := range old {
		c.disconnect(true)
	}
	m.mu.Lock()
	m.connections = loaded
	m.order = order
	m.nextID = next
	m.replacing = false
	m.mu.Unlock()
	return total, nil
}

// RestoreConnections rolls back only configuration targets. Other connections
// retain their engines, polling tasks and logs. Restored targets stay stopped.
func (m *Manager) RestoreConnections(p *project.Project, ids []string) error {
	staged := NewManager(Events{})
	if _, err := staged.ApplyProject(p); err != nil {
		return err
	}
	selected := map[string]bool{}
	for _, id := range ids {
		selected[id] = true
	}
	m.workspace.Lock()
	defer m.workspace.Unlock()
	m.mu.Lock()
	old := []*connection{}
	for id := range selected {
		if c := m.connections[id]; c != nil {
			old = append(old, c)
		}
	}
	m.mu.Unlock()
	for _, c := range old {
		c.cancel()
		c.disconnect(true)
	}
	m.mu.Lock()
	for id := range selected {
		delete(m.connections, id)
		if c := staged.connections[id]; c != nil {
			c.owner = m
			m.connections[id] = c
		}
	}
	order := []string{}
	for _, c := range p.Connections {
		if m.connections[c.ID] != nil {
			order = append(order, c.ID)
		}
	}
	m.order = order
	if staged.nextID > m.nextID {
		m.nextID = staged.nextID
	}
	m.mu.Unlock()
	for id := range selected {
		if m.events.Emit != nil {
			m.events.Emit("master-connection-state", ConnectionStateEvent{id, "Disconnected"})
		}
	}
	return nil
}
