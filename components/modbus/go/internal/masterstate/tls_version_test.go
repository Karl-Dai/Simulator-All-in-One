package masterstate

import (
	"encoding/json"
	"github.com/Karl-Dai/ModbusSim/go/internal/project"
	"testing"
)

func TestTLSVersionSettingsRoundTrip(t *testing.T) {
	for _, version := range []string{"", "1.2", "1.3"} {
		s := DefaultSettings()
		s.Transport.Type = project.TransportTCPTLS
		s.Transport.TLS = &project.ClientTLS{Enabled: true, Version: version, VerifyServerName: true}
		encoded, err := json.Marshal(s.project("tls", nil))
		if err != nil {
			t.Fatal(err)
		}
		var c project.Connection
		if err = json.Unmarshal(encoded, &c); err != nil {
			t.Fatal(err)
		}
		s.Transport = c.Transport
		if err = s.validate(); err != nil {
			t.Fatal(err)
		}
		if !s.engine().Config.TLS.VerifyServerName {
			t.Fatal("server name setting lost")
		}
		if got := s.engine().Config.TLS.Version; got != version {
			t.Fatalf("version lost during save/load: got %q want %q", got, version)
		}
	}
	s := DefaultSettings()
	s.Transport.Type = project.TransportTCPTLS
	s.Transport.TLS = &project.ClientTLS{Version: "1.1"}
	if err := s.validate(); err == nil {
		t.Fatal("invalid version accepted")
	}
}
