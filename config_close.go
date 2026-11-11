package main

import (
	"context"
	"sync"

	"github.com/wailsapp/wails/v2/pkg/runtime"
)

type configurationClose struct {
	mu        sync.Mutex
	armed     bool
	requested bool
	approved  bool
	ctx       context.Context
}

func (c *configurationClose) enable() { c.mu.Lock(); c.armed = true; c.mu.Unlock() }
func (c *configurationClose) request(ctx context.Context, notify func()) bool {
	c.mu.Lock()
	if !c.armed || c.approved {
		c.mu.Unlock()
		return false
	}
	if c.requested {
		c.mu.Unlock()
		return true
	}
	c.requested = true
	c.ctx = ctx
	c.mu.Unlock()
	notify()
	return true
}
func (c *configurationClose) confirm(quit func(context.Context)) {
	c.mu.Lock()
	if !c.requested || c.approved {
		c.mu.Unlock()
		return
	}
	c.approved = true
	ctx := c.ctx
	c.mu.Unlock()
	quit(ctx)
}
func (a *App) EnableConfigurationClose() { a.closeState.enable() }
func (a *App) beforeClose(ctx context.Context) bool {
	return a.closeState.request(ctx, func() { runtime.EventsEmit(ctx, "saio-before-close") })
}
func (a *App) ConfirmConfigurationClose() {

	a.closeState.confirm(runtime.Quit)
}
