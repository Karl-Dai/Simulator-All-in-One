package appstate

import (
	"bytes"
	"fmt"
	"github.com/Karl-Dai/ModbusSim/go/internal/mbap"
	"github.com/Karl-Dai/ModbusSim/go/internal/register"
	"math"
	"net"
	"path/filepath"
	"testing"
	"time"
)

func TestExplicitRegisterRangesWireAndProject(t *testing.T) {
	m := NewManager(Events{})
	defer m.CloseAll()
	port := freePort(t)
	ranges := []RegisterRange{
		{"coil", 0, 15, "bool", "big"},
		{"discrete_input", 0, 1, "bool", "big"},
		{"holding_register", 0, 9, "uint16", "big"},
		{"holding_register", 100, 119, "float32", "little"},
		{"input_register", 65534, 65535, "uint32", "mid_big"},
	}
	info, err := m.CreateConnection(CreateConnectionRequest{Transport: tcpTransport(port), SlaveID: 1, RegisterRanges: ranges})
	if err != nil {
		t.Fatal(err)
	}
	defs, err := m.ListRegisters(info.ID, 1, "")
	if err != nil || len(defs) != 39 {
		t.Fatalf("definitions: %d %v", len(defs), err)
	}
	if err = m.StartConnection(info.ID); err != nil {
		t.Fatal(err)
	}
	peer, err := net.DialTimeout("tcp", fmt.Sprintf("127.0.0.1:%d", port), time.Second)
	if err != nil {
		t.Fatal(err)
	}
	defer peer.Close()
	_ = peer.SetDeadline(time.Now().Add(5 * time.Second))
	exchange := func(request, want []byte) {
		t.Helper()
		if err := mbap.WriteFrame(peer, 1, 1, request); err != nil {
			t.Fatal(err)
		}
		_, got, err := mbap.ReadFrame(peer)
		if err != nil || !bytes.Equal(got, want) {
			t.Fatalf("request %x: got %x want %x (%v)", request, got, want, err)
		}
	}
	exchange([]byte{6, 0, 0, 0, 42}, []byte{6, 0, 0, 0, 42})
	exchange([]byte{3, 0, 0, 0, 1}, []byte{3, 2, 0, 42})
	exchange([]byte{16, 0, 1, 0, 2, 4, 0, 7, 0, 8}, []byte{16, 0, 1, 0, 2})
	exchange([]byte{3, 0, 0, 0, 3}, []byte{3, 6, 0, 42, 0, 7, 0, 8})
	exchange([]byte{5, 0, 0, 255, 0}, []byte{5, 0, 0, 255, 0})
	exchange([]byte{1, 0, 0, 0, 1}, []byte{1, 1, 1})
	exchange([]byte{15, 0, 1, 0, 2, 1, 3}, []byte{15, 0, 1, 0, 2})
	exchange([]byte{1, 0, 0, 0, 3}, []byte{1, 1, 7})
	exchange([]byte{2, 0, 0, 0, 1}, []byte{2, 1, 0})
	exchange([]byte{4, 255, 254, 0, 2}, []byte{4, 4, 0, 0, 0, 0})
	exchange([]byte{3, 0, 9, 0, 2}, []byte{0x83, 2})   // crosses a hole
	exchange([]byte{6, 0, 10, 0, 42}, []byte{0x86, 2}) // not configured
	exchange([]byte{4, 0, 0, 0, 1}, []byte{0x84, 2})   // same address, different area
	path := filepath.Join(t.TempDir(), "ranges.modbusproj")
	if err := m.SaveProject(path); err != nil {
		t.Fatal(err)
	}
	if _, err := m.LoadProject(path); err != nil {
		t.Fatal(err)
	}
	loaded, err := m.ListRegisters(info.ID, 1, "")
	if err != nil || len(loaded) != 39 {
		t.Fatalf("loaded: %d %v", len(loaded), err)
	}
	value, err := m.ReadRegister(info.ID, 1, "holding_register", 0)
	if err != nil || value.Value != 42 {
		t.Fatalf("saved value: %+v %v", value, err)
	}
	empty, err := m.AddDevice(AddDeviceRequest{ConnectionID: info.ID, SlaveID: 2, InitMode: "zero", RegisterRanges: []RegisterRange{}})
	if err != nil || empty.RegisterCount != 0 {
		t.Fatalf("empty device: %+v %v", empty, err)
	}
	newEmpty, err := m.CreateConnection(CreateConnectionRequest{Transport: tcpTransport(freePort(t)), SlaveID: 1, InitMode: "random", RegisterRanges: []RegisterRange{}})
	if err != nil {
		t.Fatal(err)
	}
	emptyDefs, err := m.ListRegisters(newEmpty.ID, 1, "")
	if err != nil || len(emptyDefs) != 0 {
		t.Fatalf("empty connection: %d %v", len(emptyDefs), err)
	}
}

func TestRegisterRangeValidationIsAtomic(t *testing.T) {
	cases := map[string][]RegisterRange{
		"negative":     {{"coil", -1, 2, "bool", "big"}},
		"overflow":     {{"holding_register", 65534, 65536, "uint16", "big"}},
		"reversed":     {{"holding_register", 2, 1, "uint16", "big"}},
		"incomplete":   {{"holding_register", 0, 2, "float32", "big"}},
		"overlap":      {{"holding_register", 0, 1, "uint32", "big"}, {"holding_register", 1, 2, "uint16", "big"}},
		"bit type":     {{"coil", 0, 1, "uint16", "big"}},
		"word type":    {{"input_register", 0, 1, "bool", "big"}},
		"unknown area": {{"other", 0, 1, "uint16", "big"}},
		"endian":       {{"input_register", 0, 1, "uint16", "bad"}},
	}
	for name, ranges := range cases {
		t.Run(name, func(t *testing.T) {
			m := NewManager(Events{})
			defer m.CloseAll()
			if _, err := m.CreateConnection(CreateConnectionRequest{Transport: tcpTransport(5020), SlaveID: 1, RegisterRanges: ranges}); err == nil {
				t.Fatal("accepted invalid range")
			}
			if len(m.ListConnections()) != 0 {
				t.Fatal("failed creation left a connection")
			}
			conn, err := m.CreateConnection(CreateConnectionRequest{Transport: tcpTransport(5020), SlaveID: 1, RegisterRanges: []RegisterRange{}})
			if err != nil {
				t.Fatal(err)
			}
			if _, err = m.AddDevice(AddDeviceRequest{ConnectionID: conn.ID, SlaveID: 2, RegisterRanges: ranges}); err == nil {
				t.Fatal("accepted invalid device")
			}
			devices, err := m.ListDevices(conn.ID)
			if err != nil || len(devices) != 1 {
				t.Fatalf("failed add changed devices: %+v %v", devices, err)
			}
		})
	}
}

func TestRandomTypedRangesAndMaximumAddress(t *testing.T) {
	ranges := []RegisterRange{{"coil", 65535, 65535, "bool", "big"}}
	for i, typ := range []string{"uint16", "int16", "uint32", "int32", "float32"} {
		ranges = append(ranges, RegisterRange{"holding_register", i * 20, i*20 + 19, typ, "mid_little"})
	}
	d, err := createDevice(1, "random", "random", ranges, false)
	if err != nil {
		t.Fatal(err)
	}
	anyNonZero := false
	for _, def := range d.RegisterDefs {
		if def.RegisterType != register.HoldingRegType {
			continue
		}
		words := d.RegisterMap.ReadHoldingRegisters(def.Address, def.DataType.RegisterCount())
		value, err := register.DecodeValue(words, def.DataType, def.Endian)
		if err != nil || math.IsNaN(value) || math.IsInf(value, 0) {
			t.Fatalf("invalid random value: %v %v", value, err)
		}
		anyNonZero = anyNonZero || value != 0
	}
	if !anyNonZero {
		t.Fatal("random points are all zero")
	}
	defs, err := rangeDefinitions([]RegisterRange{{"holding_register", 0, 65535, "uint16", "big"}})
	if err != nil || len(defs) != 65536 {
		t.Fatalf("full area: %d %v", len(defs), err)
	}
}
