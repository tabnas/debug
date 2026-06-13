// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package debug_test

import (
	"strings"
	"testing"

	tabnas "github.com/tabnas/parser/go"

	debug "github.com/tabnas/debug/go"
)

// TestLoads checks that the plugin value is present, mirroring the
// "loads" case in ../ts/test/debug.test.js.
func TestLoads(t *testing.T) {
	if debug.Debug == nil {
		t.Fatal("debug.Debug is nil")
	}
}

// TestUseAndDescribe checks that the plugin loads onto an instance and
// that Describe returns a populated grammar dump, mirroring the
// "decorates an instance with describe()" case in the TypeScript tests.
func TestUseAndDescribe(t *testing.T) {
	j := tabnas.Make()
	if err := j.Use(debug.Debug, map[string]any{"trace": false}); err != nil {
		t.Fatalf("Use returned error: %v", err)
	}

	out := debug.Describe(j)
	if out == "" {
		t.Fatal("Describe returned an empty string")
	}
	for _, header := range []string{
		"========= TOKENS ========",
		"========= RULES =========",
		"========= ALTS =========",
		"========= LEXER =========",
		"========= PLUGIN =========",
	} {
		if !strings.Contains(out, header) {
			t.Errorf("Describe output missing section %q", header)
		}
	}
}

// TestTraceEnables checks that loading with trace enabled does not error
// and that a subsequent parse runs (trace output goes to stdout).
func TestTraceEnables(t *testing.T) {
	j := tabnas.Make()
	if err := j.Use(debug.Debug, map[string]any{"trace": true}); err != nil {
		t.Fatalf("Use with trace returned error: %v", err)
	}
	if _, err := j.Parse("1"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}
}

// TestDefaults checks that tracing is on by default, keeping the Go
// defaults in step with the canonical TypeScript DEFAULTS.
func TestDefaults(t *testing.T) {
	if trace, ok := debug.Defaults["trace"].(bool); !ok || !trace {
		t.Error(`Defaults["trace"] should be true`)
	}
}
