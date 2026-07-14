// Copyright (c) 2021-2026 Richard Rodger, MIT License

// trace.go implements the granular parse-trace streams of the Debug
// plugin, mirroring the canonical TypeScript trace kinds (step, rule,
// lex, parse, node, stack) in ../ts/src/debug.ts.
//
// The TypeScript engine drives tracing through a ctx.log callback that
// the engine itself invokes with a kind tag at each parse event. The Go
// engine has no such callback; it exposes:
//
//   - Tabnas.Sub lex subscribers (one event per lexed token),
//   - Tabnas.Sub rule subscribers (one event per rule step, fired
//     before the step runs — the same point the TS engine logs `step`,
//     `stack` and `rule`),
//   - options.parse.prepare hooks (run once at the start of each parse),
//   - RuleSpec after-open / after-close state actions (run at the end of
//     a rule step, after alternate matching and push/replace resolution —
//     the closest Go analogue of the points where the TS engine logs
//     `parse` and `node`).
//
// Each TS trace kind therefore has a Go counterpart with a closely
// matching line shape; the differences that remain (documented in
// ../docs/reference.md) are the alt index on `parse` lines (the engine
// does not expose which alternate matched) and the lex matcher name.
package tabnasdebug

import (
	"fmt"
	"io"
	"sort"
	"strconv"
	"strings"

	tabnas "github.com/tabnas/parser/go"
)

// traceKindNames lists the recognised trace kinds, matching the keys of
// the canonical TypeScript DebugOptions.trace object.
var traceKindNames = []string{"step", "rule", "lex", "parse", "node", "stack"}

// traceState carries the live trace configuration for one instance. It is
// stored as an instance decoration so repeated plugin application (or
// child derivation, which re-runs parent plugins) updates the existing
// state instead of stacking duplicate subscribers — the Go analogue of
// the TypeScript __debugUseWrapped guard.
type traceState struct {
	out    io.Writer
	kinds  map[string]bool
	hooked map[*tabnas.RuleSpec]bool
}

// resolveTrace interprets the `trace` option, mirroring the canonical
// TypeScript handling of true | false | per-kind object:
//
//   - opts nil, or no "trace" key: fall back to Defaults["trace"].
//   - an explicit false (bool or *bool): off.
//   - true: on, every kind enabled.
//   - a per-kind flag map (map[string]any or map[string]bool): on; the
//     map is merged over the all-true defaults, so a partial map cannot
//     turn other kinds off implicitly (set them false explicitly) —
//     matching the engine-side deep-merge of Debug.defaults in TS.
//   - any other non-nil value: on, every kind enabled.
func resolveTrace(opts map[string]any) (bool, map[string]bool) {
	v, ok := opts["trace"]
	if !ok {
		v, ok = Defaults["trace"]
		if !ok {
			return false, nil
		}
	}

	allKinds := func(on bool) map[string]bool {
		kinds := make(map[string]bool, len(traceKindNames))
		for _, k := range traceKindNames {
			kinds[k] = on
		}
		return kinds
	}

	switch t := v.(type) {
	case nil:
		return false, nil
	case bool:
		return t, allKinds(t)
	case *bool:
		on := t != nil && *t
		return on, allKinds(on)
	case map[string]any:
		kinds := allKinds(true)
		for k, kv := range t {
			switch b := kv.(type) {
			case bool:
				kinds[k] = b
			case *bool:
				kinds[k] = b != nil && *b
			}
		}
		return true, kinds
	case map[string]bool:
		kinds := allKinds(true)
		for k, b := range t {
			kinds[k] = b
		}
		return true, kinds
	default:
		// A non-false, non-nil value means "on, all kinds".
		return true, allKinds(true)
	}
}

// installTrace wires the trace streams onto an instance: the lex and rule
// subscribers, and a parse.prepare hook (registered through the options
// tree like the TS plugin, so later SetOptions calls preserve it) that
// prints the TRACE banner and installs the per-rule-spec parse/node
// hooks. Repeated application updates the existing state in place.
func installTrace(j *tabnas.Tabnas, out io.Writer, kinds map[string]bool) {
	if st, ok := j.Decoration(traceDecoration).(*traceState); ok && st != nil {
		st.out = out
		st.kinds = kinds
		return
	}

	st := &traceState{
		out:    out,
		kinds:  kinds,
		hooked: make(map[*tabnas.RuleSpec]bool),
	}
	j.Decorate(traceDecoration, st)
	j.Sub(st.lexSub, st.ruleSub)
	j.SetOptions(tabnas.Options{
		Parse: &tabnas.ParseOptions{
			Prepare: map[string]func(ctx *tabnas.Context){
				// The "debug" key matches the TS plugin's prepare entry so a
				// re-application replaces rather than stacks the hook.
				"debug": st.prepare,
			},
		},
	})
}

// prepare runs once at the start of every parse: it prints the TRACE
// banner, installs the parse/node hooks on any rule spec not yet hooked
// (covering rules added after the plugin loaded), and — mirroring the TS
// `ctx.log = ctx.log || ...` — sets ctx.Log when unset so grammar code
// can emit its own kind-gated trace lines.
func (st *traceState) prepare(ctx *tabnas.Context) {
	fmt.Fprintln(st.out, "\n========= TRACE ==========")
	if ctx.RSM != nil {
		st.installParseHooks(ctx.RSM)
	}
	if ctx.Log == nil {
		ctx.Log = st.ctxLog
	}
}

// ctxLog is the ctx.Log implementation installed by prepare. The first
// argument is the trace kind; the remaining arguments are printed joined
// by the log gap when that kind is enabled. Unknown kinds are dropped,
// mirroring the TS `LOGKIND[kind] && options.trace[kind]` gate.
func (st *traceState) ctxLog(args ...any) {
	if len(args) == 0 {
		return
	}
	kind, ok := args[0].(string)
	if !ok || !st.kinds[kind] {
		return
	}
	parts := make([]string, 0, len(args)-1)
	for _, a := range args[1:] {
		parts = append(parts, fmt.Sprint(a))
	}
	fmt.Fprintln(st.out, strings.Join(parts, "  "))
}

// installParseHooks adds the after-open / after-close state actions that
// emit the `parse` and `node` streams to every (non-nil) rule spec not
// already hooked.
func (st *traceState) installParseHooks(rsm map[string]*tabnas.RuleSpec) {
	for _, rs := range rsm {
		if rs == nil || st.hooked[rs] {
			continue
		}
		st.hooked[rs] = true
		rs.AddAO(func(r *tabnas.Rule, ctx *tabnas.Context) {
			st.afterStep(r, ctx, true)
		})
		rs.AddAC(func(r *tabnas.Rule, ctx *tabnas.Context) {
			st.afterStep(r, ctx, false)
		})
	}
}

// lexSub emits one `lex` line per lexed token, mirroring the TS lex
// stream: parse state, indent, token name, formatted source, source
// index, and row:col. The lex matcher name is not exposed by the Go
// engine's subscriber and is omitted.
func (st *traceState) lexSub(tkn *tabnas.Token, rule *tabnas.Rule, ctx *tabnas.Context) {
	if !st.kinds["lex"] || tkn == nil || rule == nil || ctx == nil {
		return
	}
	st.emit(
		"  lex  ",
		fmtParseState(ctx, rule),
		traceIndent(rule.D)+tkn.Name,
		fmtVal(ctx, tkn.Src),
		strconv.Itoa(tkn.SI),
		strconv.Itoa(tkn.RI)+":"+strconv.Itoa(tkn.CI),
	)
}

// ruleSub fires before each rule step and emits the `step`, `stack` and
// `rule` lines in that order — the same order the TS engine logs them
// (parser loop: step, stack; rule at the top of process()).
func (st *traceState) ruleSub(rule *tabnas.Rule, ctx *tabnas.Context) {
	if rule == nil || ctx == nil {
		return
	}

	if st.kinds["step"] {
		st.emit("  step ", strconv.Itoa(ctx.KI)+":")
	}

	if st.kinds["stack"] {
		depth := rule.D
		if ctx.RSI < depth {
			depth = ctx.RSI
		}
		names := make([]string, 0, depth)
		nodes := make([]string, 0, depth)
		for i := 0; i < depth && i < len(ctx.RS); i++ {
			r := ctx.RS[i]
			if r == nil {
				break
			}
			names = append(names, r.Name+"~"+strconv.Itoa(r.I))
			nodes = append(nodes, fmtVal(ctx, r.Node))
		}
		st.emit(
			"  stack",
			fmtParseState(ctx, rule),
			traceIndent(rule.D)+"/"+strings.Join(names, "/"),
			"~",
			"/"+strings.Join(nodes, "/"),
		)
	}

	if st.kinds["rule"] {
		state := "OPEN"
		if rule.State == tabnas.CLOSE {
			state = "CLOSE"
		}
		st.emit(
			"  rule ",
			fmtParseState(ctx, rule),
			traceIndent(rule.D)+padRight(rule.Name+"~"+strconv.Itoa(rule.I)+":"+state, 16),
			padRight("prev="+ruleId(rule.Prev)+" parent="+ruleId(rule.Parent)+
				" child="+ruleId(rule.Child), 28),
			fmtRuleState(ctx, rule),
		)
	}
}

// afterStep runs at the end of a rule step (after-open / after-close) and
// emits the `parse` and `node` lines. It is the Go counterpart of the TS
// engine's parse/node log points: alternate matching and push/replace
// resolution have completed, so the matched token sequence (r.O / r.C)
// and the resulting push (`p:`) or replace (`r:`) target are known. The
// engine does not expose which alternate index matched, so lines carry
// `alt` / `no-alt` without the TS `alt=N` index.
func (st *traceState) afterStep(r *tabnas.Rule, ctx *tabnas.Context, isOpen bool) {
	if r == nil || ctx == nil || r.Spec == nil {
		return
	}
	if !st.kinds["parse"] && !st.kinds["node"] {
		return
	}

	var alts []*tabnas.AltSpec
	var toks []*tabnas.Token
	if isOpen {
		alts = r.Spec.OpenAlts()
		if r.ON <= len(r.O) {
			toks = r.O[:r.ON]
		}
	} else {
		alts = r.Spec.CloseAlts()
		if r.CN <= len(r.C) {
			toks = r.C[:r.CN]
		}
	}

	// TS logs `parse` only when the phase had alternates to try.
	if st.kinds["parse"] && len(alts) > 0 {
		matched := ctx.ParseErr == nil
		altLabel := "no-alt"
		seq := ""
		effect := ""
		if matched {
			altLabel = "alt"
			names := make([]string, 0, len(toks))
			for _, tk := range toks {
				if tk != nil {
					names = append(names, tk.Name)
				}
			}
			seq = "[" + strings.Join(names, " ") + "] "
			if next := r.Next; next != nil && next != tabnas.NoRule {
				if next.Parent == r {
					effect = "p:" + next.Name + " "
				} else if next.Prev == r {
					effect = "r:" + next.Name + " "
				}
			}
		}
		st.emit(
			"  parse",
			fmtParseState(ctx, r),
			traceIndent(r.D)+altLabel,
			seq,
			effect,
			fmtRuleState(ctx, r),
		)
	}

	if st.kinds["node"] {
		// TS sets next.why to 'O'/'C' (or an alt-specific code) before its
		// node log; the Go engine records failure codes on rule.Why only.
		why := "O"
		if !isOpen {
			why = "C"
		}
		if r.Why != "" {
			why = r.Why
		}
		st.emit(
			"  node ",
			fmtParseState(ctx, r),
			traceIndent(r.D)+padRight("why="+why+" <"+fmtVal(ctx, r.Node)+">", 46),
			fmtRuleState(ctx, r),
		)
	}
}

// emit writes one trace line: the parts joined by the two-space log gap,
// mirroring the TS trace logger's `.join('  ')`.
func (st *traceState) emit(parts ...string) {
	fmt.Fprintln(st.out, strings.Join(parts, "  "))
}

// fmtParseState renders the shared line prefix (TS descParseState): the
// upcoming source fragment, the token window, and the parse depth.
func fmtParseState(ctx *tabnas.Context, r *tabnas.Rule) string {
	sI := 0
	if ctx.Lex != nil {
		sI = ctx.Lex.Cursor().SI
	}
	if sI < 0 {
		sI = 0
	}
	if sI > len(ctx.Src) {
		sI = len(ctx.Src)
	}
	end := sI + 16
	if end > len(ctx.Src) {
		end = len(ctx.Src)
	}
	return padRight(fmtVal(ctx, ctx.Src[sI:end]), 18) + " " +
		padRight(fmtTokenState(ctx), 34) + " " +
		padLeft(strconv.Itoa(r.D), 4)
}

// fmtTokenState renders the two-token lookahead window (TS
// descTokenState): `[src0 src1]~[name0 name1]`.
func fmtTokenState(ctx *tabnas.Context) string {
	srcs := ""
	names := ""
	if t := ctx.T0; t != nil && !t.IsNoToken() {
		srcs += fmtVal(ctx, t.Src)
		names += t.Name
	}
	if t := ctx.T1; t != nil && !t.IsNoToken() {
		srcs += " " + fmtVal(ctx, t.Src)
		names += " " + t.Name
	}
	return "[" + srcs + "]~[" + names + "]"
}

// fmtRuleState renders a rule's counters and custom props (TS
// descRuleState): ` N<k=v;…> U<k=v;…> K<k=v;…>`, each block present only
// when non-empty; N entries with zero values are dropped (TS filters
// falsy values). Keys are sorted for deterministic output.
func fmtRuleState(ctx *tabnas.Context, r *tabnas.Rule) string {
	out := ""

	if len(r.N) > 0 {
		keys := make([]string, 0, len(r.N))
		for k, v := range r.N {
			if v != 0 {
				keys = append(keys, k)
			}
		}
		sort.Strings(keys)
		if len(keys) > 0 {
			pairs := make([]string, 0, len(keys))
			for _, k := range keys {
				pairs = append(pairs, k+"="+strconv.Itoa(r.N[k]))
			}
			out += " N<" + strings.Join(pairs, ";") + ">"
		}
	}

	for _, block := range []struct {
		label string
		m     map[string]any
	}{{"U", r.U}, {"K", r.K}} {
		if len(block.m) == 0 {
			continue
		}
		keys := make([]string, 0, len(block.m))
		for k := range block.m {
			keys = append(keys, k)
		}
		sort.Strings(keys)
		pairs := make([]string, 0, len(keys))
		for _, k := range keys {
			pairs = append(pairs, k+"="+fmtVal(ctx, block.m[k]))
		}
		out += " " + block.label + "<" + strings.Join(pairs, ";") + ">"
	}

	return out
}

// fmtVal formats a value for a trace line via the context formatter
// (ctx.F, the TS ctx.F), falling back to the engine's Str when unset.
func fmtVal(ctx *tabnas.Context, v any) string {
	if ctx != nil && ctx.F != nil {
		return ctx.F(v)
	}
	return tabnas.Str(v, 44)
}

// ruleId renders a related rule's instance number for a `rule` line;
// the NoRule sentinel (and nil) render as its -1 id, matching TS NORULE.
func ruleId(r *tabnas.Rule) string {
	if r == nil {
		return "-1"
	}
	return strconv.Itoa(r.I)
}

// traceIndent is the per-depth line indent (TS S.indent = ". ").
func traceIndent(d int) string {
	if d < 0 {
		d = 0
	}
	return strings.Repeat(". ", d)
}

// padLeft pads s with leading spaces to at least width characters.
func padLeft(s string, width int) string {
	if len(s) >= width {
		return s
	}
	return strings.Repeat(" ", width-len(s)) + s
}
