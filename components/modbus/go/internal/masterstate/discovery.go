package masterstate

import (
	"context"
	"errors"
	"fmt"
	"github.com/Karl-Dai/ModbusSim/go/internal/master"
	"strconv"
	"time"
)

func (c *connection) stopDiscovery(kind string) {
	c.mu.Lock()
	t := c.scans[kind]
	if t != nil {
		t.cancel()
	}
	c.mu.Unlock()
	if t != nil {
		<-t.done
	}
}
func (c *connection) stopDiscoveries() {
	c.mu.Lock()
	tasks := []*task{}
	for _, t := range c.scans {
		t.cancel()
		tasks = append(tasks, t)
	}
	c.mu.Unlock()
	for _, t := range tasks {
		<-t.done
	}
}
func (c *connection) newDiscovery(kind string) (*task, *master.Connection, error) {
	c.stopDiscovery(kind)
	ctx, engine, err := c.request()
	if err != nil {
		return nil, nil, err
	}
	ctx, cancel := context.WithCancel(ctx)
	t := &task{ctx: ctx, cancel: cancel, done: make(chan struct{}), id: "scan_" + strconv.FormatUint(c.owner.sequence.Add(1), 10)}
	c.mu.Lock()
	c.scans[kind] = t
	c.mu.Unlock()
	return t, engine, nil
}
func (c *connection) finishDiscovery(kind string, t *task) {
	c.mu.Lock()
	if c.scans[kind] == t {
		delete(c.scans, kind)
	}
	c.mu.Unlock()
	close(t.done)
}

// A device exception is still a response from the addressed device. Gateway
// exceptions 0x0a/0x0b do not establish that the downstream device is present.
func deviceResponded(err error) bool {
	if err == nil {
		return true
	}
	var protocol *master.Error
	if !errors.As(err, &protocol) || protocol.Kind != "exception" {
		return false
	}
	switch protocol.Exception {
	case 1, 2, 3, 4, 5, 6, 8:
		return true
	}
	return false
}

func (m *Manager) StartSlaveIDScan(id string, req SlaveScanRequest) (string, error) {
	if req.StartID < 1 || req.EndID > 247 || req.EndID < req.StartID || req.TimeoutMS < 1 || req.TimeoutMS > 600000 {
		return "", fmt.Errorf("invalid station scan range or timeout")
	}
	c, err := m.get(id)
	if err != nil {
		return "", err
	}
	c.op.Lock()
	defer c.op.Unlock()
	t, engine, err := c.newDiscovery("slave_scan")
	if err != nil {
		return "", err
	}
	go func() {
		defer c.finishDiscovery("slave_scan", t)
		found := []uint16{}
		current, completed := req.StartID, uint16(0)
		failure := ""
		send := func(done, cancelled bool) {
			p := SlaveScanEvent{ConnectionID: c.id, TaskID: t.id, CurrentID: current, Completed: completed, Total: uint16(req.EndID) - uint16(req.StartID) + 1, FoundIDs: append([]uint16{}, found...), Done: done, Cancelled: cancelled, Error: failure}
			c.publish("scan-slave-progress", p, func() bool { return c.scans["slave_scan"] == t })
		}
		for n := uint16(req.StartID); n <= uint16(req.EndID); n++ {
			if t.ctx.Err() != nil {
				break
			}
			current = uint8(n)
			_, err := engine.ReadSlave(t.ctx, current, master.ReadHoldingRegisters, 0, 1, time.Duration(req.TimeoutMS)*time.Millisecond)
			if t.ctx.Err() != nil {
				break
			}
			completed++
			if deviceResponded(err) {
				found = append(found, n)
			}
			var protocol *master.Error
			if errors.As(err, &protocol) && (protocol.Kind == "transport" || protocol.Kind == "not_connected") {
				failure = err.Error()
				break
			}
			if n != uint16(req.EndID) {
				send(false, false)
			}
		}
		send(t.ctx.Err() == nil, t.ctx.Err() != nil)
	}()
	return t.id, nil
}

func (m *Manager) StartRegisterScan(id string, req RegisterScanRequest) (string, error) {
	f, err := readFunction(req.Function)
	if err != nil {
		return "", err
	}
	if req.EndAddress < req.StartAddress || req.ChunkSize == 0 || req.ChunkSize > 2000 || req.TimeoutMS < 1 || req.TimeoutMS > 600000 {
		return "", fmt.Errorf("invalid register scan range, chunk size or timeout")
	}
	if req.SlaveID != nil && (*req.SlaveID < 1 || *req.SlaveID > 247) {
		return "", fmt.Errorf("slave ID must be between 1 and 247")
	}
	c, err := m.get(id)
	if err != nil {
		return "", err
	}
	c.op.Lock()
	defer c.op.Unlock()
	t, engine, err := c.newDiscovery("register_scan")
	if err != nil {
		return "", err
	}
	target := engine.Config.SlaveID
	if req.SlaveID != nil {
		target = *req.SlaveID
	}
	limit := engine.Config.Requests.MaxReadRegisters
	if f == master.ReadCoils || f == master.ReadDiscreteInputs {
		limit = engine.Config.Requests.MaxReadBits
	}
	chunk := min(req.ChunkSize, limit)
	go func() {
		defer c.finishDiscovery("register_scan", t)
		found := []FoundRegister{}
		current, completed := req.StartAddress, uint32(0)
		failure := ""
		lastProgress := time.Time{}
		send := func(done, cancelled bool) {
			if !done && !cancelled && time.Since(lastProgress) < 50*time.Millisecond {
				return
			}
			lastProgress = time.Now()
			p := RegisterScanEvent{ConnectionID: c.id, TaskID: t.id, CurrentAddress: current, EndAddress: req.EndAddress, Completed: completed, Total: uint32(req.EndAddress) - uint32(req.StartAddress) + 1, FoundCount: len(found), FoundRegisters: append([]FoundRegister{}, found...), Done: done, Cancelled: cancelled, Error: failure}
			c.publish("scan-register-progress", p, func() bool { return c.scans["register_scan"] == t })
		}
		var readRange func(uint32, uint16) bool
		readRange = func(address uint32, count uint16) bool {
			if t.ctx.Err() != nil {
				return false
			}
			result, err := engine.ReadSlave(t.ctx, target, f, uint16(address), count, time.Duration(req.TimeoutMS)*time.Millisecond)
			if t.ctx.Err() != nil {
				return false
			}
			if err == nil {
				dto := resultDTO(ScanGroup{Function: req.Function, StartAddress: uint16(address)}, result)
				for _, value := range dto.Values {
					found = append(found, FoundRegister{value.Address, value.RawValue})
				}
			} else {
				var protocol *master.Error
				if errors.As(err, &protocol) && protocol.Kind == "exception" && (protocol.Exception == 2 || protocol.Exception == 3) {
					// A single invalid address makes a whole read fail. Split rejected blocks
					// until valid islands and their exact boundaries have been recovered.
					if count > 1 {
						left := count / 2
						return readRange(address, left) && readRange(address+uint32(left), count-left)
					}
				} else {
					failure = err.Error()
					return false
				}
			}
			completed += uint32(count)
			current = uint16(address + uint32(count) - 1)
			send(false, false)
			return true
		}
		for address := uint32(req.StartAddress); address <= uint32(req.EndAddress); {
			count := uint16(min(uint32(chunk), uint32(req.EndAddress)-address+1))
			if !readRange(address, count) {
				break
			}
			address += uint32(count)
		}
		send(t.ctx.Err() == nil, t.ctx.Err() != nil)
	}()
	return t.id, nil
}
func (m *Manager) CancelScan(id, kind string) error {
	if kind != "slave_scan" && kind != "register_scan" {
		return fmt.Errorf("unknown scan type")
	}
	c, err := m.get(id)
	if err != nil {
		return err
	}
	c.mu.Lock()
	t := c.scans[kind]
	if t != nil {
		t.cancel()
	}
	c.mu.Unlock()
	if t != nil {
		<-t.done
	}
	return nil
}
