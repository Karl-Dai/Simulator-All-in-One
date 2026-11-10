// Package webviewruntime handles Windows prerequisites before creating a webview.
package webviewruntime

const DownloadURL = "https://simlab.carldai.cloud/downloads/webview2/"

// gate keeps cancellation and opening the download page separate from launching.
func gate(available bool, ask func() bool, open func(string) error) (bool, error) {
	if available {
		return true, nil
	}
	if ask() {
		return false, open(DownloadURL)
	}
	return false, nil
}
