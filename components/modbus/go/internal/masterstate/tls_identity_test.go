package masterstate

import (
	"encoding/json"
	"os"
	"path/filepath"
	"testing"

	"github.com/Karl-Dai/ModbusSim/go/internal/project"
)

func TestTLSNameVerificationPersistsThroughSettingsAndProject(t *testing.T) {
	for _, enabled := range []bool{false, true} {
		name := "disabled"
		if enabled {
			name = "enabled"
		}
		t.Run(name, func(t *testing.T) {
			manager := NewManager(Events{})
			defer manager.CloseAll()
			settings := DefaultSettings()
			settings.Transport.Type = project.TransportTCPTLS
			settings.Transport.TLS = &project.ClientTLS{Enabled: true, CAFile: "ca.pem", VerifyServerName: enabled}
			info, err := manager.CreateConnection(settings)
			if err != nil {
				t.Fatal(err)
			}
			path := filepath.Join(t.TempDir(), "identity.modbusproj")
			if err := manager.SaveProject(path); err != nil {
				t.Fatal(err)
			}
			if _, err := manager.LoadProject(path); err != nil {
				t.Fatal(err)
			}
			got, err := manager.GetSettings(info.ID)
			if err != nil {
				t.Fatal(err)
			}
			cfg := got.engine().Config.TLS
			if cfg.VerifyServerName != enabled || cfg.AcceptInvalidCerts || cfg.CAFile != "ca.pem" {
				t.Fatalf("TLS policy changed during save/load: %+v", cfg)
			}
			// Simulate a pre-switch v2 project: the missing field defaults to false.
			data, err := os.ReadFile(path)
			if err != nil {
				t.Fatal(err)
			}
			var doc map[string]any
			if err := json.Unmarshal(data, &doc); err != nil {
				t.Fatal(err)
			}
			conn := doc["Connections"].([]any)[0].(map[string]any)
			tls := conn["Transport"].(map[string]any)["TLS"].(map[string]any)
			delete(tls, "VerifyServerName")
			data, err = json.Marshal(doc)
			if err != nil {
				t.Fatal(err)
			}
			if err := os.WriteFile(path, data, 0600); err != nil {
				t.Fatal(err)
			}
			if _, err := manager.LoadProject(path); err != nil {
				t.Fatal(err)
			}
			legacy, err := manager.GetSettings(info.ID)
			if err != nil {
				t.Fatal(err)
			}
			if legacy.engine().Config.TLS.VerifyServerName {
				t.Fatal("legacy project enabled name verification")
			}
		})
	}
}
