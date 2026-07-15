// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasdebug_test

import (
	"bytes"
	"os"
	"path/filepath"
	"strings"
	"testing"

	tabnas "github.com/tabnas/parser/go"

	tabnasdebug "github.com/tabnas/debug/go"
)

// headersGolden is the shared cross-runtime fixture of the eight canonical
// section headers; the TypeScript suite reads the same file. Keeping both
// suites pinned to it enforces the diffability claim.
const headersGolden = "../test/headers.golden"

// buildTreeGrammar installs a small non-trivial grammar on a fresh
// instance: a `top` rule that open-pushes to a single-character rule name
// `x` (carrying a group tag), with `x` matching a second token. It mirrors
// the makeTreeGrammar helper in ../ts/test/debug.test.js so the two suites
// assert the same describe() bodies and trace content.
func buildTreeGrammar(t *testing.T) *tabnas.Tabnas {
	t.Helper()
	j := tabnas.Make()
	ta := j.Token("Ta", "a")
	tx := j.Token("Tx", "x")
	zz := j.Token("#ZZ", "")

	j.Rule("top", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{ta}}, P: "x", G: "topgrp"})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}})
	})
	j.Rule("x", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{tx}}})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}})
	})
	j.SetOptions(tabnas.Options{Rule: &tabnas.RuleOptions{Start: "top"}})
	return j
}

// TestLoads checks that the plugin loads onto a fresh instance without
// error, mirroring the "loads" case in ../ts/test/debug.test.js.
func TestLoads(t *testing.T) {
	j := tabnas.Make()
	if err := j.Use(tabnasdebug.Debug, map[string]any{"print": false, "trace": false}); err != nil {
		t.Fatalf("Use(Debug) returned error: %v", err)
	}
}

// TestUseAndDescribe checks that the plugin loads onto an instance and
// that Describe returns a populated grammar dump, mirroring the
// "decorates an instance with describe()" case in the TypeScript tests.
func TestUseAndDescribe(t *testing.T) {
	j := tabnas.Make()
	if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": false}); err != nil {
		t.Fatalf("Use returned error: %v", err)
	}

	out, err := tabnasdebug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}
	if out == "" {
		t.Fatal("Describe returned an empty string")
	}
	for _, header := range []string{
		"========= INSTANCE ========",
		"========= TOKENS ========",
		"========= RULES =========",
		"========= ALTS =========",
		"========= LEXER =========",
		"========= CONFIG ========",
		"========= PLUGIN =========",
		"========= ABNF =========",
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
	if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true}); err != nil {
		t.Fatalf("Use with trace returned error: %v", err)
	}
	if _, err := j.Parse("1"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}
}

// TestDefaults checks that printing and tracing are on by default,
// keeping the Go defaults in step with the canonical TypeScript DEFAULTS.
func TestDefaults(t *testing.T) {
	if trace, ok := tabnasdebug.Defaults["trace"].(bool); !ok || !trace {
		t.Error(`Defaults["trace"] should be true`)
	}
	if print, ok := tabnasdebug.Defaults["print"].(bool); !ok || !print {
		t.Error(`Defaults["print"] should be true`)
	}
}

// TestDescribeIncludesTagAndConfig checks that the INSTANCE section reports
// the instance tag and the CONFIG section reports the rule start, mirroring
// the canonical TypeScript describe() output.
func TestDescribeIncludesTagAndConfig(t *testing.T) {
	j := tabnas.Make(tabnas.Options{Tag: "demo"})

	out, err := tabnasdebug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}
	if !strings.Contains(out, "tag: demo") {
		t.Error("Describe INSTANCE section should report the instance tag")
	}
	if !strings.Contains(out, "  start: ") {
		t.Error("Describe CONFIG section should report the rule start")
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

	out, err := tabnasdebug.Describe(j)
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
	out, err := tabnasdebug.Describe(nil)
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

// TestTraceContentCaptured checks that, with a grammar loaded, enabling
// tracing and capturing output via opts["out"] yields lex and rule trace
// lines for the parse. This exercises the capturable output writer added
// to the Go Debug plugin, the Go counterpart to the TypeScript trace
// content test (which injects a console via get_console).
func TestTraceContentCaptured(t *testing.T) {
	var buf bytes.Buffer
	j := buildTreeGrammar(t)
	if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true, "out": &buf}); err != nil {
		t.Fatalf("Use with trace+out returned error: %v", err)
	}

	// `ax` drives top -> push x -> close, producing both event streams.
	if _, err := j.Parse("ax"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}

	out := buf.String()
	if out == "" {
		t.Fatal("trace produced no captured output")
	}
	if !strings.Contains(out, "========= TRACE ==========") {
		t.Errorf("captured trace missing the TRACE banner:\n%s", out)
	}
	// Every TS trace kind must have a Go counterpart stream.
	for _, kind := range []string{"  step ", "  stack", "  rule ", "  lex  ", "  parse", "  node "} {
		if !strings.Contains(out, kind) {
			t.Errorf("captured trace missing %q lines:\n%s", strings.TrimSpace(kind), out)
		}
	}
	// The rule stream should name the rules that ran, including the
	// pushed single-character rule x.
	if !strings.Contains(out, "top~") {
		t.Errorf("captured trace missing the top rule:\n%s", out)
	}
	if !strings.Contains(out, "x~") {
		t.Errorf("captured trace missing the pushed rule x:\n%s", out)
	}
}

// TestTraceHonoursPerKindFlags checks the granular trace kinds, mirroring
// the TypeScript "honours per-kind trace flags (rule on, lex off)" case:
// with only the rule kind enabled, rule lines appear and every other
// stream is suppressed.
func TestTraceHonoursPerKindFlags(t *testing.T) {
	var buf bytes.Buffer
	j := buildTreeGrammar(t)
	err := j.Use(tabnasdebug.Debug, map[string]any{
		"print": false,
		"out":   &buf,
		"trace": map[string]any{
			"rule":  true,
			"lex":   false,
			"parse": false,
			"node":  false,
			"stack": false,
			"step":  false,
		},
	})
	if err != nil {
		t.Fatalf("Use with per-kind trace returned error: %v", err)
	}
	if _, err := j.Parse("ax"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}

	out := buf.String()
	if !strings.Contains(out, "  rule ") {
		t.Errorf("rule trace lines should appear:\n%s", out)
	}
	for _, kind := range []string{"  lex  ", "  parse", "  node ", "  stack", "  step "} {
		if strings.Contains(out, kind) {
			t.Errorf("%q trace lines should be suppressed:\n%s", strings.TrimSpace(kind), out)
		}
	}
}

// TestTracePartialKindMapKeepsOthersOn checks that a partial per-kind map
// merges over the all-true defaults (a partial object cannot turn other
// kinds off implicitly), mirroring the engine-side deep-merge of
// Debug.defaults in TypeScript.
func TestTracePartialKindMapKeepsOthersOn(t *testing.T) {
	var buf bytes.Buffer
	j := buildTreeGrammar(t)
	err := j.Use(tabnasdebug.Debug, map[string]any{
		"print": false,
		"out":   &buf,
		// Only lex is mentioned (off); every other kind stays on.
		"trace": map[string]any{"lex": false},
	})
	if err != nil {
		t.Fatalf("Use with partial trace map returned error: %v", err)
	}
	if _, err := j.Parse("ax"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}

	out := buf.String()
	if strings.Contains(out, "  lex  ") {
		t.Errorf("lex trace lines should be suppressed:\n%s", out)
	}
	for _, kind := range []string{"  rule ", "  parse", "  node ", "  stack", "  step "} {
		if !strings.Contains(out, kind) {
			t.Errorf("%q trace lines should stay on with a partial map:\n%s",
				strings.TrimSpace(kind), out)
		}
	}
}

// TestTraceDefaultOutDoesNotCrash checks that enabling tracing without an
// out writer (so it defaults to os.Stdout) parses cleanly. Output goes to
// stdout; we only assert the no-error, no-panic path here.
func TestTraceDefaultOutDoesNotCrash(t *testing.T) {
	j := buildTreeGrammar(t)
	if err := j.Use(tabnasdebug.Debug, map[string]any{"trace": true}); err != nil {
		t.Fatalf("Use with trace returned error: %v", err)
	}
	if _, err := j.Parse("ax"); err != nil {
		t.Fatalf("Parse returned error: %v", err)
	}
}

// TestDescribeBodies checks the populated TOKENS / ALTS / RULES bodies of
// Describe for a non-trivial grammar, asserting parity with the
// expectations in ../ts/test/debug.test.js's "describe() bodies" suite:
//   - custom tokens (Ta, Tx) and their fixed sources appear in TOKENS,
//   - the ALTS section shows the token sequence and push/group actions,
//   - the RULES transition tree keeps the single-character push edge
//     op: x (the off-by-one regression).
func TestDescribeBodies(t *testing.T) {
	j := buildTreeGrammar(t)

	out, err := tabnasdebug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}

	// TOKENS: custom tokens and their fixed source text.
	tokens := section(out, "========= TOKENS ========", "========= RULES =========")
	if !strings.Contains(tokens, "Ta") {
		t.Errorf("TOKENS missing custom token Ta:\n%s", tokens)
	}
	if !strings.Contains(tokens, "Tx") {
		t.Errorf("TOKENS missing custom token Tx:\n%s", tokens)
	}
	if !strings.Contains(tokens, `"a"`) {
		t.Errorf("TOKENS missing fixed source \"a\":\n%s", tokens)
	}

	// RULES: the single-character push edge must survive.
	rules := section(out, "========= RULES =========", "========= ALTS =========")
	if !strings.Contains(rules, "op: x") {
		t.Errorf("RULES tree missing single-char push edge op: x:\n%s", rules)
	}

	// ALTS: token sequence and push/group actions.
	alts := section(out, "========= ALTS =========", "========= LEXER =========")
	for _, want := range []string{"top:", "OPEN:", "CLOSE:", "[Ta]", "p=x", "g=topgrp"} {
		if !strings.Contains(alts, want) {
			t.Errorf("ALTS missing %q:\n%s", want, alts)
		}
	}
}

// TestHeadersMatchGolden checks that Describe emits, in order, exactly the
// eight canonical section headers held in the shared golden fixture. The
// TypeScript suite asserts the same fixture, so this pins both runtimes to
// one diffable layout.
func TestHeadersMatchGolden(t *testing.T) {
	data, err := os.ReadFile(filepath.Clean(headersGolden))
	if err != nil {
		t.Fatalf("reading golden headers fixture: %v", err)
	}
	golden := make([]string, 0, 8)
	for _, line := range strings.Split(string(data), "\n") {
		if line != "" {
			golden = append(golden, line)
		}
	}
	if len(golden) != 8 {
		t.Fatalf("golden fixture should hold 8 headers, got %d", len(golden))
	}

	j := tabnas.Make()
	out, err := tabnasdebug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}

	cursor := -1
	for _, header := range golden {
		at := strings.Index(out[cursor+1:], header)
		if at < 0 {
			t.Fatalf("Describe output missing header %q", header)
		}
		at += cursor + 1
		if at <= cursor {
			t.Fatalf("header out of order: %q", header)
		}
		cursor = at
	}
}

// buildAddGrammar installs the hand-written add grammar used to assert the
// ABNF emitter's exact output, mirroring the worked example in the task:
// `val` pushes `add`; `add` matches #NR then optionally a #PL-replace back
// into `add`, with an epsilon close and the #ZZ end close. The `+` fixed
// token is registered via options so its literal is recoverable from the
// fixed-token table.
func buildAddGrammar(t *testing.T) *tabnas.Tabnas {
	t.Helper()
	plus := "+"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{Token: map[string]*string{"#PL": &plus}},
		Rule:  &tabnas.RuleOptions{Start: "val"},
	})
	zz := j.Token("#ZZ")
	nr := j.Token("#NR")
	pl := j.Token("#PL")

	j.Rule("val", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "add"})
	})
	j.Rule("add", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{nr}}})
		// #PL replace continuation, an epsilon close (makes it optional),
		// and the #ZZ end close (skipped by the emitter).
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{pl}}, R: "add"})
		rs.AddClose(&tabnas.AltSpec{})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{zz}}})
	})
	return j
}

// TestAbnfAddGrammar checks that Abnf emits the add grammar byte-for-byte
// as the canonical TypeScript tabnas.debug.abnf() does (verified against
// the live TS emitter): productions reference tokens by bare name, the
// optional continuation folds into `[ PL add ]`, and each used token is
// defined after a blank line with `=` aligned to the longest name.
func TestAbnfAddGrammar(t *testing.T) {
	j := buildAddGrammar(t)

	out, err := tabnasdebug.Abnf(j)
	if err != nil {
		t.Fatalf("Abnf returned error: %v", err)
	}

	want := "val = add\n" +
		"add = NR [ PL add ]\n" +
		"\n" +
		"NR = <number>\n" +
		"PL = \"+\""
	if out != want {
		t.Errorf("Abnf output mismatch\n--- got ---\n%s\n--- want ---\n%s", out, want)
	}
}

// TestDescribeIncludesAbnf checks that Describe appends the ABNF section
// (header + emitted grammar) as the last section, mirroring the TS
// describe() placement.
func TestDescribeIncludesAbnf(t *testing.T) {
	j := buildAddGrammar(t)

	out, err := tabnasdebug.Describe(j)
	if err != nil {
		t.Fatalf("Describe returned error: %v", err)
	}
	if !strings.Contains(out, "========= ABNF =========") {
		t.Error("Describe output missing ABNF header")
	}
	if !strings.Contains(out, "add = NR [ PL add ]") {
		t.Errorf("Describe ABNF section missing the emitted add rule:\n%s", out)
	}
	// ABNF must be the last section: nothing else follows its header.
	abnfAt := strings.Index(out, "========= ABNF =========")
	pluginAt := strings.Index(out, "========= PLUGIN =========")
	if abnfAt < pluginAt {
		t.Error("ABNF section should come after PLUGIN")
	}
}

// section returns the substring of out between the start header and the
// end header (exclusive of end). If end is empty, it returns the tail from
// start onward.
func section(out, start, end string) string {
	si := strings.Index(out, start)
	if si < 0 {
		return ""
	}
	if end == "" {
		return out[si:]
	}
	ei := strings.Index(out, end)
	if ei < 0 || ei < si {
		return out[si:]
	}
	return out[si:ei]
}

// buildAbnfOptGrammar reproduces, by hand, the rule structure the abnf
// forward-compiler emits for `add = NR [ PL add ]` / `PL = "+"`: the
// optional `[ … ]` and its group / chain-step are separate synthetic
// productions (`_gen…_opt…`, `_gen…_group`, `…$step1`). The debug ABNF
// emitter must fold these back into `NR [ PL add ]` rather than print the
// `_gen…` rules. (debug does not depend on @tabnas/abnf, so the shape is
// reconstructed here — mirrors the sibling-path round-trip test in
// ../ts/test/abnf.test.js.)
func buildAbnfOptGrammar(t *testing.T) *tabnas.Tabnas {
	t.Helper()
	plus := "+"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{Token: map[string]*string{"#T": &plus}},
		Rule:  &tabnas.RuleOptions{Start: "add"},
	})
	nr := j.Token("#NR")
	tt := j.Token("#T")

	j.Rule("add", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{nr}}, P: "_gen2_opt__gen1_group"})
		rs.AddClose(&tabnas.AltSpec{}) // epsilon
	})
	j.Rule("PL", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{tt}}})
	})
	j.Rule("_gen1_group", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "PL"})
		rs.AddClose(&tabnas.AltSpec{R: "_gen1_group$step1"})
	})
	j.Rule("_gen1_group$step1", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "add"})
		rs.AddClose(&tabnas.AltSpec{}) // epsilon
	})
	j.Rule("_gen2_opt__gen1_group", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{tt}}, P: "_gen1_group", B: 1})
		rs.AddOpen(&tabnas.AltSpec{}) // epsilon alt (makes the group optional)
		rs.AddClose(&tabnas.AltSpec{})
	})
	return j
}

// TestAbnfFoldsSyntheticOptional is the positive round-trip case: the abnf
// compiler's synthetic `_gen…` helpers fold back into `[ … ]`, so a grammar
// authored as `add = NR [ PL add ]` re-emits as that same ABNF — no `_gen`
// productions leak into the output. Parity with the TS emitAbnf() folding.
func TestAbnfFoldsSyntheticOptional(t *testing.T) {
	j := buildAbnfOptGrammar(t)

	out, err := tabnasdebug.Abnf(j)
	if err != nil {
		t.Fatalf("Abnf returned error: %v", err)
	}

	want := "add = NR [ PL add ]\n" +
		"PL = T\n" +
		"\n" +
		"NR = <number>\n" +
		"T  = \"+\""
	if out != want {
		t.Errorf("Abnf folding mismatch\n--- got ---\n%s\n--- want ---\n%s", out, want)
	}
	if strings.Contains(out, "_gen") {
		t.Errorf("Abnf leaked a synthetic _gen production:\n%s", out)
	}
}

// buildAbnfStarGrammar reproduces the shape the abnf compiler emits for a
// repetition (`rep = *PL`): a `_gen…_star_…` production with an empty
// (zero-or-more) open alternative. Unlike `[ … ]`, repetition uses a
// probe-optimised subgraph that does NOT reconstruct as `*(…)` reliably, so
// the emitter must KEEP it as a production rather than fold it.
func buildAbnfStarGrammar(t *testing.T) *tabnas.Tabnas {
	t.Helper()
	plus := "+"
	j := tabnas.Make(tabnas.Options{
		Fixed: &tabnas.FixedOptions{Token: map[string]*string{"#T": &plus}},
		Rule:  &tabnas.RuleOptions{Start: "rep"},
	})
	tt := j.Token("#T")

	j.Rule("rep", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "_gen1_star_T"})
		rs.AddClose(&tabnas.AltSpec{}) // epsilon
	})
	j.Rule("_gen1_star_T", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{tt}}})
		rs.AddOpen(&tabnas.AltSpec{}) // empty alt -> zero-or-more
		rs.AddClose(&tabnas.AltSpec{})
	})
	return j
}

// TestAbnfKeepsRepetitionProduction is the negative case: a `_gen…_star_…`
// synthetic is NOT folded. It stays a bareword reference in its parent and
// is emitted as its own production, whose empty open alternative is
// preserved as a trailing `/` (the zero-or-more marker). Parity with the TS
// isFoldable() exclusion of `_star`/`_plus`/`$alt`.
func TestAbnfKeepsRepetitionProduction(t *testing.T) {
	j := buildAbnfStarGrammar(t)

	out, err := tabnasdebug.Abnf(j)
	if err != nil {
		t.Fatalf("Abnf returned error: %v", err)
	}

	want := "rep = _gen1_star_T\n" +
		"_gen1_star_T = T /\n" +
		"\n" +
		"T = \"+\""
	if out != want {
		t.Errorf("Abnf repetition mismatch\n--- got ---\n%s\n--- want ---\n%s", out, want)
	}
	// The star rule must survive as a production (not inlined away).
	if !strings.Contains(out, "_gen1_star_T = T /") {
		t.Errorf("repetition production was not kept:\n%s", out)
	}
}
