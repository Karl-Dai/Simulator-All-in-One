//go:build windows && !bindings

package webviewruntime

import (
	"fmt"
	"syscall"
	"unsafe"

	"github.com/wailsapp/go-webview2/webviewloader"
)

var user32 = syscall.NewLazyDLL("user32.dll")

func messageBox(message string, flags uintptr) uintptr {
	text, _ := syscall.UTF16PtrFromString(message)
	title, _ := syscall.UTF16PtrFromString("SAIO — Microsoft WebView2")
	result, _, _ := user32.NewProc("MessageBoxW").Call(0, uintptr(unsafe.Pointer(text)), uintptr(unsafe.Pointer(title)), flags)
	return result
}

func openDownload(url string) error {
	action, _ := syscall.UTF16PtrFromString("open")
	target, _ := syscall.UTF16PtrFromString(url)
	result, _, _ := syscall.NewLazyDLL("shell32.dll").NewProc("ShellExecuteW").Call(0, uintptr(unsafe.Pointer(action)), uintptr(unsafe.Pointer(target)), 0, 0, 1)
	if result <= 32 {
		return fmt.Errorf("cannot open the download page (Windows code %d): %s", result, url)
	}
	return nil
}

// Ensure uses the same loader and minimum version as Wails 2.14.0. It runs
// before any Vue/Wails UI, so the missing-runtime prompt works on clean hosts.
func Ensure() bool {
	version, err := webviewloader.GetAvailableCoreWebView2BrowserVersionString("")
	available := err == nil && version != ""
	if available {
		comparison, compareErr := webviewloader.CompareBrowserVersions(version, "94.0.992.31")
		available = compareErr == nil && comparison >= 0
	}
	ok, err := gate(available, func() bool {
		language, _, _ := syscall.NewLazyDLL("kernel32.dll").NewProc("GetUserDefaultUILanguage").Call()
		text := "SAIO needs Microsoft Edge WebView2 Runtime to display its windows.\n\nIt is missing or needs an update. Install it once, then reopen SAIO. Your projects are preserved.\n\nOpen the SimLab download page?\n" + DownloadURL
		if language&0x3ff == 0x04 {
			text = "SAIO 需要 Microsoft Edge WebView2 运行库来显示界面。\n\n当前未安装或版本过旧。安装一次后，重新打开 SAIO 即可，已有工程会保留。\n\n是否打开 SimLab 下载页面？\n" + DownloadURL
		}
		return messageBox(text, 0x04|0x40) == 6
	}, openDownload)
	if err != nil {
		messageBox(err.Error(), 0x10)
	}
	return ok
}
