//go:build simlab_suite

package main

import (
	"github.com/Karl-Dai/Simulator-All-in-One/storage/go"
	"github.com/Karl-Dai/ModbusSim/go/internal/suitehost"
	"github.com/wailsapp/wails/v2/pkg/options"
)

func configureSuite(app *options.App) error {
	app.Bind = append(app.Bind, suitehost.NewAppearance(true, configstore.ReadJSON))
	return suitehost.Configure(app, "master")
}
