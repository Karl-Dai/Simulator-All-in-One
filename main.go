package main

import (
	"embed"
	"encoding/json"
	"flag"
	"fmt"
	"os"
	"path/filepath"

	"github.com/Karl-Dai/SimLab-Desktop/internal/suite"
	"github.com/wailsapp/wails/v2"
	"github.com/wailsapp/wails/v2/pkg/menu"
	"github.com/wailsapp/wails/v2/pkg/options"
	"github.com/wailsapp/wails/v2/pkg/options/assetserver"
	"github.com/wailsapp/wails/v2/pkg/options/mac"
	"github.com/wailsapp/wails/v2/pkg/options/windows"
)

//go:embed all:frontend/dist
var assets embed.FS

func main() {
	rootFlag := flag.String("suite-root", "", "Suite directory override for development and validation")
	verify := flag.Bool("verify", false, "Verify the complete installed suite without opening a window")
	launch := flag.String("launch", "", "Launch one built-in role without opening the suite window")
	flag.Parse()
	executable, err := os.Executable()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	root := *rootFlag
	if root == "" {
		root = suite.LocateRoot(executable)
	}
	data, err := suite.DefaultDataDir()
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
	service := suite.New(root, data)
	if *verify {
		snapshot := service.Snapshot()
		json.NewEncoder(os.Stdout).Encode(snapshot)
		for _, role := range snapshot.Roles {
			if !role.Ready {
				os.Exit(1)
			}
		}
		return
	}
	if *launch != "" {
		result, err := service.Launch(*launch)
		if err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		json.NewEncoder(os.Stdout).Encode(result)
		return
	}
	app := &App{service: service}
	appMenu := menu.NewMenu()
	appMenu.Append(menu.AppMenu())
	appMenu.Append(menu.EditMenu())
	err = wails.Run(&options.App{
		Title: "SimLab", Width: 1160, Height: 900, MinWidth: 820, MinHeight: 650,
		BackgroundColour: options.NewRGB(239, 243, 246),
		AssetServer:      &assetserver.Options{Assets: assets},
		Bind:             []interface{}{app}, Menu: appMenu,
		Mac:     &mac.Options{TitleBar: &mac.TitleBar{TitlebarAppearsTransparent: true, FullSizeContent: false}, About: &mac.AboutInfo{Title: "SimLab", Message: "工业协议模拟器套件 · Karl-Dai"}},
		Windows: &windows.Options{WebviewUserDataPath: filepath.Join(data, "launcher-webview")},
	})
	if err != nil {
		fmt.Fprintln(os.Stderr, err)
		os.Exit(1)
	}
}
