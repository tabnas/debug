// Copyright (c) 2021-2026 Richard Rodger, MIT License

// Package debug is the Go implementation of the tabnas Debug plugin.
//
// It mirrors the canonical TypeScript implementation in ../ts: a Debug
// plugin that traces a parse, and a Describe function that dumps a
// parser instance's active grammar (tokens, token sets, rules, alternates,
// lexer matchers and plugins). The TypeScript version is authoritative.
//
// The Go tabnas engine exposes tracing through instance subscribers
// (Tabnas.Sub) rather than the TypeScript context-log hook, and its
// introspection is read through exported accessors (Config, RSM,
// TinName, TokenSet, Plugins). The output here therefore tracks the
// TypeScript behaviour as closely as the Go engine API allows; see
// ../docs/reference.md for the documented differences.
package tabnasdebug

import (
	"fmt"
	"io"
	"os"
	"regexp"
	"sort"
	"strconv"
	"strings"

	tabnas "github.com/tabnas/parser/go"
)

// Version is the module version, injected at release by `make publish-go`.
const Version = "0.2.0"

// Defaults are the option values used when the plugin is loaded without
// an explicit configuration. They mirror the canonical TypeScript
// DEFAULTS, where tracing is on by default.
var Defaults = map[string]any{
	"trace": true,
}

// internalError converts a recovered panic value into an "internal"-code
// *tabnas.TabnasError. It mirrors the engine's own no-panic guarantee
// (see vendor .../go/plugin.go internalError): every error-returning
// entry point in this package surfaces a panic as a returned error rather
// than crashing the caller. The engine's helper is unexported, so the
// equivalent value is constructed here from TabnasError's exported fields.
func internalError(api string, r any) error {
	return &tabnas.TabnasError{
		Code:   "internal",
		Detail: fmt.Sprintf("%s: %v", api, r),
		Row:    1,
		Col:    1,
	}
}

// traceEnabled reports whether tracing should be installed for the given
// options. It mirrors the canonical TypeScript handling of the `trace`
// option, which accepts true | false | object:
//
//   - opts nil, or no "trace" key: fall back to Defaults["trace"].
//   - an explicit false (bool or *bool): off.
//   - any other non-nil value (true, a per-kind flag map/object): on.
//
// The Go engine surfaces only two event streams (token and rule), so a
// per-kind object cannot select kinds the way TypeScript does; any
// non-false trace value simply enables both streams.
func traceEnabled(opts map[string]any) bool {
	v, ok := opts["trace"]
	if !ok {
		// Honour the package default when the option is absent.
		v, ok = Defaults["trace"]
		if !ok {
			return false
		}
	}
	switch t := v.(type) {
	case nil:
		return false
	case bool:
		return t
	case *bool:
		return t != nil && *t
	default:
		// A non-false, non-nil value (e.g. a per-kind flag map) means "on".
		return true
	}
}

// Debug is the tabnas plugin entry point. Load it with
//
//	j.Use(debug.Debug, map[string]any{"trace": true})
//
// and call debug.Describe(j) for a grammar dump. When tracing is enabled
// (see traceEnabled) the plugin installs lex and rule subscribers that
// log each parse event.
//
// Trace output goes to os.Stdout by default; pass an io.Writer under
// opts["out"] to capture it (e.g. for tests).
//
// Loading via j.Use already runs under the engine's no-panic guard, but
// Debug guards itself too so that calling it directly cannot panic the
// caller: any panic while wiring trace subscribers is returned as an
// "internal"-code error.
var Debug tabnas.Plugin = func(j *tabnas.Tabnas, opts map[string]any) (err error) {
	defer func() {
		if r := recover(); r != nil {
			err = internalError("Debug", r)
		}
	}()

	if traceEnabled(opts) {
		var out io.Writer = os.Stdout
		if w, ok := opts["out"].(io.Writer); ok && w != nil {
			out = w
		}
		addTrace(j, out)
	}
	return nil
}

// addTrace installs the lex and rule subscribers that emit trace lines to
// out. The Go engine surfaces two event streams (token and rule); the
// canonical TypeScript plugin's finer kinds (step, parse, node, stack)
// have no Go engine equivalent and are not emitted here.
func addTrace(j *tabnas.Tabnas, out io.Writer) {
	j.Sub(
		func(tkn *tabnas.Token, rule *tabnas.Rule, ctx *tabnas.Context) {
			fmt.Fprintf(out, "[lex]  %-6s tin=%d src=%q val=%v at %d:%d\n",
				tkn.Name, tkn.Tin, tkn.Src, tkn.Val, tkn.RI, tkn.CI)
		},
		func(rule *tabnas.Rule, ctx *tabnas.Context) {
			fmt.Fprintf(out, "[rule] %s~%d:%s d=%d node=%v\n",
				rule.Name, rule.I, rule.State, rule.D, ctx.F(rule.Node))
		},
	)
}

// Describe returns a human-readable description of a parser instance's
// active configuration, mirroring the sections of the canonical
// TypeScript describe(): tokens, token sets, rules, alternates, lexer
// matchers and plugins.
//
// Unlike the TypeScript describe(), which returns a bare string, the Go
// form returns (string, error): it upholds the engine's no-panic
// guarantee. Malformed grammar specs (a nil config, a nil rule spec, a
// nil alternate) are rendered defensively rather than dereferenced, and
// any remaining panic is recovered and returned as an "internal"-code
// error with an empty string. On success the error is nil. This
// divergence is intentional; see ../docs/reference.md.
func Describe(j *tabnas.Tabnas) (out string, err error) {
	defer func() {
		if r := recover(); r != nil {
			out, err = "", internalError("Describe", r)
		}
	}()

	cfg := j.Config()

	return strings.Join([]string{
		"========= INSTANCE ========",
		describeInstance(j),
		"",
		"========= TOKENS ========",
		describeTokens(j, cfg),
		"",
		describeTokenSets(j),
		"",
		"========= RULES =========",
		describeRules(j),
		"",
		"========= ALTS =========",
		describeAlts(j),
		"",
		"========= LEXER =========",
		describeLexer(cfg),
		"",
		"========= CONFIG ========",
		describeConfig(cfg),
		"",
		"========= PLUGIN =========",
		describePlugins(j),
		"",
		"========= ABNF =========",
		emitAbnf(j),
		"",
	}, "\n"), nil
}

// Abnf returns an ABNF representation of the instance's live grammar,
// mirroring the canonical TypeScript tabnas.debug.abnf(). Unlike the
// TypeScript form, which returns a bare string, the Go form returns
// (string, error) to uphold the engine's no-panic guarantee: a malformed
// grammar spec is rendered defensively and any remaining panic is
// recovered and returned as an "internal"-code *tabnas.TabnasError with an
// empty string. On success the error is nil.
func Abnf(j *tabnas.Tabnas) (out string, err error) {
	defer func() {
		if r := recover(); r != nil {
			out, err = "", internalError("Abnf", r)
		}
	}()
	return emitAbnf(j), nil
}

// describeInstance reports the instance tag (empty when unset), mirroring
// the canonical TypeScript describe()'s INSTANCE section.
func describeInstance(j *tabnas.Tabnas) string {
	return "  tag: " + j.Options().Tag
}

// describeTokens lists every named token with its tin and, for fixed
// tokens, the source text it matches. It iterates (tin, name) pairs
// directly: the built-in tins in their canonical order, then any custom
// tins registered via j.Token (cfg.TinNames). There is no reverse scan
// and no one-to-one TinNames assumption.
func describeTokens(j *tabnas.Tabnas, cfg *tabnas.LexConfig) string {
	if cfg == nil {
		return ""
	}

	// Invert FixedTokens (source -> tin) to tin -> source, once.
	fixedSrc := make(map[tabnas.Tin]string, len(cfg.FixedTokens))
	for src, tin := range cfg.FixedTokens {
		fixedSrc[tin] = src
	}

	render := func(tin tabnas.Tin, name string) string {
		fixed := ""
		if src, ok := fixedSrc[tin]; ok && src != "" {
			fixed = `"` + src + `"`
		}
		return fmt.Sprintf("  %s\t%d\t%s", name, tin, fixed)
	}

	lines := make([]string, 0, int(tabnas.TinMAX))
	// Built-in tokens, in canonical tin order (TinBD..TinCA).
	for tin := tabnas.TinBD; tin < tabnas.TinMAX; tin++ {
		lines = append(lines, render(tin, j.TinName(tin)))
	}

	// Custom tokens registered via j.Token, keyed by tin in cfg.TinNames.
	// Iterate the (tin, name) pairs directly, in tin order for determinism.
	custom := make([]tabnas.Tin, 0, len(cfg.TinNames))
	for tin := range cfg.TinNames {
		if tin >= tabnas.TinMAX {
			custom = append(custom, tin)
		}
	}
	sort.Ints(custom)
	for _, tin := range custom {
		lines = append(lines, render(tin, cfg.TinNames[tin]))
	}

	return strings.Join(lines, "\n")
}

// describeTokenSets lists the named token sets (IGNORE, VAL, KEY, plus any
// custom set the engine exposes) and their member token names, mirroring
// the canonical TypeScript describe()'s tokenSet sub-block (debug.ts
// describe() lines ~88-97). Member tins are resolved to names and ordered
// deterministically (see ../docs/reference.md on ordering).
func describeTokenSets(j *tabnas.Tabnas) string {
	lines := make([]string, 0, 3)
	for _, name := range []string{"IGNORE", "VAL", "KEY"} {
		tins := j.TokenSet(name)
		if tins == nil {
			continue
		}
		// IGNORE is backed by a Go map (unordered); sort all sets by tin so
		// the output is deterministic regardless of engine storage.
		sorted := make([]tabnas.Tin, len(tins))
		copy(sorted, tins)
		sort.Ints(sorted)
		members := make([]string, 0, len(sorted))
		for _, tin := range sorted {
			members = append(members, j.TinName(tin))
		}
		lines = append(lines, "    "+name+"\t"+strings.Join(members, ","))
	}
	return strings.Join(lines, "\n")
}

// describeRules renders, for each rule, its open/close push/replace
// transition tree: the distinct rule-name targets reached by an open-push
// (op), open-replace (or), close-push (cp) and close-replace (cr)
// alternate. Empty categories are omitted; a single-character rule name is
// a valid target and is not dropped. Mirrors ruleTree()/ruleTreeStep() in
// debug.ts.
func describeRules(j *tabnas.Tabnas) string {
	rsm := j.RSM()
	names := sortedRuleNames(rsm)

	var b strings.Builder
	for _, name := range names {
		rs := rsm[name]
		b.WriteString("  " + name + ":\n    ")
		cats := []struct {
			label, data string
		}{
			{"op", ruleTreeStep(rs, "open", "p")},
			{"or", ruleTreeStep(rs, "open", "r")},
			{"cp", ruleTreeStep(rs, "close", "p")},
			{"cr", ruleTreeStep(rs, "close", "r")},
		}
		parts := make([]string, 0, len(cats))
		for _, c := range cats {
			// Drop only truly-empty categories; len > 0 keeps single-character
			// rule-name targets (matching the corrected TS off-by-one).
			if len(c.data) > 0 {
				parts = append(parts, c.label+": "+c.data)
			}
		}
		b.WriteString(strings.Join(parts, "\n    "))
		b.WriteString("\n")
	}
	return strings.TrimRight(b.String(), "\n")
}

// ruleTreeStep collects the distinct rule-name targets of one phase/step:
// the static (P / R) or dynamic (PF / RF) push/replace target of each
// alternate in the named phase. Function-valued targets render as "<F>".
// Mirrors ruleTreeStep() in debug.ts.
func ruleTreeStep(rs *tabnas.RuleSpec, phase, step string) string {
	if rs == nil {
		return ""
	}
	var alts []*tabnas.AltSpec
	if phase == "open" {
		alts = rs.OpenAlts()
	} else {
		alts = rs.CloseAlts()
	}

	seen := make(map[string]bool)
	var targets []string
	for _, a := range alts {
		if a == nil {
			continue
		}
		var name string
		switch step {
		case "p":
			if a.P != "" {
				name = a.P
			} else if a.PF != nil {
				name = "<F>"
			}
		case "r":
			if a.R != "" {
				name = a.R
			} else if a.RF != nil {
				name = "<F>"
			}
		}
		if name == "" || seen[name] {
			continue
		}
		seen[name] = true
		targets = append(targets, name)
	}
	return strings.Join(targets, " ")
}

// describeAlts renders every open and close alternate of every rule,
// showing its token sequence and actions.
func describeAlts(j *tabnas.Tabnas) string {
	rsm := j.RSM()
	names := sortedRuleNames(rsm)

	blocks := make([]string, 0, len(names))
	for _, name := range names {
		rs := rsm[name]
		block := "  " + name + ":\n"
		if rs != nil {
			block += descAltPhase(j, "OPEN", rs.OpenAlts()) +
				descAltPhase(j, "CLOSE", rs.CloseAlts())
		}
		blocks = append(blocks, strings.TrimRight(block, "\n"))
	}
	return strings.Join(blocks, "\n\n")
}

// descAltPhase renders the alternates of one phase (open or close).
func descAltPhase(j *tabnas.Tabnas, phase string, alts []*tabnas.AltSpec) string {
	if len(alts) == 0 {
		return ""
	}
	var b strings.Builder
	b.WriteString("    " + phase + ":\n")
	for i, a := range alts {
		if a == nil {
			// Mirror the TypeScript describe(), which renders a missing
			// alternate entry as ***INVALID*** rather than dereferencing it.
			b.WriteString(fmt.Sprintf("      %5d %s\n", i, "***INVALID***"))
			continue
		}
		b.WriteString(fmt.Sprintf("      %5d %s%s\n",
			i, padRight(altSeq(j, a.S), 24), altActions(a)))
	}
	return b.String()
}

// altSeq renders an alternate's token-sequence matcher. Each position may
// accept several tins: a single tin renders bare, a multi-token set
// renders as "[" + comma-join + "]". Mirrors descAlt/descAltSeq in
// debug.ts.
func altSeq(j *tabnas.Tabnas, seq [][]tabnas.Tin) string {
	positions := make([]string, 0, len(seq))
	for _, posTins := range seq {
		names := make([]string, 0, len(posTins))
		for _, tin := range posTins {
			names = append(names, j.TinName(tin))
		}
		switch len(names) {
		case 0:
			// Wildcard position (no tin constraint).
			positions = append(positions, "")
		case 1:
			positions = append(positions, names[0])
		default:
			positions = append(positions, "["+strings.Join(names, ",")+"]")
		}
	}
	return "[" + strings.Join(positions, " ") + "]"
}

// altActions renders the action/condition fields of an alternate:
//   - push (p) and replace (r), including function-valued targets ("<F>"),
//   - backtrack (b), counters (n), group (g),
//   - the action / condition / modifier presence flags (A / C / H),
//   - the declarative condition (CD).
//
// Mirrors descAlt() in debug.ts. The TS condition counter map (a.c.n,
// rendered there as CN=) has no Go AltSpec field — the engine folds
// counter conditions into the C function rather than retaining a separate
// map — so CN is not emitted; see ../docs/reference.md.
func altActions(a *tabnas.AltSpec) string {
	var parts []string

	// Push: static P, else function-valued PF as "<F>".
	if a.P != "" {
		parts = append(parts, "p="+a.P)
	} else if a.PF != nil {
		parts = append(parts, "p=<F>")
	}
	// Replace: static R, else function-valued RF as "<F>".
	if a.R != "" {
		parts = append(parts, "r="+a.R)
	} else if a.RF != nil {
		parts = append(parts, "r=<F>")
	}

	if a.B != 0 {
		parts = append(parts, fmt.Sprintf("b=%d", a.B))
	}

	if len(a.N) > 0 {
		parts = append(parts, "n="+joinIntMap(a.N))
	}

	if a.G != "" {
		parts = append(parts, "g="+a.G)
	}

	// Presence flags: action (A), condition (C), modifier (H).
	flags := ""
	if a.A != nil {
		flags += "A"
	}
	if a.C != nil {
		flags += "C"
	}
	if a.H != nil {
		flags += "H"
	}
	if flags != "" {
		parts = append(parts, flags)
	}

	// Declarative condition (CD). Render its comparison entries.
	if len(a.CD) > 0 {
		parts = append(parts, "CD="+joinCondMap(a.CD))
	}

	return strings.Join(parts, " ")
}

// joinIntMap renders a string→int map as "k:v,k:v" with keys sorted.
func joinIntMap(m map[string]int) string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	pairs := make([]string, 0, len(keys))
	for _, k := range keys {
		pairs = append(pairs, fmt.Sprintf("%s:%d", k, m[k]))
	}
	return strings.Join(pairs, ",")
}

// joinCondMap renders a declarative condition map (AltSpec.CD) as
// "k:v,k:v" with keys sorted. A value may be a bare int (an $eq shorthand)
// or a tabnas.CondOp (operator + value); both are rendered.
func joinCondMap(m map[string]any) string {
	keys := make([]string, 0, len(m))
	for k := range m {
		keys = append(keys, k)
	}
	sort.Strings(keys)
	pairs := make([]string, 0, len(keys))
	for _, k := range keys {
		switch v := m[k].(type) {
		case tabnas.CondOp:
			pairs = append(pairs, fmt.Sprintf("%s:%s%d", k, v.Op, v.Val))
		case int:
			pairs = append(pairs, fmt.Sprintf("%s:%d", k, v))
		default:
			pairs = append(pairs, fmt.Sprintf("%s:%v", k, v))
		}
	}
	return strings.Join(pairs, ",")
}

// describeLexer lists the custom lexer matchers in priority order. The
// Go engine's public API exposes only custom matchers (the built-in
// matchers cannot be enumerated); the built-in lex enable flags are
// reported in the CONFIG section instead. TypeScript lists every matcher
// here — see ../docs/reference.md for this divergence.
func describeLexer(cfg *tabnas.LexConfig) string {
	if cfg == nil {
		return ""
	}
	lines := make([]string, 0, len(cfg.CustomMatchers))
	for _, m := range cfg.CustomMatchers {
		if m == nil {
			continue
		}
		lines = append(lines, fmt.Sprintf("  %s (priority=%d)", m.Name, m.Priority))
	}
	return strings.Join(lines, "\n")
}

// describeConfig reports the key parser settings — rule start, finish,
// safe-key, and the built-in lex enable flags — mirroring the canonical
// TypeScript describe()'s CONFIG section.
func describeConfig(cfg *tabnas.LexConfig) string {
	if cfg == nil {
		return ""
	}
	return strings.Join([]string{
		fmt.Sprintf("  start: %s", cfg.RuleStart),
		fmt.Sprintf("  finish: %v", cfg.FinishRule),
		fmt.Sprintf("  safeKey: %v", cfg.SafeKey),
		fmt.Sprintf("  lex.fixed: %v", cfg.FixedLex),
		fmt.Sprintf("  lex.space: %v", cfg.SpaceLex),
		fmt.Sprintf("  lex.line: %v", cfg.LineLex),
		fmt.Sprintf("  lex.text: %v", cfg.TextLex),
		fmt.Sprintf("  lex.number: %v", cfg.NumberLex),
		fmt.Sprintf("  lex.comment: %v", cfg.CommentLex),
		fmt.Sprintf("  lex.string: %v", cfg.StringLex),
		fmt.Sprintf("  lex.value: %v", cfg.ValueLex),
	}, "\n")
}

// describePlugins reports the loaded plugin count. The Go engine stores
// plugins as bare functions, so individual names are not recoverable.
func describePlugins(j *tabnas.Tabnas) string {
	return fmt.Sprintf("  plugins: %d", len(j.Plugins()))
}

// emitAbnf renders an ABNF representation of the instance's *live*
// grammar, mirroring the canonical TypeScript emitAbnf() in
// ../ts/src/debug.ts. It reads ONLY the running engine (config + rule
// specs); it never imports a bnf port (Go has none).
//
// tabnas rules become ABNF productions; OPEN alts become `/`-separated
// alternatives; the token sequence (.S) plus any push/replace target
// (.P/.R) becomes a space-separated element list; and each token resolves
// to an ABNF terminal via the fixed-literal / match-regex config. A
// close-alt continuation (.P/.R, ignoring the #ZZ end token) folds onto
// the last open alt; if an epsilon close alt also exists, the continuation
// is wrapped as `[ ... ]` (optional). The synthetic `__start__` wrapper
// and the `#ZZ` end token are skipped; the real start rule leads.
//
// Each used token is then defined as its own ABNF rule (a quoted literal,
// a char-range, or a prose-val `<...>` for built-in lexer tokens), emitted
// after the productions with `=` aligned to the longest token name.
func emitAbnf(j *tabnas.Tabnas) string {
	cfg := j.Config()
	rsm := j.RSM()
	if cfg == nil {
		return ""
	}

	// bnf wraps grammars in a synthetic '__start__' rule (open .P -> the
	// real start, close matches #ZZ); skip it and lead with the real
	// start. A hand-written grammar's RuleStart is itself a real rule, so
	// keep it and emit it first like any other production.
	synthWrapper := ""
	if cfg.RuleStart == "__start__" {
		synthWrapper = cfg.RuleStart
	}
	startRule := ""
	if synthWrapper == "" {
		startRule = cfg.RuleStart
	} else if wrapper := rsm[synthWrapper]; wrapper != nil {
		for _, alt := range wrapper.OpenAlts() {
			if alt == nil {
				continue
			}
			if alt.P != "" {
				startRule = alt.P
				break
			}
			if alt.R != "" {
				startRule = alt.R
				break
			}
		}
	}

	// The engine's reserved end-of-source token; never an ABNF element.
	endTin := tabnas.TinZZ

	// Invert FixedTokens (source -> tin) to tin -> source once, the same
	// inversion describeTokens uses; this is the Go equivalent of TS
	// cfg.fixed.ref[tin].
	fixedSrc := make(map[tabnas.Tin]string, len(cfg.FixedTokens))
	for src, tin := range cfg.FixedTokens {
		fixedSrc[tin] = src
	}

	// Order productions: real start first, then every other non-wrapper
	// rule in stable (sorted) order, de-duplicated. The TS form uses the
	// engine's insertion order; the Go RSM is a map, so a sorted order is
	// used for determinism (the shared add grammar emits identically
	// because its rule names already sort as start-first by construction).
	ruleNames := make([]string, 0, len(rsm))
	for rn := range rsm {
		if rn != synthWrapper {
			ruleNames = append(ruleNames, rn)
		}
	}
	sort.Strings(ruleNames)

	ordered := make([]string, 0, len(ruleNames))
	seen := make(map[string]bool)
	if startRule != "" && rsm[startRule] != nil {
		ordered = append(ordered, startRule)
		seen[startRule] = true
	}
	for _, rn := range ruleNames {
		if !seen[rn] {
			ordered = append(ordered, rn)
			seen[rn] = true
		}
	}

	// Every token renders as its bare name in rule bodies; `used` collects
	// each token's definition for the legend printed after the
	// productions. usedOrder preserves first-seen order so the legend is
	// deterministic and matches the TS Map iteration order.
	used := make(map[string]string)
	var usedOrder []string
	recordUsed := func(name, form string) {
		if _, ok := used[name]; !ok {
			used[name] = form
			usedOrder = append(usedOrder, name)
		}
	}

	var lines []string
	for _, rn := range ordered {
		rs := rsm[rn]
		var alts []string

		if rs != nil {
			for _, alt := range rs.OpenAlts() {
				alts = append(alts, emitAbnfAlt(j, cfg, fixedSrc, rsm, alt, endTin, recordUsed))
			}

			closeAlts := rs.CloseAlts()

			// An epsilon close alt (no .S/.P/.R, and not the #ZZ end) means
			// the close continuation is OPTIONAL — render it as `[ cont ]`.
			hasEpsilonClose := false
			for _, alt := range closeAlts {
				if alt == nil {
					continue
				}
				if isEndAlt(alt, endTin) {
					continue
				}
				if len(alt.S) == 0 && alt.P == "" && alt.R == "" {
					hasEpsilonClose = true
					break
				}
			}

			for _, alt := range closeAlts {
				if alt == nil || isEndAlt(alt, endTin) {
					continue
				}
				// Only fold in close continuations that actually reference a
				// rule; pure capture-actions (no .S/.P/.R) are noise.
				if alt.P != "" || alt.R != "" {
					cont := emitAbnfAlt(j, cfg, fixedSrc, rsm, alt, endTin, recordUsed)
					piece := cont
					if hasEpsilonClose {
						piece = "[ " + cont + " ]"
					}
					if len(alts) > 0 {
						alts[len(alts)-1] = strings.TrimSpace(alts[len(alts)-1] + " " + piece)
					} else {
						alts = append(alts, piece)
					}
				}
			}
		}

		body := ""
		if len(alts) > 0 {
			body = strings.Join(alts, " / ")
		}
		lines = append(lines, rn+" = "+body)
	}

	// Define each used token as its own ABNF rule, with `=` aligned for
	// readability (named terminals, like the ABNF core rules).
	if len(usedOrder) > 0 {
		pad := 0
		for _, n := range usedOrder {
			if len(n) > pad {
				pad = len(n)
			}
		}
		lines = append(lines, "")
		for _, name := range usedOrder {
			lines = append(lines, padRight(name, pad)+" = "+used[name])
		}
	}

	return strings.Join(lines, "\n")
}

// isEndAlt reports whether a close alt is the engine's #ZZ end-of-source
// close (its first matched position is the end token). Mirrors the TS
// closeFirstTin === endTin check.
func isEndAlt(alt *tabnas.AltSpec, endTin tabnas.Tin) bool {
	if len(alt.S) != 1 {
		return false
	}
	pos := alt.S[0]
	if len(pos) == 0 {
		return false
	}
	return pos[0] == endTin
}

// emitAbnfAlt renders a single alt as an ABNF element sequence: each token
// in .S, then any .P/.R rule reference, space-separated. An alt with no
// elements renders as the empty string (an empty alternative). Mirrors the
// TS emitAbnfAlt().
func emitAbnfAlt(
	j *tabnas.Tabnas,
	cfg *tabnas.LexConfig,
	fixedSrc map[tabnas.Tin]string,
	rsm map[string]*tabnas.RuleSpec,
	alt *tabnas.AltSpec,
	endTin tabnas.Tin,
	recordUsed func(name, form string),
) string {
	if alt == nil {
		return ""
	}
	var els []string

	// When `B` (backup/lookahead) is set together with a push/replace, the
	// .S tokens are a *predictive peek* — matched to choose this alt but
	// not consumed here; the pushed rule consumes them. Emitting them as
	// terminals would double-count the input, so skip them and let the
	// rule reference stand alone (FIRST-set guard / optional dispatch).
	peekOnly := alt.B != 0 && (alt.P != "" || alt.R != "")

	if !peekOnly {
		for _, pos := range alt.S {
			// A position is a set of acceptable tins. A single tin renders
			// bare; a multi-tin set renders as `( a / b )`. Drop the end
			// token from any position.
			inner := make([]string, 0, len(pos))
			for _, tin := range pos {
				if tin == endTin {
					continue
				}
				inner = append(inner, emitAbnfTerminal(j, cfg, fixedSrc, rsm, tin, recordUsed))
			}
			switch len(inner) {
			case 0:
				// Wildcard / end-only position: contributes nothing.
			case 1:
				els = append(els, inner[0])
			default:
				els = append(els, "( "+strings.Join(inner, " / ")+" )")
			}
		}
	}

	// A push/replace target is a forward reference to another production.
	if alt.P != "" {
		els = append(els, alt.P)
	} else if alt.R != "" {
		els = append(els, alt.R)
	}

	return strings.Join(els, " ")
}

// emitAbnfTerminal renders a token reference: every token appears by its
// bare NAME (e.g. #PL -> PL, #NR -> NR), and its definition is recorded via
// recordUsed for the legend. A token name that is actually a rule name is a
// nonterminal reference and is returned as-is. Mirrors the TS
// emitAbnfTerminal().
func emitAbnfTerminal(
	j *tabnas.Tabnas,
	cfg *tabnas.LexConfig,
	fixedSrc map[tabnas.Tin]string,
	rsm map[string]*tabnas.RuleSpec,
	tin tabnas.Tin,
	recordUsed func(name, form string),
) string {
	fullName := j.TinName(tin)

	if fullName != "" {
		if _, ok := rsm[fullName]; ok {
			return fullName
		}
	}

	name := fullName
	if name == "" {
		name = fmt.Sprintf("T%d", tin)
	}
	name = strings.TrimPrefix(name, "#")
	recordUsed(name, abnfTokenForm(cfg, fixedSrc, tin, fullName))
	return name
}

// abnfTokenForm returns the legend definition for a token — what it
// matches:
//   - fixed literal       -> %s"<lit>" (letters) / "<lit>" (punctuation)
//   - /^<lit>/i (letters) -> "<lit>"     (case-insensitive literal)
//   - char range          -> %xLO-HI
//   - built-in matcher     -> <number> / <string> / ...   (lexer-provided)
//
// Mirrors the TS abnfTokenForm().
func abnfTokenForm(cfg *tabnas.LexConfig, fixedSrc map[tabnas.Tin]string, tin tabnas.Tin, fullName string) string {
	if lit, ok := fixedSrc[tin]; ok {
		if hasAsciiLetter(lit) {
			return `%s"` + lit + `"`
		}
		return `"` + lit + `"`
	}

	if cfg.MatchTokens != nil {
		if re := cfg.MatchTokens[tin]; re != nil {
			return regexToAbnf(re)
		}
	}

	// Built-in lexer token: describe it (it is lexer-provided, so a grammar
	// using it does not round-trip through bnf).
	bare := strings.TrimPrefix(fullName, "#")
	if bare == "" {
		bare = fmt.Sprintf("%d", tin)
	}
	desc := map[string]string{
		"NR": "number",
		"ST": "string",
		"TX": "text",
		"VL": "value",
		"SP": "space",
		"LN": "line",
		"CM": "comment",
		"AA": "any",
		"UK": "unknown",
		"BD": "bad",
		"ZZ": "end-of-source",
	}
	if d, ok := desc[bare]; ok {
		return "<" + d + ">"
	}
	return "<built-in " + bare + ">"
}

// regexToAbnf translates the anchored regexp the engine installs for a
// match token back to ABNF, covering the shapes that map cleanly. Mirrors
// the TS regexToAbnf(), adapted to Go's regexp syntax: Go has no RegExp
// flags field, so case-insensitivity is read from a leading (?i) group
// rather than a `flags` property.
func regexToAbnf(re *regexp.Regexp) string {
	src := re.String()

	// Go encodes case-insensitivity as a leading (?i) flag group. Detect
	// and strip it so the literal shape below can match.
	caseInsensitive := false
	if strings.HasPrefix(src, "(?i)") {
		caseInsensitive = true
		src = src[len("(?i)"):]
	}

	// Drop a leading anchor.
	if strings.HasPrefix(src, "^") {
		src = src[1:]
	}

	// Single char-class range: [\x{XXXX}-\x{YYYY}] or [\uXXXX-\uYYYY] ->
	// %xLO-HI.
	if lo, hi, ok := matchCharRange(src); ok {
		return "%x" + lo + "-" + hi
	}

	// Case-insensitive literal: recover the literal by unescaping the
	// regex metacharacters, then verify the round-trip so a genuine regex
	// is never misread as a literal.
	if caseInsensitive {
		lit := unescapeRegexLiteral(src)
		reEncoded := "(?i)^" + escapeRegExpLike(lit)
		if reEncoded == re.String() && isAbnfQuotable(lit) {
			return `"` + lit + `"`
		}
	}

	// Anything else: keep it visible but mark it as non-round-tripping.
	return "; /" + re.String() + "/"
}

// charRangeRe matches a single char-class range in the two escape forms a
// regex source may use: [\uXXXX-\uYYYY] and Go's [\x{XXXX}-\x{YYYY}], plus
// the bare two-digit hex form [\xXX-\xYY].
var (
	charRangeU   = regexp.MustCompile(`^\[\\u([0-9A-Fa-f]{4})-\\u([0-9A-Fa-f]{4})\]$`)
	charRangeX4  = regexp.MustCompile(`^\[\\x\{([0-9A-Fa-f]+)\}-\\x\{([0-9A-Fa-f]+)\}\]$`)
	charRangeX2  = regexp.MustCompile(`^\[\\x([0-9A-Fa-f]{2})-\\x([0-9A-Fa-f]{2})\]$`)
	metaEscapeRe = regexp.MustCompile(`\\([\\^$.*+?()\[\]{}|/])`)
	abnfMetaRe   = regexp.MustCompile(`[\\^$.*+?()\[\]{}|]`)
)

// matchCharRange extracts the lo/hi bounds of a single char-class range and
// returns them as uppercase hex (leading zeros stripped, mirroring the TS
// parseInt(...).toString(16).toUpperCase()), or ok=false if src is not such
// a range.
func matchCharRange(src string) (lo, hi string, ok bool) {
	for _, re := range []*regexp.Regexp{charRangeU, charRangeX4, charRangeX2} {
		if m := re.FindStringSubmatch(src); m != nil {
			return hexNormalize(m[1]), hexNormalize(m[2]), true
		}
	}
	return "", "", false
}

// hexNormalize parses a hex string and re-renders it as uppercase hex with
// no leading zeros, e.g. "0030" -> "30". Mirrors the TS
// parseInt(h,16).toString(16).toUpperCase().
func hexNormalize(h string) string {
	n, err := strconv.ParseInt(h, 16, 64)
	if err != nil {
		return strings.ToUpper(h)
	}
	return strings.ToUpper(strconv.FormatInt(n, 16))
}

// unescapeRegexLiteral reverses the metacharacter escaping the engine
// applies when encoding a bare ABNF literal as a regex.
func unescapeRegexLiteral(src string) string {
	return metaEscapeRe.ReplaceAllString(src, "$1")
}

// escapeRegExpLike mirrors the engine's escapeRegExp, used only to validate
// that an unescaped candidate literal re-escapes to the observed source.
func escapeRegExpLike(s string) string {
	return abnfMetaRe.ReplaceAllString(s, `\$0`)
}

// isAbnfQuotable reports whether s fits in an ABNF char-val (quoted
// string): printable ASCII except the double quote.
func isAbnfQuotable(s string) bool {
	if len(s) == 0 {
		return false
	}
	for _, c := range []byte(s) {
		if c == 0x22 {
			return false
		}
		if c < 0x20 || c > 0x7e {
			return false
		}
	}
	return true
}

// hasAsciiLetter reports whether s contains an ASCII letter (A-Z or a-z),
// the test the TS form uses to choose %s"..." over "...".
func hasAsciiLetter(s string) bool {
	for _, c := range []byte(s) {
		if (c >= 'A' && c <= 'Z') || (c >= 'a' && c <= 'z') {
			return true
		}
	}
	return false
}

// sortedRuleNames returns the rule names of a spec map in stable order.
func sortedRuleNames(rsm map[string]*tabnas.RuleSpec) []string {
	names := make([]string, 0, len(rsm))
	for name := range rsm {
		names = append(names, name)
	}
	sort.Strings(names)
	return names
}

// padRight pads s with trailing spaces to at least width characters.
func padRight(s string, width int) string {
	if len(s) >= width {
		return s
	}
	return s + strings.Repeat(" ", width-len(s))
}
