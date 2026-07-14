// Copyright (c) 2021-2026 Richard Rodger, MIT License

// model.go implements the structured counterpart to Describe: the
// instance/grammar as a typed, JSON-serialisable object, mirroring the
// canonical TypeScript tabnas.debug.model() and its exported types in
// ../ts/src/debug.ts (DebugModel, DebugTokenInfo, DebugTokenSet,
// DebugAltInfo, DebugRuleInfo, DebugRuleEdges, DebugLexMatcher,
// DebugConfigInfo, DebugPluginInfo).
package tabnasdebug

import (
	"reflect"
	"runtime"
	"sort"
	"strings"

	tabnas "github.com/tabnas/parser/go"
)

// DebugTokenInfo is one row of the token table: the token's tin (token
// identification number), its name, and — for fixed (literal) tokens —
// the source text it matches. Mirrors the TS DebugTokenInfo.
type DebugTokenInfo struct {
	Tin   int    `json:"tin"`             // Token identification number.
	Name  string `json:"name"`            // Token name (e.g. "#NR").
	Fixed string `json:"fixed,omitempty"` // Fixed source text, when a literal token.
}

// DebugTokenSet is a named token set and its member tins. Mirrors the TS
// DebugTokenSet.
type DebugTokenSet struct {
	Name string `json:"name"` // Set name (IGNORE, VAL, KEY).
	Tins []int  `json:"tins"` // Member tins, ascending.
}

// DebugAltInfo is the structured form of a single rule alternate (the
// data behind the ALTS text of Describe). Mirrors the TS DebugAltInfo.
//
// Seq entries are token names (string) per lookahead position; a
// multi-token position is a nested []any of names, and a wildcard
// (unconstrained) position is the empty string. A nil alternate renders
// as the single entry "***INVALID***". Push/Replace hold the literal
// target rule name, or "<fn>" for a function-valued (PF/RF) target.
type DebugAltInfo struct {
	Seq      []any          `json:"seq"`                // Token name(s) per lookahead position.
	Push     string         `json:"push,omitempty"`     // `p` target rule (or "<fn>").
	Replace  string         `json:"replace,omitempty"`  // `r` target rule (or "<fn>").
	Back     int            `json:"back,omitempty"`     // `b` token push-back.
	Counters map[string]int `json:"counters,omitempty"` // `n` counter ops.
	Groups   []string       `json:"groups"`             // `g` group tags.
	Action   bool           `json:"action"`             // `a` present.
	Cond     bool           `json:"cond"`               // `c` (or declarative `CD`) present.
	Modifier bool           `json:"modifier"`           // `h` present.
}

// DebugRuleInfo is one rule: its name and its open/close alternates as
// structured data. Mirrors the TS DebugRuleInfo.
type DebugRuleInfo struct {
	Name  string         `json:"name"`  // Rule name.
	Open  []DebugAltInfo `json:"open"`  // Open-phase alternates, in order.
	Close []DebugAltInfo `json:"close"` // Close-phase alternates, in order.
}

// DebugRuleEdges is one rule's outgoing edges in the rule-reference
// graph: the distinct push/replace rule-name targets of its open and
// close alternates (function-valued targets recorded as "<fn>"). Mirrors
// the TS DebugRuleEdges.
type DebugRuleEdges struct {
	Name         string   `json:"name"`         // Rule name.
	OpenPush     []string `json:"openPush"`     // Distinct `p` targets of open alts.
	OpenReplace  []string `json:"openReplace"`  // Distinct `r` targets of open alts.
	ClosePush    []string `json:"closePush"`    // Distinct `p` targets of close alts.
	CloseReplace []string `json:"closeReplace"` // Distinct `r` targets of close alts.
}

// DebugLexMatcher is one lexer matcher, in priority order. Mirrors the
// TS DebugLexMatcher: Order is the priority, Matcher the registered
// matcher name, and Make the underlying function's name (the Go analogue
// of the TS factory name; empty when unrecoverable). The Go engine
// enumerates only custom matchers; the built-in matchers appear as
// enable flags under Config.Lex instead.
type DebugLexMatcher struct {
	Order   int    `json:"order"`   // Priority (lower runs first).
	Matcher string `json:"matcher"` // Registered matcher name.
	Make    string `json:"make"`    // Matcher function name, when recoverable.
}

// DebugConfigInfo reports the key parser settings: start rule, finish
// flag, safe-key, and the built-in per-lexer enable flags (fixed, space,
// line, text, number, comment, string, value). Mirrors the TS
// DebugConfigInfo.
type DebugConfigInfo struct {
	Start   string          `json:"start"`   // Starting rule name.
	Finish  bool            `json:"finish"`  // Auto-close unclosed structures at EOF.
	SafeKey bool            `json:"safeKey"` // Prevent __proto__ keys.
	Lex     map[string]bool `json:"lex"`     // Built-in lexer enable flags.
}

// DebugPluginInfo is one applied plugin: its name (derived from the
// plugin function's symbol, the Go analogue of the TS function name) and
// its options when registered in the instance's plugin-options namespace
// (Tabnas.PluginOptions). Mirrors the TS DebugPluginInfo.
type DebugPluginInfo struct {
	Name    string         `json:"name"`              // Plugin function name.
	Options map[string]any `json:"options,omitempty"` // Plugin options, when known.
}

// DebugModel is the full structured description of an instance: the
// token table, token sets, rules and alternates as data, the
// rule-reference graph, lexer matchers, config, plugins, and the ABNF
// text. The grammar-structure fields are JSON-serialisable and
// round-trip through encoding/json. Mirrors the TS DebugModel.
type DebugModel struct {
	Tag       string            `json:"tag"`       // Instance tag ("" when unset).
	Tokens    []DebugTokenInfo  `json:"tokens"`    // The token table.
	TokenSets []DebugTokenSet   `json:"tokenSets"` // Named token sets.
	Rules     []DebugRuleInfo   `json:"rules"`     // Rules and their alternates.
	Graph     []DebugRuleEdges  `json:"graph"`     // Per-rule push/replace edges.
	Lexer     []DebugLexMatcher `json:"lexer"`     // Lexer matchers, priority order.
	Config    DebugConfigInfo   `json:"config"`    // Key parser settings.
	Plugins   []DebugPluginInfo `json:"plugins"`   // Applied plugins.
	Abnf      string            `json:"abnf"`      // Same text as Abnf(j).
}

// Model returns the structured counterpart to Describe: the instance's
// active grammar and configuration as a typed, JSON-serialisable
// *DebugModel, mirroring the canonical TypeScript tabnas.debug.model().
//
// Unlike the TypeScript model(), which returns a bare object, the Go
// form returns (*DebugModel, error): it upholds the engine's no-panic
// guarantee. Malformed grammar specs (a nil rule spec, a nil alternate)
// are rendered defensively (a nil alternate's Seq is ["***INVALID***"]),
// and any remaining panic is recovered and returned as an
// "internal"-code *tabnas.TabnasError with a nil model. On success the
// error is nil.
//
// Rule and token ordering is deterministic (rules sorted by name, tokens
// by tin) rather than TS insertion order; see ../docs/reference.md.
func Model(j *tabnas.Tabnas) (m *DebugModel, err error) {
	defer func() {
		if r := recover(); r != nil {
			m, err = nil, internalError("Model", r)
		}
	}()

	cfg := j.Config()
	rsm := j.RSM()

	m = &DebugModel{
		Tag:       j.Options().Tag,
		Tokens:    modelTokens(j, cfg),
		TokenSets: modelTokenSets(j),
		Rules:     modelRules(j, rsm),
		Graph:     modelGraph(rsm),
		Lexer:     modelLexer(cfg),
		Config:    modelConfig(cfg),
		Plugins:   modelPlugins(j),
		Abnf:      emitAbnf(j),
	}
	return m, nil
}

// modelTokens builds the token table with the same iteration and order
// as describeTokens: built-in tins in canonical order, then custom tins
// ascending; fixed tokens carry their source text.
func modelTokens(j *tabnas.Tabnas, cfg *tabnas.LexConfig) []DebugTokenInfo {
	tokens := make([]DebugTokenInfo, 0, int(tabnas.TinMAX))
	if cfg == nil {
		return tokens
	}

	fixedSrc := make(map[tabnas.Tin]string, len(cfg.FixedTokens))
	for src, tin := range cfg.FixedTokens {
		fixedSrc[tin] = src
	}

	add := func(tin tabnas.Tin, name string) {
		info := DebugTokenInfo{Tin: int(tin), Name: name}
		if src, ok := fixedSrc[tin]; ok && src != "" {
			info.Fixed = src
		}
		tokens = append(tokens, info)
	}

	for tin := tabnas.TinBD; tin < tabnas.TinMAX; tin++ {
		add(tin, j.TinName(tin))
	}
	custom := make([]tabnas.Tin, 0, len(cfg.TinNames))
	for tin := range cfg.TinNames {
		if tin >= tabnas.TinMAX {
			custom = append(custom, tin)
		}
	}
	sort.Ints(custom)
	for _, tin := range custom {
		add(tin, cfg.TinNames[tin])
	}
	return tokens
}

// modelTokenSets builds the named token sets, mirroring
// describeTokenSets: the engine's enumerable sets (IGNORE, VAL, KEY)
// with member tins sorted ascending for determinism.
func modelTokenSets(j *tabnas.Tabnas) []DebugTokenSet {
	sets := make([]DebugTokenSet, 0, 3)
	for _, name := range []string{"IGNORE", "VAL", "KEY"} {
		tins := j.TokenSet(name)
		if tins == nil {
			continue
		}
		sorted := make([]int, len(tins))
		copy(sorted, tins)
		sort.Ints(sorted)
		sets = append(sets, DebugTokenSet{Name: name, Tins: sorted})
	}
	return sets
}

// modelRules builds the rules-as-data section: each rule's open and
// close alternates via altInfo, rules sorted by name.
func modelRules(j *tabnas.Tabnas, rsm map[string]*tabnas.RuleSpec) []DebugRuleInfo {
	names := sortedRuleNames(rsm)
	rules := make([]DebugRuleInfo, 0, len(names))
	for _, name := range names {
		info := DebugRuleInfo{
			Name:  name,
			Open:  []DebugAltInfo{},
			Close: []DebugAltInfo{},
		}
		if rs := rsm[name]; rs != nil {
			for _, a := range rs.OpenAlts() {
				info.Open = append(info.Open, altInfo(j, a))
			}
			for _, a := range rs.CloseAlts() {
				info.Close = append(info.Close, altInfo(j, a))
			}
		}
		rules = append(rules, info)
	}
	return rules
}

// altInfo builds the structured form of a single alternate (the data
// behind descAltPhase's text), mirroring the TS altInfo(). A nil
// alternate — the Go counterpart of the TS null alt entry — renders
// defensively with Seq ["***INVALID***"].
func altInfo(j *tabnas.Tabnas, a *tabnas.AltSpec) DebugAltInfo {
	info := DebugAltInfo{Seq: []any{}, Groups: []string{}}
	if a == nil {
		info.Seq = append(info.Seq, "***INVALID***")
		return info
	}

	for _, pos := range a.S {
		names := make([]string, 0, len(pos))
		for _, tin := range pos {
			names = append(names, j.TinName(tin))
		}
		switch len(names) {
		case 0:
			// Wildcard position (no tin constraint), as in altSeq.
			info.Seq = append(info.Seq, "")
		case 1:
			info.Seq = append(info.Seq, names[0])
		default:
			multi := make([]any, len(names))
			for i, n := range names {
				multi[i] = n
			}
			info.Seq = append(info.Seq, multi)
		}
	}

	if a.P != "" {
		info.Push = a.P
	} else if a.PF != nil {
		info.Push = "<fn>"
	}
	if a.R != "" {
		info.Replace = a.R
	} else if a.RF != nil {
		info.Replace = "<fn>"
	}
	info.Back = a.B
	if len(a.N) > 0 {
		counters := make(map[string]int, len(a.N))
		for k, v := range a.N {
			counters[k] = v
		}
		info.Counters = counters
	}
	if a.G != "" {
		for _, g := range strings.Split(a.G, ",") {
			if g = strings.TrimSpace(g); g != "" {
				info.Groups = append(info.Groups, g)
			}
		}
	}
	info.Action = a.A != nil
	info.Cond = a.C != nil || len(a.CD) > 0
	info.Modifier = a.H != nil
	return info
}

// modelGraph builds the rule-reference graph: per rule, the distinct
// push/replace targets of its open and close alternates (the structured
// form of the RULES transition tree), rules sorted by name.
func modelGraph(rsm map[string]*tabnas.RuleSpec) []DebugRuleEdges {
	names := sortedRuleNames(rsm)
	graph := make([]DebugRuleEdges, 0, len(names))
	for _, name := range names {
		rs := rsm[name]
		graph = append(graph, DebugRuleEdges{
			Name:         name,
			OpenPush:     modelEdges(rs, "open", "p"),
			OpenReplace:  modelEdges(rs, "open", "r"),
			ClosePush:    modelEdges(rs, "close", "p"),
			CloseReplace: modelEdges(rs, "close", "r"),
		})
	}
	return graph
}

// modelEdges adapts ruleEdgeTargets to the TS model convention, where a
// function-valued target is recorded as "<fn>" (the text tree uses
// "<F>").
func modelEdges(rs *tabnas.RuleSpec, phase, step string) []string {
	targets := ruleEdgeTargets(rs, phase, step)
	for i, t := range targets {
		if t == "<F>" {
			targets[i] = "<fn>"
		}
	}
	return targets
}

// modelLexer lists the custom lexer matchers in priority order. Order is
// the matcher's priority, Matcher its registered name, and Make the
// underlying function's name when recoverable.
func modelLexer(cfg *tabnas.LexConfig) []DebugLexMatcher {
	matchers := []DebugLexMatcher{}
	if cfg == nil {
		return matchers
	}
	for _, m := range cfg.CustomMatchers {
		if m == nil {
			continue
		}
		matchers = append(matchers, DebugLexMatcher{
			Order:   m.Priority,
			Matcher: m.Name,
			Make:    goFuncName(m.Match),
		})
	}
	return matchers
}

// modelConfig reports the key parser settings, the structured form of
// describeConfig.
func modelConfig(cfg *tabnas.LexConfig) DebugConfigInfo {
	info := DebugConfigInfo{Lex: map[string]bool{}}
	if cfg == nil {
		return info
	}
	info.Start = cfg.RuleStart
	info.Finish = cfg.FinishRule
	info.SafeKey = cfg.SafeKey
	info.Lex = map[string]bool{
		"fixed":   cfg.FixedLex,
		"space":   cfg.SpaceLex,
		"line":    cfg.LineLex,
		"text":    cfg.TextLex,
		"number":  cfg.NumberLex,
		"comment": cfg.CommentLex,
		"string":  cfg.StringLex,
		"value":   cfg.ValueLex,
	}
	return info
}

// modelPlugins lists the applied plugins in load order. Names are
// derived from the plugin function's symbol (the Go engine stores
// plugins as bare functions); options are attached when the plugin
// registered them in the instance's plugin-options namespace.
func modelPlugins(j *tabnas.Tabnas) []DebugPluginInfo {
	plugins := []DebugPluginInfo{}
	for _, p := range j.Plugins() {
		info := DebugPluginInfo{Name: pluginName(p)}
		if opts := j.PluginOptions(info.Name); opts != nil {
			info.Options = opts
		}
		plugins = append(plugins, info)
	}
	return plugins
}

// pluginName derives a plugin's short name from its function symbol —
// the Go analogue of the TS `plugin.name` (a function's name property).
// E.g. github.com/tabnas/debug/go.Debug -> "Debug".
func pluginName(p tabnas.Plugin) string {
	return goFuncName(p)
}

// goFuncName returns the short name of a function value's symbol: the
// full runtime name with any method-value suffix ("-fm") removed, then
// trimmed to the part after the package path. Returns "" for nil or
// non-func values.
func goFuncName(fn any) string {
	if fn == nil {
		return ""
	}
	v := reflect.ValueOf(fn)
	if v.Kind() != reflect.Func || v.IsNil() {
		return ""
	}
	f := runtime.FuncForPC(v.Pointer())
	if f == nil {
		return ""
	}
	name := strings.TrimSuffix(f.Name(), "-fm")
	if i := strings.LastIndex(name, "/"); i >= 0 {
		name = name[i+1:]
	}
	if i := strings.Index(name, "."); i >= 0 {
		name = name[i+1:]
	}
	return name
}
