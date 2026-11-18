package masterstate

import (
	"encoding/binary"
	"encoding/json"
	"fmt"
	"github.com/Karl-Dai/ModbusSim/go/internal/mbap"
	"net"
	"reflect"
	"sync"
	"testing"
	"time"
)

func scanServer(t *testing.T, reply func(uint8, []byte) []byte) (*harness, func()) {
	t.Helper()
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	var mu sync.Mutex
	var clients []net.Conn
	stop := func() {
		listener.Close()
		mu.Lock()
		defer mu.Unlock()
		for _, conn := range clients {
			conn.Close()
		}
	}
	t.Cleanup(stop)
	go func() {
		for {
			conn, err := listener.Accept()
			if err != nil {
				return
			}
			mu.Lock()
			clients = append(clients, conn)
			mu.Unlock()
			go func() {
				defer conn.Close()
				for {
					header, pdu, err := mbap.ReadFrame(conn)
					if err != nil {
						return
					}
					response := reply(header.UnitID, pdu)
					if response != nil {
						if mbap.WriteFrame(conn, header.TransactionID, header.UnitID, response) != nil {
							return
						}
					}
				}
			}()
		}
	}()
	return &harness{port: uint16(listener.Addr().(*net.TCPAddr).Port)}, stop
}
func sparseReply(id uint8, request []byte) []byte {
	fc := request[0]
	if id != 1 && id != 2 && id != 7 {
		return []byte{fc | 0x80, 0x0b}
	}
	if id == 2 {
		return []byte{fc | 0x80, 0x01}
	}
	start, count := binary.BigEndian.Uint16(request[1:3]), binary.BigEndian.Uint16(request[3:5])
	valid := map[uint16]bool{2: true, 3: true, 10: true, 11: true}
	for n := uint32(start); n < uint32(start)+uint32(count); n++ {
		if !valid[uint16(n)] {
			return []byte{fc | 0x80, 0x02}
		}
	}
	response := []byte{fc, byte(count * 2)}
	for n := uint16(0); n < count; n++ {
		value := uint16(id)*1000 + start + n
		response = append(response, byte(value>>8), byte(value))
	}
	return response
}
func TestSlaveDiscoveryCountsDeviceExceptionsAndSendsNumericJSON(t *testing.T) {
	h, _ := scanServer(t, sparseReply)
	events := make(chan SlaveScanEvent, 64)
	manager, id := newTestManager(t, h, Events{Emit: func(name string, payload any) {
		if name == "scan-slave-progress" {
			events <- payload.(SlaveScanEvent)
		}
	}})
	task, err := manager.StartSlaveIDScan(id, SlaveScanRequest{StartID: 1, EndID: 8, TimeoutMS: 20})
	if err != nil {
		t.Fatal(err)
	}
	var terminal SlaveScanEvent
	eventually(t, func() bool {
		for len(events) > 0 {
			event := <-events
			if event.TaskID == task && event.Done {
				terminal = event
				return true
			}
		}
		return false
	})
	if fmt.Sprint(terminal.FoundIDs) != "[1 2 7]" {
		t.Errorf("responsive devices missed (exclude gateway failures): %v", terminal.FoundIDs)
	}
	data, err := json.Marshal(terminal)
	if err != nil {
		t.Fatal(err)
	}
	var wire struct{ FoundIDs []int }
	if err = json.Unmarshal(data, &wire); err != nil {
		t.Fatalf("scan IDs must be a JSON numeric array: %s (%v)", data, err)
	}
}
func TestRegisterDiscoveryRecoversValidAddressesInsideRejectedBlocks(t *testing.T) {
	h, _ := scanServer(t, sparseReply)
	events := make(chan RegisterScanEvent, 128)
	manager, id := newTestManager(t, h, Events{Emit: func(name string, payload any) {
		if name == "scan-register-progress" {
			events <- payload.(RegisterScanEvent)
		}
	}})
	var request RegisterScanRequest
	json.Unmarshal([]byte(`{"Function":"read_holding_registers","StartAddress":0,"EndAddress":19,"ChunkSize":10,"TimeoutMS":20,"SlaveID":7}`), &request)
	task, err := manager.StartRegisterScan(id, request)
	if err != nil {
		t.Fatal(err)
	}
	var terminal RegisterScanEvent
	eventually(t, func() bool {
		for len(events) > 0 {
			event := <-events
			if event.TaskID == task && event.Done {
				terminal = event
				return true
			}
		}
		return false
	})
	expected := []FoundRegister{{2, 7002}, {3, 7003}, {10, 7010}, {11, 7011}}
	if !reflect.DeepEqual(terminal.FoundRegisters, expected) {
		t.Fatalf("sparse scan wrong target or skipped addresses: got %+v want %+v", terminal.FoundRegisters, expected)
	}
	settings, err := manager.GetSettings(id)
	if err != nil || settings.SlaveID != 1 {
		t.Fatalf("scan changed default: %+v %v", settings, err)
	}
}
func TestDiscoveryCancelInterruptsAWaitingResponse(t *testing.T) {
	received := make(chan struct{}, 1)
	h, stop := scanServer(t, func(_ uint8, _ []byte) []byte {
		select {
		case received <- struct{}{}:
		default:
		}
		return nil
	})
	manager, id := newTestManager(t, h, Events{})
	_, err := manager.StartSlaveIDScan(id, SlaveScanRequest{StartID: 1, EndID: 2, TimeoutMS: 60000})
	if err != nil {
		t.Fatal(err)
	}
	select {
	case <-received:
	case <-time.After(time.Second):
		t.Fatal("no scan request")
	}
	cancelled := make(chan error, 1)
	go func() { cancelled <- manager.CancelScan(id, "slave_scan") }()
	select {
	case err := <-cancelled:
		if err != nil {
			t.Fatal(err)
		}
	case <-time.After(500 * time.Millisecond):
		stop()
		<-cancelled
		t.Fatal("cancel waits for full timeout")
	}
}
