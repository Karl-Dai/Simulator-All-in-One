package master

import (
	"context"
	"crypto/tls"
	"github.com/Karl-Dai/ModbusSim/go/internal/socks5"
	"io"
	"log"
	"net"
	"net/http/httptest"
	"testing"
	"time"
)

func TestTLSVersionNegotiation(t *testing.T) {
	for _, serverVersion := range []uint16{tls.VersionTLS12, tls.VersionTLS13} {
		for _, selected := range []string{"", "auto", "1.2", "1.3"} {
			t.Run(tls.VersionName(serverVersion)+"/"+selected, func(t *testing.T) {
				srv := httptest.NewUnstartedServer(nil)
				srv.Config.ErrorLog = log.New(io.Discard, "", 0)
				srv.TLS = &tls.Config{MinVersion: serverVersion, MaxVersion: serverVersion}
				srv.StartTLS()
				defer srv.Close()
				addr := srv.Listener.Addr().(*net.TCPAddr)
				tr, err := newTLSTransport(context.Background(), "127.0.0.1", uint16(addr.Port), TLSConfig{Version: selected, AcceptInvalidCerts: true}, socks5.Config{}, time.Second)
				mismatch := (selected == "1.2" && serverVersion == tls.VersionTLS13) || (selected == "1.3" && serverVersion == tls.VersionTLS12)
				if mismatch {
					if err == nil {
						tr.close()
						t.Fatal("fixed version silently negotiated another version")
					}
					return
				}
				if err != nil {
					t.Fatal(err)
				}
				defer tr.close()
				if got := tr.conn.ConnectionState().Version; got != serverVersion {
					t.Fatalf("negotiated %x, want %x", got, serverVersion)
				}
			})
		}
	}
	if _, err := BuildClientTLSConfig(TLSConfig{Version: "1.1"}, "localhost"); err == nil {
		t.Fatal("unsupported version accepted")
	}
}
