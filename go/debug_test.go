// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package debug_test

import (
	"testing"

	tabnas "github.com/rjrodger/tabnas/go"

	debug "github.com/rjrodger/tabnas-debug/go"
)

// TestLoads checks that the plugin value is present, mirroring the
// "loads" case in ../ts/test/debug.test.js.
func TestLoads(t *testing.T) {
	if debug.Debug == nil {
		t.Fatal("debug.Debug is nil")
	}
}

// TestDescribe checks that loading the plugin decorates the instance
// with a working Describe method, mirroring the "decorates an instance
// with describe()" case in the TypeScript tests.
func TestDescribe(t *testing.T) {
	am := tabnas.New()
	am.Use(debug.Debug, &debug.Options{Print: false, Trace: nil})

	if am.Debug == nil || am.Debug.Describe == nil {
		t.Fatal("describe() was not attached to the instance")
	}
	if _, ok := any(am.Debug.Describe()).(string); !ok {
		t.Fatal("describe() did not return a string")
	}
}

// TestDefaults checks that every trace kind is enabled by default, so
// the Go defaults stay in step with the canonical TypeScript DEFAULTS.
func TestDefaults(t *testing.T) {
	if !debug.Defaults.Print {
		t.Error("Defaults.Print should be true")
	}
	for _, kind := range []string{"step", "rule", "lex", "parse", "node", "stack"} {
		if !debug.Defaults.Trace[kind] {
			t.Errorf("Defaults.Trace[%q] should be true", kind)
		}
	}
}
