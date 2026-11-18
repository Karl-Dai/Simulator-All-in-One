package main

import (
	"embed"
	"io/fs"
	"log"
	"runtime"

	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/menu"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
)

//go:embed all:assets
var assets embed.FS

func main() {
	frontend, err := fs.Sub(assets, "assets/dist")
	if err != nil {
		log.Fatal(err)
	}
	app := newApp()
	// Native application/edit roles are only supported on macOS.
	// Passing them to Windows produces an empty menu bar.
	var appMenu *menu.Menu
	if runtime.GOOS == "darwin" {
		appMenu = menu.NewMenu()
		appMenu.Append(menu.AppMenu())
		appMenu.Append(menu.EditMenu())
	}
	appOptions := &options.App{
		Title: "ModbusMaster — Go", Width: 1280, Height: 800,
		MinWidth: 900, MinHeight: 600, Menu: appMenu,
		AssetServer: &assetserver.Options{Assets: frontend},
		OnStartup:   app.startup, OnShutdown: app.shutdown, OnBeforeClose: app.beforeClose,
		Bind: []interface{}{app},
	}
	if err := configureSuite(appOptions); err != nil {
		log.Fatal(err)
	}
	if err := wails.Run(appOptions); err != nil {
		log.Fatal(err)
	}
}
