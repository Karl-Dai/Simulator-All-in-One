package webviewruntime

import (
	"errors"
	"testing"
)

func TestRuntimeReadyDoesNotPrompt(t *testing.T) {
	ok, err := gate(true, func() bool { t.Fatal("unexpected prompt"); return false }, func(string) error { t.Fatal("unexpected browser"); return nil })
	if !ok || err != nil {
		t.Fatalf("ready: %v %v", ok, err)
	}
}
func TestMissingRuntimeDownloadAndCancelNeverLaunch(t *testing.T) {
	for _, accept := range []bool{true, false} {
		opened := false
		ok, err := gate(false, func() bool { return accept }, func(url string) error {
			opened = true
			if url != DownloadURL {
				t.Fatal(url)
			}
			return nil
		})
		if ok || err != nil || opened != accept {
			t.Fatalf("accept=%v: launch=%v opened=%v err=%v", accept, ok, opened, err)
		}
	}
}
func TestDownloadFailureIsReported(t *testing.T) {
	failure := errors.New("browser unavailable")
	ok, err := gate(false, func() bool { return true }, func(string) error { return failure })
	if ok || !errors.Is(err, failure) {
		t.Fatalf("%v %v", ok, err)
	}
}
