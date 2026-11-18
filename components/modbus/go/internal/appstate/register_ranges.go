package appstate

import (
	"fmt"
	"math/rand"
	"sort"

	"github.com/Karl-Dai/ModbusSim/go/internal/register"
	"github.com/Karl-Dai/ModbusSim/go/internal/slave"
)

// RegisterRange describes inclusive raw addresses, not the number of typed points.
// int fields ensure invalid IPC input is rejected before narrowing to uint16.
type RegisterRange struct {
	RegisterType string
	StartAddress int
	EndAddress   int
	DataType     string
	Endian       string
}

func rangeDefinitions(ranges []RegisterRange) ([]register.RegisterDef, error) {
	ordered := append([]RegisterRange{}, ranges...)
	sort.Slice(ordered, func(i, j int) bool {
		if ordered[i].RegisterType != ordered[j].RegisterType {
			return ordered[i].RegisterType < ordered[j].RegisterType
		}
		return ordered[i].StartAddress < ordered[j].StartAddress
	})
	// Validate intervals before expanding: even repeated full-area ranges cannot
	// allocate an unbounded definition set.
	for i, r := range ordered {
		if r.StartAddress < 0 || r.EndAddress > 65535 || r.EndAddress < r.StartAddress {
			return nil, fmt.Errorf("range %s: addresses must satisfy 0 <= start <= end <= 65535", r.RegisterType)
		}
		def, err := registerDef(AddRegisterRequest{RegisterType: r.RegisterType, DataType: r.DataType, Endian: r.Endian})
		if err != nil {
			return nil, err
		}
		bitArea := def.RegisterType == register.Coil || def.RegisterType == register.DiscreteInput
		if (bitArea && def.DataType != register.TypeBool) || (!bitArea && def.DataType == register.TypeBool) {
			return nil, fmt.Errorf("range %s: use bool for bit areas and a numeric type for register areas", r.RegisterType)
		}
		width := int(def.DataType.RegisterCount())
		if (r.EndAddress-r.StartAddress+1)%width != 0 {
			return nil, fmt.Errorf("range %s@%d..%d cannot contain complete %s points", r.RegisterType, r.StartAddress, r.EndAddress, r.DataType)
		}
		if i > 0 && ordered[i-1].RegisterType == r.RegisterType && ordered[i-1].EndAddress >= r.StartAddress {
			return nil, fmt.Errorf("range %s@%d overlaps another range", r.RegisterType, r.StartAddress)
		}
	}
	defs := make([]register.RegisterDef, 0)
	for _, r := range ordered {
		def, _ := registerDef(AddRegisterRequest{RegisterType: r.RegisterType, DataType: r.DataType, Endian: r.Endian})
		for address := r.StartAddress; address <= r.EndAddress; address += int(def.DataType.RegisterCount()) {
			def.Address = uint16(address)
			defs = append(defs, def)
		}
	}
	return defs, nil
}

func createDevice(id uint8, name, initMode string, ranges []RegisterRange, legacyConnection bool) (*slave.Device, error) {
	// Preserve older IPC clients. Both current dialogs always send an explicit
	// array, including [] for an empty device. Project loading uses saved points.
	if ranges == nil {
		if initMode == "random" {
			return slave.WithRandomRegisters(id, name, defaultMaxAddress), nil
		}
		if legacyConnection || initMode == "zero" {
			return slave.WithDefaultRegisters(id, name, defaultMaxAddress), nil
		}
		return slave.NewDevice(id, name), nil
	}
	if initMode != "" && initMode != "zero" && initMode != "random" {
		return nil, fmt.Errorf("unknown initialization mode %q", initMode)
	}
	defs, err := rangeDefinitions(ranges)
	if err != nil {
		return nil, err
	}
	d := slave.NewDevice(id, name)
	d.RegisterDefs = defs
	for _, def := range defs {
		d.RegisterMap.EnsureFromDef(def)
		if initMode != "random" {
			continue
		}
		value := float64(rand.Intn(65536))
		switch def.DataType {
		case register.TypeBool:
			value = float64(rand.Intn(2))
		case register.TypeInt16:
			value -= 32768
		case register.TypeUInt32:
			value = float64(rand.Uint32())
		case register.TypeInt32:
			value = float64(int32(rand.Uint32()))
		case register.TypeFloat:
			value = rand.Float64()*2000 - 1000
		}
		words, err := register.EncodeValue(value, def.DataType, def.Endian)
		if err != nil {
			return nil, err
		}
		for offset, word := range words {
			writeRaw(d.RegisterMap, def.RegisterType, def.Address+uint16(offset), word)
		}
	}
	return d, nil
}
