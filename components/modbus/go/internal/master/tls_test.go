package master

import (
	"crypto/ecdsa"
	"crypto/elliptic"
	"crypto/rand"
	"crypto/tls"
	"crypto/x509"
	"crypto/x509/pkix"
	"encoding/pem"
	"errors"
	"math/big"
	"net"
	"os"
	"path/filepath"
	"testing"
	"time"
)

type testTLSAuthority struct {
	cert *x509.Certificate
	key  *ecdsa.PrivateKey
}

func newTestTLSAuthority(t *testing.T, name string, parent *testTLSAuthority) testTLSAuthority {
	t.Helper()
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	template := &x509.Certificate{
		SerialNumber:          big.NewInt(1),
		Subject:               pkix.Name{CommonName: name},
		IsCA:                  true,
		BasicConstraintsValid: true,
		KeyUsage:              x509.KeyUsageCertSign | x509.KeyUsageDigitalSignature,
		NotBefore:             time.Now().Add(-72 * time.Hour),
		NotAfter:              time.Now().Add(72 * time.Hour),
	}
	signer, signingKey := template, key
	if parent != nil {
		signer, signingKey = parent.cert, parent.key
	}
	der, err := x509.CreateCertificate(rand.Reader, template, signer, &key.PublicKey, signingKey)
	if err != nil {
		t.Fatal(err)
	}
	cert, err := x509.ParseCertificate(der)
	if err != nil {
		t.Fatal(err)
	}
	return testTLSAuthority{cert: cert, key: key}
}

func testTLSServerIdentity(t *testing.T, issuer testTLSAuthority, mutate func(*x509.Certificate)) tls.Certificate {
	t.Helper()
	key, err := ecdsa.GenerateKey(elliptic.P256(), rand.Reader)
	if err != nil {
		t.Fatal(err)
	}
	template := &x509.Certificate{
		SerialNumber: big.NewInt(2),
		Subject:      pkix.Name{CommonName: "legacy-device"},
		KeyUsage:     x509.KeyUsageDigitalSignature,
		ExtKeyUsage:  []x509.ExtKeyUsage{x509.ExtKeyUsageServerAuth},
		NotBefore:    time.Now().Add(-time.Hour),
		NotAfter:     time.Now().Add(time.Hour),
	}
	if mutate != nil {
		mutate(template)
	}
	der, err := x509.CreateCertificate(rand.Reader, template, issuer.cert, &key.PublicKey, issuer.key)
	if err != nil {
		t.Fatal(err)
	}
	return tls.Certificate{Certificate: [][]byte{der, issuer.cert.Raw}, PrivateKey: key}
}

func writeTestTLSCA(t *testing.T, ca testTLSAuthority) string {
	t.Helper()
	file := filepath.Join(t.TempDir(), "ca.pem")
	if err := os.WriteFile(file, pem.EncodeToMemory(&pem.Block{Type: "CERTIFICATE", Bytes: ca.cert.Raw}), 0o600); err != nil {
		t.Fatal(err)
	}
	return file
}

// Perform a real handshake through the same config builder used by the
// transport. The server presents an intermediate, not just a trusted leaf.
func testTLSHandshake(t *testing.T, identity tls.Certificate, config TLSConfig, serverName string) (error, string) {
	t.Helper()
	clientConfig, err := BuildClientTLSConfig(config, serverName)
	if err != nil {
		t.Fatal(err)
	}
	listener, err := net.Listen("tcp", "127.0.0.1:0")
	if err != nil {
		t.Fatal(err)
	}
	defer listener.Close()
	type result struct {
		err error
		sni string
	}
	done := make(chan result, 1)
	go func() {
		raw, acceptErr := listener.Accept()
		if acceptErr != nil {
			done <- result{err: acceptErr}
			return
		}
		defer raw.Close()
		_ = raw.SetDeadline(time.Now().Add(3 * time.Second))
		var sni string
		server := tls.Server(raw, &tls.Config{
			Certificates: []tls.Certificate{identity},
			MinVersion:   tls.VersionTLS12,
			GetConfigForClient: func(hello *tls.ClientHelloInfo) (*tls.Config, error) {
				sni = hello.ServerName
				return nil, nil
			},
		})
		handshakeErr := server.Handshake()
		done <- result{err: handshakeErr, sni: sni}
	}()
	client, handshakeErr := tls.DialWithDialer(&net.Dialer{Timeout: 3 * time.Second}, "tcp", listener.Addr().String(), clientConfig)
	if client != nil {
		client.Close()
	}
	serverResult := <-done
	if handshakeErr == nil && serverResult.err != nil {
		t.Fatalf("server handshake failed after client success: %v", serverResult.err)
	}
	return handshakeErr, serverResult.sni
}

func TestTLSVerifyServerNameIsOptIn(t *testing.T) {
	ca := newTestTLSAuthority(t, "root", nil)
	intermediate := newTestTLSAuthority(t, "intermediate", &ca)
	identity := testTLSServerIdentity(t, intermediate, nil)
	config := TLSConfig{CAFile: writeTestTLSCA(t, ca)}
	if err, _ := testTLSHandshake(t, identity, config, "127.0.0.1"); err != nil {
		t.Fatalf("default chain verification should accept the trusted certificate without SAN: %v", err)
	}
	config.VerifyServerName = true
	err, _ := testTLSHandshake(t, identity, config, "127.0.0.1")
	var hostnameError x509.HostnameError
	if !errors.As(err, &hostnameError) {
		t.Fatalf("enabled name verification: got %v, want x509.HostnameError", err)
	}
	identity = testTLSServerIdentity(t, intermediate, func(cert *x509.Certificate) {
		cert.IPAddresses = []net.IP{net.ParseIP("127.0.0.1")}
	})
	if err, _ := testTLSHandshake(t, identity, config, "127.0.0.1"); err != nil {
		t.Fatalf("enabled name verification should accept matching IP SAN: %v", err)
	}
}

func TestTLSNameVerificationDisabledStillChecksCertificate(t *testing.T) {
	ca := newTestTLSAuthority(t, "root", nil)
	intermediate := newTestTLSAuthority(t, "intermediate", &ca)
	otherCA := newTestTLSAuthority(t, "untrusted-root", nil)
	trustedFile, untrustedFile := writeTestTLSCA(t, ca), writeTestTLSCA(t, otherCA)
	cases := []struct {
		name   string
		caFile string
		mutate func(*x509.Certificate)
		check  func(error) bool
	}{
		{
			name: "unknown CA", caFile: untrustedFile,
			check: func(err error) bool { var target x509.UnknownAuthorityError; return errors.As(err, &target) },
		},
		{
			name: "expired", caFile: trustedFile,
			mutate: func(cert *x509.Certificate) {
				cert.NotBefore, cert.NotAfter = time.Now().Add(-48*time.Hour), time.Now().Add(-24*time.Hour)
			},
			check: func(err error) bool {
				var target x509.CertificateInvalidError
				return errors.As(err, &target) && target.Reason == x509.Expired
			},
		},
		{
			name: "not yet valid", caFile: trustedFile,
			mutate: func(cert *x509.Certificate) {
				cert.NotBefore, cert.NotAfter = time.Now().Add(24*time.Hour), time.Now().Add(48*time.Hour)
			},
			check: func(err error) bool {
				var target x509.CertificateInvalidError
				return errors.As(err, &target) && target.Reason == x509.Expired
			},
		},
		{
			name: "client-only usage", caFile: trustedFile,
			mutate: func(cert *x509.Certificate) { cert.ExtKeyUsage = []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth} },
			check: func(err error) bool {
				var target x509.CertificateInvalidError
				return errors.As(err, &target) && target.Reason == x509.IncompatibleUsage
			},
		},
	}
	for _, test := range cases {
		t.Run(test.name, func(t *testing.T) {
			identity := testTLSServerIdentity(t, intermediate, test.mutate)
			err, _ := testTLSHandshake(t, identity, TLSConfig{CAFile: test.caFile}, "127.0.0.1")
			if !test.check(err) {
				t.Fatalf("got %v, want the corresponding certificate validation error", err)
			}
		})
	}
}

func TestTLSNameVerificationDisabledPreservesSNI(t *testing.T) {
	ca := newTestTLSAuthority(t, "root", nil)
	identity := testTLSServerIdentity(t, ca, nil)
	err, sni := testTLSHandshake(t, identity, TLSConfig{CAFile: writeTestTLSCA(t, ca)}, "device.example.invalid")
	if err != nil {
		t.Fatal(err)
	}
	if sni != "device.example.invalid" {
		t.Fatalf("SNI = %q, want the dial hostname", sni)
	}
}

func TestTLSAcceptInvalidCertsKeepsExplicitFullBypass(t *testing.T) {
	ca := newTestTLSAuthority(t, "untrusted-root", nil)
	identity := testTLSServerIdentity(t, ca, func(cert *x509.Certificate) {
		cert.ExtKeyUsage = []x509.ExtKeyUsage{x509.ExtKeyUsageClientAuth}
		cert.NotBefore, cert.NotAfter = time.Now().Add(-48*time.Hour), time.Now().Add(-24*time.Hour)
	})
	for _, verifyName := range []bool{false, true} {
		err, _ := testTLSHandshake(t, identity, TLSConfig{AcceptInvalidCerts: true, VerifyServerName: verifyName}, "127.0.0.1")
		if err != nil {
			t.Fatalf("explicit full bypass (VerifyServerName=%t): %v", verifyName, err)
		}
	}
}

func TestTLSNameVerificationDisabledWithSystemRoots(t *testing.T) {
	config, err := BuildClientTLSConfig(TLSConfig{}, "device.example.invalid")
	if err != nil {
		t.Fatal(err)
	}
	if config.RootCAs != nil || config.VerifyConnection == nil {
		t.Fatal("default mode must use system roots with certificate verification")
	}
	if err := config.VerifyConnection(tls.ConnectionState{}); err == nil {
		t.Fatal("missing peer certificate must not be accepted")
	}
}
