package suitehost

import suiteui "github.com/Karl-Dai/Simulator-All-in-One/ui"

// Appearance bindings come from the shared suite ui package; this alias layer
// keeps the Modbus suitehost import path stable for the GUI entry points.
type Appearance = suiteui.Appearance
type AppearanceV1 = suiteui.AppearanceV1

func NewAppearance(suiteMode bool, readers ...func(string, string, string) ([]byte, bool, error)) *Appearance {
	return suiteui.NewAppearance(suiteMode, readers...)
}
