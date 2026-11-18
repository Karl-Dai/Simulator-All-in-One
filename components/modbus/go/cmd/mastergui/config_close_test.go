package main

import (
	"context"
	"testing"
)

func TestConfigurationCloseRequiresArmRequestAndAcknowledgement(t *testing.T) {
	var c configurationClose
	notifications, exits := 0, 0
	ctx := context.WithValue(context.Background(), struct{}{}, "close")
	notify := func() { notifications++ }
	quit := func(actual context.Context) {
		if actual != ctx {
			t.Fatal("lost native close context")
		}
		exits++
	}
	c.confirm(quit)
	if exits != 0 {
		t.Fatal("unsolicited acknowledgement closed the application")
	}
	if c.request(ctx, notify) {
		t.Fatal("disarmed guard blocked an unmounted application")
	}
	c.enable()
	if !c.request(ctx, notify) || !c.request(ctx, notify) {
		t.Fatal("close was allowed before a flush acknowledgement")
	}
	if notifications != 1 || exits != 0 {
		t.Fatalf("requests did not coalesce: notifications=%d exits=%d", notifications, exits)
	}
	c.confirm(quit)
	c.confirm(quit)
	if exits != 1 {
		t.Fatalf("acknowledgement exits=%d", exits)
	}
	if c.request(ctx, notify) {
		t.Fatal("acknowledged native close was blocked")
	}
}
