//go:build windows && bindings

package webviewruntime

import "testing"

func TestBindingGenerationDoesNotNeedDesktopRuntime(t *testing.T) {
	if !Ensure() {
		t.Fatal("binding generation must continue without WebView2")
	}
}
