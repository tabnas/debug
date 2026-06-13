// Copyright (c) 2021-2026 Richard Rodger, MIT License

// Package debug is the Go implementation of the tabnas Debug plugin.
//
// It adds tracing helpers and a Describe method to a *tabnas.Tabnas
// instance, mirroring the canonical TypeScript implementation in ../ts.
// The TypeScript version is authoritative: behaviour here is kept at
// parity with it, and any divergence is a bug in this file.
package debug

import (
	"encoding/json"
	"fmt"
	"sort"
	"strings"

	tabnas "github.com/rjrodger/tabnas/go"
)

// Options configures the Debug plugin.
//
// Print controls whether each call to Tabnas.Use prints the current
// grammar description. Trace selects which kinds of parse events are
// logged; a nil or empty map disables tracing entirely.
type Options struct {
	// Print logs the grammar description after every Use call.
	Print bool

	// Trace enables per-kind parse tracing. Recognised keys are
	// "step", "rule", "lex", "parse", "node" and "stack". A value of
	// nil disables all tracing.
	Trace map[string]bool
}

// Defaults are the option values used when the plugin is loaded without
// an explicit configuration. The tabnas parser merges these with any
// caller-supplied Options before invoking Debug.
var Defaults = Options{
	Print: true,
	Trace: map[string]bool{
		"step":  true,
		"rule":  true,
		"lex":   true,
		"parse": true,
		"node":  true,
		"stack": true,
	},
}

// traceKinds is the canonical ordering of the trace event kinds, used
// when copying Defaults.Trace so that behaviour is deterministic.
var traceKinds = []string{"step", "rule", "lex", "parse", "node", "stack"}

// defaultTrace returns a fresh copy of Defaults.Trace.
func defaultTrace() map[string]bool {
	out := make(map[string]bool, len(traceKinds))
	for _, k := range traceKinds {
		out[k] = Defaults.Trace[k]
	}
	return out
}

// Debug is the tabnas plugin entry point. Register it with
// Tabnas.Use(debug.Debug, &debug.Options{...}) or rely on Defaults.
func Debug(t *tabnas.Tabnas, options *Options) {
	if options == nil {
		o := Defaults
		o.Trace = defaultTrace()
		options = &o
	}

	util := t.Util()

	t.Debug = &tabnas.DebugAPI{
		Describe: func() string {
			cfg := t.Internal().Config
			match := cfg.Lex.Match
			rules := t.Rule()

			sections := []string{
				"========= TOKENS ========",
				describeTokens(cfg),
				"\n",

				describeTokenSets(cfg),
				"\n",

				"========= RULES =========",
				ruleTree(t, util.Keys(rules), rules),
				"\n",

				"========= ALTS =========",
				describeAllAlts(t, util.Values(rules)),
				"\n",

				"========= LEXER =========",
				describeLexer(match),
				"\n",

				"========= PLUGIN =========",
				describePlugins(t),
				"\n",
			}

			return strings.Join(sections, "\n")
		},
	}

	// Wrap Use so that loading a plugin optionally prints the grammar.
	origUse := t.Use
	t.Use = func(args ...any) *tabnas.Tabnas {
		self := origUse(args...)
		if options.Print && len(args) > 0 {
			self.Internal().Config.Debug.GetConsole().Log(
				"USE:", tabnas.PluginName(args[0]), "\n\n", self.Debug.Describe())
		}
		return self
	}

	if hasTrace(options.Trace) {
		t.Options(tabnas.Opts{
			Parse: tabnas.ParseOpts{
				Prepare: tabnas.PrepareOpts{
					"debug": func(_ *tabnas.Tabnas, ctx *tabnas.Context, _ any) {
						console := ctx.Cfg.Debug.GetConsole()
						console.Log("\n========= TRACE ==========")
						if ctx.Log == nil {
							ctx.Log = func(kind string, rest ...any) {
								formatter, ok := logKind[kind]
								if !ok || !options.Trace[kind] {
									return
								}
								console.Log(strings.Join(formatter(rest...), "  "))
							}
						}
					},
				},
			},
		})
	}
}

// hasTrace reports whether any trace kind is enabled.
func hasTrace(trace map[string]bool) bool {
	for _, on := range trace {
		if on {
			return true
		}
	}
	return false
}

// describeTokens renders the TOKENS section: every named token together
// with its fixed source text, if any.
func describeTokens(cfg *tabnas.Config) string {
	var lines []string
	for name, tin := range cfg.T {
		s, ok := tin.(string)
		if !ok {
			continue
		}
		fixed := ""
		if ref, ok := cfg.Fixed.Ref[name]; ok && ref != "" {
			fixed = `"` + fmt.Sprint(ref) + `"`
		}
		lines = append(lines, "  "+name+"\t"+s+"\t"+fixed)
	}
	sort.Strings(lines)
	return strings.Join(lines, "\n")
}

// describeTokenSets renders the named token sets and their members.
func describeTokenSets(cfg *tabnas.Config) string {
	var lines []string
	for name := range cfg.TokenSet {
		members := cfg.TokenSetTins[name]
		keys := make([]string, 0, len(members))
		for k := range members {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		lines = append(lines, "    "+name+"\t"+strings.Join(keys, ","))
	}
	sort.Strings(lines)
	return strings.Join(lines, "\n")
}

// describeAllAlts renders the ALTS section for every rule spec.
func describeAllAlts(t *tabnas.Tabnas, rules []*tabnas.RuleSpec) string {
	var blocks []string
	for _, rs := range rules {
		blocks = append(blocks,
			"  "+rs.Name+":\n"+descAlt(t, rs, "open")+descAlt(t, rs, "close"))
	}
	return strings.Join(blocks, "\n\n")
}

// describeLexer renders the LEXER section: the ordered match rules.
func describeLexer(match []*tabnas.LexMatch) string {
	if len(match) == 0 {
		return "  "
	}
	lines := make([]string, 0, len(match))
	for _, m := range match {
		lines = append(lines,
			fmt.Sprintf("%d: %s (%s)", m.Order, m.Matcher, m.Make.Name()))
	}
	return "  " + strings.Join(lines, "\n  ")
}

// describePlugins renders the PLUGIN section: each loaded plugin and its
// options.
func describePlugins(t *tabnas.Tabnas) string {
	plugins := t.Internal().Plugins
	lines := make([]string, 0, len(plugins))
	for _, p := range plugins {
		line := p.Name()
		for _, e := range t.Util().Entries(p.Options()) {
			val, _ := json.Marshal(e.Value)
			line += "\n    " + e.Key + ": " + string(val)
		}
		lines = append(lines, line)
	}
	return "  " + strings.Join(lines, "\n  ")
}

// descAlt renders the open or close alternates of a single rule spec.
func descAlt(t *tabnas.Tabnas, rs *tabnas.RuleSpec, kind string) string {
	alts := rs.Def[kind]
	if len(alts) == 0 {
		return ""
	}

	var b strings.Builder
	b.WriteString("    " + strings.ToUpper(kind) + ":\n")

	for i, a := range alts {
		seq := make([]string, 0, len(a.S))
		for _, tin := range a.S {
			seq = append(seq, formatTin(t, tin))
		}
		head := "[" + strings.Join(seq, " ") + "] "

		row := "      " + leftPad(fmt.Sprintf("%d", i), 5) + " " + rightPad(head, 32)
		row += altRef("r", a.R) + altRef("p", a.P)
		if a.R == nil && a.P == nil {
			row += "\t"
		}
		row += "\t" + optStr("b=", a.B)
		row += "\t" + nMap("n=", t, a.N)
		row += "\t" + flag("A", a.A) + flag("C", a.C) + flag("H", a.H)
		row += "\t" + condDetail(t, a)
		if a.G != "" {
			row += "\tg=" + a.G
		}
		b.WriteString(row)
		if i < len(alts)-1 {
			b.WriteString("\n")
		}
	}
	b.WriteString("\n")
	return b.String()
}

// formatTin renders a token identification number (or nested set) using
// the human-readable token name table.
func formatTin(t *tabnas.Tabnas, tin any) string {
	switch v := tin.(type) {
	case nil:
		return "***INVALID***"
	case int:
		return t.Token[v]
	case []any:
		parts := make([]string, 0, len(v))
		for _, sub := range v {
			parts = append(parts, formatTin(t, sub))
		}
		return "[" + strings.Join(parts, ",") + "]"
	default:
		return fmt.Sprint(v)
	}
}

// altRef renders an alternate's rule (r) or push (p) reference.
func altRef(label string, ref any) string {
	if ref == nil {
		return ""
	}
	if s, ok := ref.(string); ok {
		return " " + label + "=" + s
	}
	return " " + label + "=<F>"
}

// optStr renders an optional scalar with a label, or empty if nil.
func optStr(label string, v any) string {
	if v == nil {
		return ""
	}
	return label + fmt.Sprint(v)
}

// nMap renders an alternate's counter map (n) as k:v pairs.
func nMap(label string, t *tabnas.Tabnas, n map[string]any) string {
	if n == nil {
		return ""
	}
	return label + joinEntries(t, n)
}

// condDetail renders the condition counters and depth of an alternate.
func condDetail(t *tabnas.Tabnas, a *tabnas.NormAltSpec) string {
	out := "\t"
	if a.C != nil && a.C.N != nil {
		out = " CN=" + joinEntries(t, a.C.N)
	}
	if a.C != nil && a.C.D != nil {
		out += " CD=" + fmt.Sprint(a.C.D)
	}
	return out
}

// flag renders a single-letter presence flag.
func flag(label string, v any) string {
	if v == nil {
		return ""
	}
	return label
}

// joinEntries renders a string-keyed map as semicolon-free k:v pairs in
// stable key order.
func joinEntries(t *tabnas.Tabnas, m map[string]any) string {
	entries := t.Util().Entries(m)
	parts := make([]string, 0, len(entries))
	for _, e := range entries {
		parts = append(parts, e.Key+":"+fmt.Sprint(e.Value))
	}
	return strings.Join(parts, ",")
}

// ruleTree renders the per-rule open/close push/rule transition tree.
func ruleTree(t *tabnas.Tabnas, names []string, rules map[string]*tabnas.RuleSpec) string {
	var b strings.Builder
	for _, n := range names {
		steps := []struct{ key, value string }{
			{"op", ruleTreeStep(rules, n, "open", "p")},
			{"or", ruleTreeStep(rules, n, "open", "r")},
			{"cp", ruleTreeStep(rules, n, "close", "p")},
			{"cr", ruleTreeStep(rules, n, "close", "r")},
		}
		var lines []string
		for _, s := range steps {
			if len(s.value) > 1 {
				lines = append(lines, s.key+": "+s.value)
			}
		}
		b.WriteString("  " + n + ":\n    " + strings.Join(lines, "\n    ") + "\n")
	}
	return b.String()
}

// ruleTreeStep collects the distinct rule/push targets for one rule in a
// given state and step.
func ruleTreeStep(rules map[string]*tabnas.RuleSpec, name, state, step string) string {
	seen := map[string]bool{}
	var order []string
	for _, alt := range rules[name].Def[state] {
		var ref any
		if step == "p" {
			ref = alt.P
		} else {
			ref = alt.R
		}
		if ref == nil {
			continue
		}
		token := "<F>"
		if s, ok := ref.(string); ok {
			token = s
		}
		if !seen[token] {
			seen[token] = true
			order = append(order, token)
		}
	}
	return strings.Join(order, " ")
}

// descTokenState renders the lookahead token window for a trace line.
func descTokenState(ctx *tabnas.Context) string {
	src0, tin0 := "", ""
	if ctx.NOTOKEN != ctx.T0 {
		src0 = ctx.F(ctx.T0.Src)
		tin0 = ctx.Util().Tokenize(ctx.T0.Tin, ctx.Cfg)
	}
	src1, tin1 := "", ""
	if ctx.NOTOKEN != ctx.T1 {
		src1 = " " + ctx.F(ctx.T1.Src)
		tin1 = " " + ctx.Util().Tokenize(ctx.T1.Tin, ctx.Cfg)
	}
	return "[" + src0 + src1 + "]~[" + tin0 + tin1 + "]"
}

// descParseState renders the leading source, token window and rule depth.
func descParseState(ctx *tabnas.Context, rule *tabnas.Rule, lex *tabnas.Lex) string {
	window := ctx.F(substr(ctx.Src(), lex.Pnt.SI, 16))
	return rightPad(window, 18) + " " +
		rightPad(descTokenState(ctx), 34) + " " +
		leftPad(fmt.Sprintf("%d", rule.D), 4)
}

// descRuleState renders the active rule's named, used and key counters.
func descRuleState(ctx *tabnas.Context, rule *tabnas.Rule) string {
	out := ""
	if n := ctx.Util().Entries(rule.N); len(n) > 0 {
		var parts []string
		for _, e := range n {
			if e.Value != nil && e.Value != false {
				parts = append(parts, e.Key+"="+fmt.Sprint(e.Value))
			}
		}
		if len(parts) > 0 {
			out += " N<" + strings.Join(parts, ";") + ">"
		}
	}
	if u := ctx.Util().Entries(rule.U); len(u) > 0 {
		var parts []string
		for _, e := range u {
			parts = append(parts, e.Key+"="+ctx.F(e.Value))
		}
		out += " U<" + strings.Join(parts, ";") + ">"
	}
	if k := ctx.Util().Entries(rule.K); len(k) > 0 {
		var parts []string
		for _, e := range k {
			parts = append(parts, e.Key+"="+ctx.F(e.Value))
		}
		out += " K<" + strings.Join(parts, ";") + ">"
	}
	return out
}

// descAltSeq renders an alternate's token sequence using token names.
func descAltSeq(alt *tabnas.NormAltSpec, util tabnas.Util, cfg *tabnas.Config) string {
	parts := make([]string, 0, len(alt.S))
	for _, tin := range alt.S {
		switch v := tin.(type) {
		case int:
			parts = append(parts, util.Tokenize(v, cfg))
		case []any:
			sub := make([]string, 0, len(v))
			for _, s := range v {
				if n, ok := s.(int); ok {
					sub = append(sub, util.Tokenize(n, cfg))
				}
			}
			parts = append(parts, "["+strings.Join(sub, ",")+"]")
		default:
			parts = append(parts, "")
		}
	}
	return "[" + strings.Join(parts, " ") + "] "
}

// logKind maps a trace event kind to a formatter producing the printable
// fields of a single trace line. It mirrors the LOGKIND table in the
// canonical TypeScript implementation.
var logKind = map[string]func(rest ...any) []string{
	"step": func(rest ...any) []string {
		out := make([]string, 0, len(rest))
		for _, r := range rest {
			out = append(out, fmt.Sprint(r))
		}
		return out
	},

	"stack": func(rest ...any) []string {
		ctx, rule, lex := triple(rest)
		return []string{
			tabnas.S.LogIndent + tabnas.S.Stack,
			descParseState(ctx, rule, lex),
			strings.Repeat(tabnas.S.Indent, rule.D) + "/" + ruleStack(ctx, rule, false),
			"~",
			"/" + ruleStack(ctx, rule, true),
		}
	},

	"rule": func(rest ...any) []string {
		ctx, rule, lex := triple(rest)
		head := leftPad(rule.Name+"~"+fmt.Sprint(rule.I)+tabnas.S.Colon+ruleStateName(rule), 16)
		links := rightPad(fmt.Sprintf("prev=%d parent=%d child=%d",
			rule.Prev.I, rule.Parent.I, rule.Child.I), 28)
		return []string{
			tabnas.S.LogIndent + tabnas.S.Rule + tabnas.S.Space,
			descParseState(ctx, rule, lex),
			strings.Repeat(tabnas.S.Indent, rule.D) + head,
			links,
			descRuleState(ctx, rule),
		}
	},

	"node": func(rest ...any) []string {
		ctx, rule, lex := triple(rest)
		next, _ := rest[3].(*tabnas.Rule)
		body := rightPad("why="+next.Why+tabnas.S.Space+"<"+ctx.F(rule.Node)+">", 46)
		return []string{
			tabnas.S.LogIndent + tabnas.S.Node + tabnas.S.Space,
			descParseState(ctx, rule, lex),
			strings.Repeat(tabnas.S.Indent, rule.D) + body,
			descRuleState(ctx, rule),
		}
	},

	"parse": func(rest ...any) []string {
		ctx, rule, lex := triple(rest)
		match, _ := rest[3].(bool)
		cond, _ := rest[4].(bool)
		altI, _ := rest[5].(int)
		alt, _ := rest[6].(*tabnas.NormAltSpec)
		out, _ := rest[7].(*tabnas.AltMatch)

		altLabel := "no-alt"
		if match {
			altLabel = "alt=" + fmt.Sprint(altI)
		}
		seq := ""
		if match && alt != nil {
			seq = descAltSeq(alt, ctx.Util(), ctx.Cfg)
		}
		gFlag := ""
		if match && out.G != "" {
			gFlag = "g:" + out.G + " "
		}
		prb := ""
		if match {
			prb = optLabel("p:", out.P) + optLabel("r:", out.R) + optLabel("b:", out.B)
		}
		condFlag := ""
		if alt != nil && alt.C != nil {
			condFlag = "c:" + fmt.Sprint(cond)
		}
		return []string{
			tabnas.S.LogIndent + tabnas.S.Parse,
			descParseState(ctx, rule, lex),
			strings.Repeat(tabnas.S.Indent, rule.D) + altLabel,
			seq,
			gFlag,
			prb,
			condFlag,
			matchMap("n:", match, out.N, ctx),
			matchMap("u:", match, out.U, ctx),
			matchMap("k:", match, out.K, ctx),
		}
	},

	"lex": func(rest ...any) []string {
		ctx, rule, lex := triple(rest)
		pnt, _ := rest[3].(*tabnas.Point)
		sI, _ := rest[4].(int)
		match, _ := rest[5].(*tabnas.LexMatcher)
		tkn, _ := rest[6].(*tabnas.Token)
		var alt *tabnas.NormAltSpec
		var altI, tI int
		if len(rest) > 7 {
			alt, _ = rest[7].(*tabnas.NormAltSpec)
		}
		if len(rest) > 8 {
			altI, _ = rest[8].(int)
		}
		if len(rest) > 9 {
			tI, _ = rest[9].(int)
		}

		matchName := ""
		if match != nil {
			matchName = match.Name()
		}
		onAlt := ""
		if alt != nil {
			onAlt = fmt.Sprintf("on:alt=%d;%s;t=%d;%s",
				altI, alt.G, tI, descAltSeq(alt, ctx.Util(), ctx.Cfg))
		}
		return []string{
			tabnas.S.LogIndent + tabnas.S.Lex + tabnas.S.Space + tabnas.S.Space,
			descParseState(ctx, rule, lex),
			strings.Repeat(tabnas.S.Indent, rule.D) + ctx.Util().Tokenize(tkn.Tin, ctx.Cfg),
			ctx.F(tkn.Src),
			fmt.Sprint(pnt.SI),
			fmt.Sprintf("%d:%d", pnt.RI, pnt.CI),
			matchName,
			onAlt,
			ctx.F(substr(lex.Src, sI, 16)),
		}
	},
}

// ruleStateName maps a rule state code to its upper-case display name.
func ruleStateName(rule *tabnas.Rule) string {
	if rule.State == "o" {
		return strings.ToUpper(tabnas.S.Open)
	}
	return strings.ToUpper(tabnas.S.Close)
}

// ruleStack renders the rule ancestry, either as name~index pairs or as
// formatted nodes when nodes is true.
func ruleStack(ctx *tabnas.Context, rule *tabnas.Rule, nodes bool) string {
	parts := make([]string, 0, rule.D)
	for _, r := range ctx.RS[:rule.D] {
		if nodes {
			parts = append(parts, ctx.F(r.Node))
		} else {
			parts = append(parts, r.Name+"~"+fmt.Sprint(r.I))
		}
	}
	return strings.Join(parts, "/")
}

// matchMap renders an out-counter map for the parse trace line.
func matchMap(label string, match bool, m map[string]any, ctx *tabnas.Context) string {
	if !match || m == nil {
		return ""
	}
	entries := ctx.Util().Entries(m)
	parts := make([]string, 0, len(entries))
	for _, e := range entries {
		parts = append(parts, e.Key+"="+fmt.Sprint(e.Value))
	}
	return label + strings.Join(parts, ";")
}

// optLabel renders a labelled optional value, or empty if nil.
func optLabel(label string, v any) string {
	if v == nil {
		return ""
	}
	return label + fmt.Sprint(v) + " "
}

// triple extracts the (ctx, rule, lex) arguments common to most trace
// formatters.
func triple(rest []any) (*tabnas.Context, *tabnas.Rule, *tabnas.Lex) {
	ctx, _ := rest[0].(*tabnas.Context)
	rule, _ := rest[1].(*tabnas.Rule)
	lex, _ := rest[2].(*tabnas.Lex)
	return ctx, rule, lex
}

// substr returns up to length runes of s starting at start, clamping to
// the string bounds (the Go analogue of String.prototype.substring).
func substr(s string, start, length int) string {
	if start < 0 {
		start = 0
	}
	if start > len(s) {
		return ""
	}
	end := start + length
	if end > len(s) {
		end = len(s)
	}
	return s[start:end]
}

// leftPad pads s with leading spaces to at least width characters.
func leftPad(s string, width int) string {
	if len(s) >= width {
		return s
	}
	return strings.Repeat(" ", width-len(s)) + s
}

// rightPad pads s with trailing spaces to at least width characters.
func rightPad(s string, width int) string {
	if len(s) >= width {
		return s
	}
	return s + strings.Repeat(" ", width-len(s))
}
