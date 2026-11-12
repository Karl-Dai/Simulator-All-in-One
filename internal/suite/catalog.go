package suite

// Role is a stable product identity, independent of display language or filename.
type Role struct {
	ID       string `json:"id"`
	Protocol string `json:"protocol"`
	Name     string `json:"name"`
	Product  string `json:"product"`
}

var Roles = []Role{
	{"iec104.master", "iec104", "主站", "IEC104Master"},
	{"iec104.slave", "iec104", "子站", "IEC104Slave"},
	{"iec101.master", "iec101", "主站", "IEC101Master"},
	{"iec101.slave", "iec101", "子站", "IEC101Slave"},
	{"iec102.master", "iec102", "主站", "IEC102Master"},
	{"iec102.slave", "iec102", "子站", "IEC102Slave"},
	{"modbus.master", "modbus", "主站", "ModbusMaster"},
	{"modbus.slave", "modbus", "从站", "ModbusSlave"},
	{"pmu.master", "pmu", "主站", "PmuSim"},
	{"pmu.sub", "pmu", "子站", "PmuSub"},
	{"opcua.master", "opcua", "客户端", "OPCUAMaster"},
	{"opcua.server", "opcua", "服务端", "OPCUAServer"},
}

func FindRole(id string) (Role, bool) {
	for _, role := range Roles {
		if role.ID == id {
			return role, true
		}
	}
	return Role{}, false
}
