package main

import "github.com/Karl-Dai/Simulator-All-in-One/internal/suite"

type App struct{ service *suite.Service }

func (a *App) Snapshot() suite.Snapshot                     { return a.service.Snapshot() }
func (a *App) Launch(id string) (suite.LaunchResult, error) { return a.service.Launch(id) }
func (a *App) SetFavorite(id string, enabled bool) (suite.Preferences, error) {
	return a.service.SetFavorite(id, enabled)
}
func (a *App) SetTheme(theme string) (suite.Preferences, error) { return a.service.SetTheme(theme) }
func (a *App) SetLanguage(language string) (suite.Preferences, error) {
	return a.service.SetLanguage(language)
}
