// Copyright (c) 2021-2026 Richard Rodger, MIT License

// Package debug is the Go implementation of the tabnas Debug plugin.
//
// It mirrors the canonical TypeScript implementation in ../ts: a Debug
// plugin that traces a parse, and a Describe function that dumps a
// parser instance's active grammar (tokens, rules, alternates, lexer
// matchers and plugins). The TypeScript version is authoritative.
//
// The Go tabnas engine exposes tracing through instance subscribers
// (Tabnas.Sub) rather than the TypeScript context-log hook, and its
// introspection is read through exported accessors (Config, RSM,
// TinName, Plugins). The output here therefore tracks the TypeScript
// behaviour as closely as the Go engine API allows; see
// ../docs/reference.md for the documented differences.
package debug

import (
	"fmt"
	"sort"
	"strings"

	tabnas "github.com/tabnas/parser/go"
)

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

// Debug is the tabnas plugin entry point. Load it with
//
//	j.Use(debug.Debug, map[string]any{"trace": true})
//
// and call debug.Describe(j) for a grammar dump. When opts["trace"] is
// true the plugin installs lex and rule subscribers that log each parse
// event.
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

	if trace, ok := opts["trace"].(bool); ok && trace {
		addTrace(j)
	}
	return nil
}

// addTrace installs the lex and rule subscribers that emit trace lines.
// The Go engine surfaces two event streams (token and rule); the
// TypeScript plugin's finer kinds (parse, node, stack) are not exposed
// by the Go engine and are intentionally omitted here.
func addTrace(j *tabnas.Tabnas) {
	j.Sub(
		func(tkn *tabnas.Token, rule *tabnas.Rule, ctx *tabnas.Context) {
			fmt.Printf("[lex]  %-6s tin=%d src=%q val=%v at %d:%d\n",
				tkn.Name, tkn.Tin, tkn.Src, tkn.Val, tkn.RI, tkn.CI)
		},
		func(rule *tabnas.Rule, ctx *tabnas.Context) {
			fmt.Printf("[rule] %s~%d:%s d=%d node=%v\n",
				rule.Name, rule.I, rule.State, rule.D, ctx.F(rule.Node))
		},
	)
}

// Describe returns a human-readable description of a parser instance's
// active configuration, mirroring the sections of the canonical
// TypeScript describe(): tokens, rules, alternates, lexer matchers and
// plugins.
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
		"========= TOKENS ========",
		describeTokens(j, cfg),
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
		"========= PLUGIN =========",
		describePlugins(j),
		"",
	}, "\n"), nil
}

// describeTokens lists every named token with its tin and, for fixed
// tokens, the source text it matches.
func describeTokens(j *tabnas.Tabnas, cfg *tabnas.LexConfig) string {
	if cfg == nil {
		return ""
	}

	// Invert FixedTokens (source -> tin) to tin -> source.
	fixedSrc := make(map[tabnas.Tin]string, len(cfg.FixedTokens))
	for src, tin := range cfg.FixedTokens {
		fixedSrc[tin] = src
	}

	names := make([]string, 0, len(cfg.TinNames))
	for tin := range cfg.TinNames {
		names = append(names, cfg.TinNames[tin])
	}
	sort.Strings(names)

	lines := make([]string, 0, len(names))
	for _, name := range names {
		// Resolve the tin for this name to look up any fixed source.
		var tin tabnas.Tin
		for t, n := range cfg.TinNames {
			if n == name {
				tin = t
				break
			}
		}
		fixed := ""
		if src, ok := fixedSrc[tin]; ok && src != "" {
			fixed = `"` + src + `"`
		}
		lines = append(lines, fmt.Sprintf("  %s\t%d\t%s", name, tin, fixed))
	}
	return strings.Join(lines, "\n")
}

// describeRules lists each rule with the alternate counts for its open
// and close phases.
func describeRules(j *tabnas.Tabnas) string {
	rsm := j.RSM()
	names := sortedRuleNames(rsm)

	lines := make([]string, 0, len(names))
	for _, name := range names {
		rs := rsm[name]
		if rs == nil {
			lines = append(lines, fmt.Sprintf("  %s:\topen=0 close=0", name))
			continue
		}
		lines = append(lines, fmt.Sprintf("  %s:\topen=%d close=%d",
			name, len(rs.Open), len(rs.Close)))
	}
	return strings.Join(lines, "\n")
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
			block += descAltPhase(j, "OPEN", rs.Open) +
				descAltPhase(j, "CLOSE", rs.Close)
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

// altSeq renders an alternate's token-sequence matcher. Each position
// may accept several tins; alternatives within a position are joined
// with "|".
func altSeq(j *tabnas.Tabnas, seq [][]tabnas.Tin) string {
	positions := make([]string, 0, len(seq))
	for _, posTins := range seq {
		names := make([]string, 0, len(posTins))
		for _, tin := range posTins {
			names = append(names, j.TinName(tin))
		}
		positions = append(positions, strings.Join(names, "|"))
	}
	return "[" + strings.Join(positions, " ") + "]"
}

// altActions renders the push/replace/back/counter/group fields of an
// alternate.
func altActions(a *tabnas.AltSpec) string {
	var parts []string
	if a.P != "" {
		parts = append(parts, "p="+a.P)
	}
	if a.R != "" {
		parts = append(parts, "r="+a.R)
	}
	if a.B != 0 {
		parts = append(parts, fmt.Sprintf("b=%d", a.B))
	}
	if len(a.N) > 0 {
		keys := make([]string, 0, len(a.N))
		for k := range a.N {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		pairs := make([]string, 0, len(keys))
		for _, k := range keys {
			pairs = append(pairs, fmt.Sprintf("%s:%d", k, a.N[k]))
		}
		parts = append(parts, "n="+strings.Join(pairs, ","))
	}
	if a.G != "" {
		parts = append(parts, "g="+a.G)
	}
	return strings.Join(parts, " ")
}

// describeLexer lists the custom lexer matchers in priority order. The
// Go engine exposes only custom matchers; the built-in matchers are
// summarised by the per-kind flags in the config.
func describeLexer(cfg *tabnas.LexConfig) string {
	if cfg == nil {
		return ""
	}
	lines := []string{
		fmt.Sprintf("  builtin: fixed=%v space=%v line=%v text=%v number=%v comment=%v string=%v value=%v",
			cfg.FixedLex, cfg.SpaceLex, cfg.LineLex, cfg.TextLex,
			cfg.NumberLex, cfg.CommentLex, cfg.StringLex, cfg.ValueLex),
	}
	for _, m := range cfg.CustomMatchers {
		if m == nil {
			continue
		}
		lines = append(lines, fmt.Sprintf("  %s (priority=%d)", m.Name, m.Priority))
	}
	return strings.Join(lines, "\n")
}

// describePlugins reports the loaded plugin count. The Go engine stores
// plugins as bare functions, so individual names are not recoverable.
func describePlugins(j *tabnas.Tabnas) string {
	return fmt.Sprintf("  plugins: %d", len(j.Plugins()))
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
