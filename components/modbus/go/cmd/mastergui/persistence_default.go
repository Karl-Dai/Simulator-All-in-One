//go:build !simlab_suite

package main

const projectFileExtension = ".modbusproj"

func (a *App) initializePersistence()               {}
func (a *App) readProject(path string) (int, error) { return a.manager.LoadProject(path) }
func (a *App) writeProject(path string) error       { return a.manager.SaveProject(path) }
