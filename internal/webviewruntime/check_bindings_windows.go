//go:build windows && bindings

package webviewruntime

// Wails executes main while exporting bindings. This build mode must never
// inspect the desktop runtime or block on a native prerequisite dialog.
func Ensure() bool { return true }
