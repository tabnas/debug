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

	out, err := debug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}
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

// TestDescribeNoPanicMalformedRules checks that Describe does not panic on
// malformed grammar specs that would previously dereference a nil pointer:
// a nil rule spec and a rule with a nil alternate. Both must render
// defensively and return without an error, upholding the engine's
// no-panic guarantee.
func TestDescribeNoPanicMalformedRules(t *testing.T) {
	j := tabnas.Make()

	rsm := j.RSM()
	// A nil rule spec: previously panicked on len(rs.Open).
	rsm["__nil_spec__"] = nil
	// A rule whose alternate slice contains a nil entry: previously
	// panicked on a.S in descAltPhase.
	nilAlt := &tabnas.RuleSpec{Name: "__nil_alt__"}
	nilAlt.AddOpen(nil)
	rsm["__nil_alt__"] = nilAlt

	out, err := debug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error on malformed rules: %v", err)
	}
	if out == "" {
		t.Fatal("Describe returned an empty string on malformed rules")
	}
	if !strings.Contains(out, "***INVALID***") {
		t.Error("Describe should render a nil alternate as ***INVALID***")
	}
}

// TestDescribeErrorIsInternal checks that when Describe cannot recover a
// rendered string it returns an "internal"-code *tabnas.TabnasError and an
// empty string, mirroring the engine's no-panic guarantee. A nil instance
// dereferences inside Describe and must surface as an error, not a crash.
func TestDescribeErrorIsInternal(t *testing.T) {
	out, err := debug.Describe(nil)
	if err == nil {
		t.Fatal("Describe(nil) should return an error, got nil")
	}
	if out != "" {
		t.Errorf("Describe(nil) should return an empty string on error, got %q", out)
	}
	te, ok := err.(*tabnas.TabnasError)
	if !ok {
		t.Fatalf("Describe(nil) error should be *tabnas.TabnasError, got %T", err)
	}
	if te.Code != "internal" {
		t.Errorf("Describe(nil) error code = %q, want internal", te.Code)
	}
}
