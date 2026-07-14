// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

// model_test.go mirrors the "model() structured output" and "print
// option" suites in ../ts/test/debug.test.js. The TypeScript tests build
// their instance from the engine's json-plugin fixture; the Go engine
// ships no grammar, so buildJSONLikeGrammar installs an equivalent
// val/map/list/pair/elem grammar locally and the same assertions run
// against it.
package tabnasdebug_test

import (
	"bytes"
	"encoding/json"
	"reflect"
	"sort"
	"strings"
	"testing"

	tabnas "github.com/tabnas/parser/go"

	tabnasdebug "github.com/tabnas/debug/go"
)

// buildJSONLikeGrammar installs a JSON-shaped grammar (val, map, list,
// pair, elem — the rule set of the TS json fixture) on a fresh instance
// and loads the Debug plugin, mirroring the jsonModel helper in
// ../ts/test/debug.test.js.
func buildJSONLikeGrammar(t *testing.T, tag string) *tabnas.Tabnas {
	t.Helper()
	j := tabnas.Make(tabnas.Options{
		Tag:  tag,
		Rule: &tabnas.RuleOptions{Start: "val"},
	})

	j.Rule("val", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinOB}}, P: "map", B: 1, G: "json,val"},
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinOS}}, P: "list", B: 1, G: "json,val"},
			&tabnas.AltSpec{
				S: [][]tabnas.Tin{{tabnas.TinNR, tabnas.TinST, tabnas.TinTX, tabnas.TinVL}},
				A: func(r *tabnas.Rule, _ *tabnas.Context) { r.Node = r.O0.Val },
				G: "json,val",
			},
		)
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinZZ}}})
	})
	j.Rule("map", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{
			S: [][]tabnas.Tin{{tabnas.TinOB}}, P: "pair",
			N: map[string]int{"pk": 1}, G: "json,map",
		})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCB}}, G: "json,map"})
	})
	j.Rule("list", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinOS}}, P: "elem", G: "json,list"})
		rs.AddClose(&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCS}}, G: "json,list"})
	})
	j.Rule("pair", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{
			S: [][]tabnas.Tin{{tabnas.TinST, tabnas.TinTX}, {tabnas.TinCL}},
			P: "val", G: "json,pair",
		})
		rs.AddClose(
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCA}}, R: "pair", G: "json,pair"},
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCB}}, B: 1, G: "json,pair"},
		)
	})
	j.Rule("elem", func(rs *tabnas.RuleSpec, _ *tabnas.Parser) {
		rs.Clear()
		rs.AddOpen(&tabnas.AltSpec{P: "val", G: "json,elem"})
		rs.AddClose(
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCA}}, R: "elem", G: "json,elem"},
			&tabnas.AltSpec{S: [][]tabnas.Tin{{tabnas.TinCS}}, B: 1, G: "json,elem"},
		)
	})

	if err := j.Use(tabnasdebug.Debug, map[string]any{"print": false, "trace": false}); err != nil {
		t.Fatalf("Use(Debug) returned error: %v", err)
	}
	return j
}

// jsonModel builds the json-like instance and its model, mirroring the
// jsonModel helper in the TypeScript suite.
func jsonModel(t *testing.T, tag string) (*tabnas.Tabnas, *tabnasdebug.DebugModel) {
	t.Helper()
	j := buildJSONLikeGrammar(t, tag)
	m, err := tabnasdebug.Model(j)
	if err != nil {
		t.Fatalf("Model returned error: %v", err)
	}
	if m == nil {
		t.Fatal("Model returned a nil model")
	}
	return j, m
}

// modelRule finds a rule by name in a model's rule list.
func modelRule(m *tabnasdebug.DebugModel, name string) *tabnasdebug.DebugRuleInfo {
	for i := range m.Rules {
		if m.Rules[i].Name == name {
			return &m.Rules[i]
		}
	}
	return nil
}

// modelEdges finds a rule's edges by name in a model's graph.
func modelEdges(m *tabnasdebug.DebugModel, name string) *tabnasdebug.DebugRuleEdges {
	for i := range m.Graph {
		if m.Graph[i].Name == name {
			return &m.Graph[i]
		}
	}
	return nil
}

// TestModelEveryDocumentedSection mirrors "returns an object with every
// documented section": the model carries every documented field, the tag
// round-trips, and the ABNF is a string of grammar text.
func TestModelEveryDocumentedSection(t *testing.T) {
	_, m := jsonModel(t, "demo")

	if m.Tag != "demo" {
		t.Errorf("model tag = %q, want demo", m.Tag)
	}
	if m.Tokens == nil {
		t.Error("model missing section tokens")
	}
	if m.TokenSets == nil {
		t.Error("model missing section tokenSets")
	}
	if m.Rules == nil {
		t.Error("model missing section rules")
	}
	if m.Graph == nil {
		t.Error("model missing section graph")
	}
	if m.Lexer == nil {
		t.Error("model missing section lexer")
	}
	if m.Config.Lex == nil {
		t.Error("model missing section config")
	}
	if m.Plugins == nil {
		t.Error("model missing section plugins")
	}
	if m.Abnf == "" {
		t.Error("model abnf should be a non-empty string")
	}
}

// TestModelRulesAndAlternatesStructured mirrors "describes rules and
// alternates as structured data": the rule set, and the shape of the
// alternate that pushes map from val.
func TestModelRulesAndAlternatesStructured(t *testing.T) {
	_, m := jsonModel(t, "")

	names := make([]string, 0, len(m.Rules))
	for _, r := range m.Rules {
		names = append(names, r.Name)
	}
	sort.Strings(names)
	want := []string{"elem", "list", "map", "pair", "val"}
	if !reflect.DeepEqual(names, want) {
		t.Fatalf("model rule names = %v, want %v", names, want)
	}

	val := modelRule(m, "val")
	if val == nil || len(val.Open) < 2 {
		t.Fatal("val should have several open alts")
	}
	var toMap *tabnasdebug.DebugAltInfo
	for i := range val.Open {
		if val.Open[i].Push == "map" {
			toMap = &val.Open[i]
			break
		}
	}
	if toMap == nil {
		t.Fatal("val should have an alt that pushes map")
	}
	if len(toMap.Seq) == 0 {
		t.Error("alt seq should carry the lookahead token(s)")
	}
	if toMap.Seq[0] != "#OB" {
		t.Errorf("alt seq[0] = %v, want #OB", toMap.Seq[0])
	}
	if toMap.Action {
		t.Error("the push-map alt has no action")
	}
	if toMap.Groups == nil {
		t.Error("alt groups should be a (possibly empty) slice")
	}
	if !reflect.DeepEqual(toMap.Groups, []string{"json", "val"}) {
		t.Errorf("alt groups = %v, want [json val]", toMap.Groups)
	}
	if toMap.Back != 1 {
		t.Errorf("alt back = %d, want 1", toMap.Back)
	}
}

// TestModelRuleReferenceGraph mirrors "exposes the rule-reference graph
// (push/replace edges)".
func TestModelRuleReferenceGraph(t *testing.T) {
	_, m := jsonModel(t, "")

	val := modelEdges(m, "val")
	if val == nil {
		t.Fatal("graph missing rule val")
	}
	openPush := append([]string(nil), val.OpenPush...)
	sort.Strings(openPush)
	if !reflect.DeepEqual(openPush, []string{"list", "map"}) {
		t.Errorf("val openPush = %v, want [list map]", openPush)
	}
	if len(val.OpenReplace) != 0 {
		t.Errorf("val openReplace = %v, want empty", val.OpenReplace)
	}

	mp := modelEdges(m, "map")
	if mp == nil {
		t.Fatal("graph missing rule map")
	}
	found := false
	for _, target := range mp.OpenPush {
		if target == "pair" {
			found = true
		}
	}
	if !found {
		t.Errorf("map should push pair, openPush = %v", mp.OpenPush)
	}

	pair := modelEdges(m, "pair")
	if pair == nil || !reflect.DeepEqual(pair.CloseReplace, []string{"pair"}) {
		t.Errorf("pair closeReplace should be [pair], got %+v", pair)
	}
}

// TestModelConfigAndPlugins mirrors "reports config and plugins
// structurally". The TS fixture asserts the json plugin's presence; the
// Go grammar is installed directly, so the applied-plugin assertion
// covers Debug (named plugin functions are listed by symbol name).
func TestModelConfigAndPlugins(t *testing.T) {
	_, m := jsonModel(t, "")

	if m.Config.Start != "val" {
		t.Errorf("config start = %q, want val", m.Config.Start)
	}
	if _, ok := m.Config.Lex["fixed"]; !ok {
		t.Error("config lex flags should include fixed")
	}
	for _, flag := range []string{"space", "line", "text", "number", "comment", "string", "value"} {
		if _, ok := m.Config.Lex[flag]; !ok {
			t.Errorf("config lex flags should include %s", flag)
		}
	}

	foundDebug := false
	for _, p := range m.Plugins {
		if p.Name == "Debug" {
			foundDebug = true
		}
	}
	if !foundDebug {
		t.Errorf("plugins should list Debug, got %+v", m.Plugins)
	}
}

// TestModelTokenList mirrors "lists tokens with tin, name, and fixed
// literals".
func TestModelTokenList(t *testing.T) {
	_, m := jsonModel(t, "")

	if len(m.Tokens) == 0 {
		t.Fatal("model should list tokens")
	}
	for _, tok := range m.Tokens {
		if tok.Tin <= 0 {
			t.Errorf("token %q has non-positive tin %d", tok.Name, tok.Tin)
		}
		if tok.Name == "" {
			t.Errorf("token with tin %d has empty name", tok.Tin)
		}
	}
	foundFixed := false
	for _, tok := range m.Tokens {
		if tok.Fixed != "" {
			foundFixed = true
		}
	}
	if !foundFixed {
		t.Error("at least one token should carry a fixed literal (json punctuation)")
	}
}

// grammarPortion is the JSON-serialisable grammar subset asserted by
// TestModelJSONRoundTrip, matching the object the TypeScript round-trip
// test builds.
type grammarPortion struct {
	Tag       string                       `json:"tag"`
	Tokens    []tabnasdebug.DebugTokenInfo `json:"tokens"`
	TokenSets []tabnasdebug.DebugTokenSet  `json:"tokenSets"`
	Rules     []tabnasdebug.DebugRuleInfo  `json:"rules"`
	Graph     []tabnasdebug.DebugRuleEdges `json:"graph"`
	Config    tabnasdebug.DebugConfigInfo  `json:"config"`
	Abnf      string                       `json:"abnf"`
}

// TestModelJSONRoundTrip mirrors "the grammar portion is
// JSON-serialisable and round-trips": marshalling the grammar-structure
// fields and unmarshalling them back yields equal data.
func TestModelJSONRoundTrip(t *testing.T) {
	_, m := jsonModel(t, "")

	grammar := grammarPortion{
		Tag: m.Tag, Tokens: m.Tokens, TokenSets: m.TokenSets,
		Rules: m.Rules, Graph: m.Graph, Config: m.Config, Abnf: m.Abnf,
	}
	data, err := json.Marshal(grammar)
	if err != nil {
		t.Fatalf("model grammar portion should be JSON-serialisable: %v", err)
	}
	var round grammarPortion
	if err := json.Unmarshal(data, &round); err != nil {
		t.Fatalf("model grammar portion should unmarshal: %v", err)
	}
	if !reflect.DeepEqual(round.Rules, m.Rules) {
		t.Errorf("rules did not round-trip:\n got %+v\nwant %+v", round.Rules, m.Rules)
	}
	if !reflect.DeepEqual(round.Graph, m.Graph) {
		t.Errorf("graph did not round-trip:\n got %+v\nwant %+v", round.Graph, m.Graph)
	}
	if round.Abnf != m.Abnf {
		t.Errorf("abnf did not round-trip: got %q want %q", round.Abnf, m.Abnf)
	}
}

// TestModelRuleAgreement mirrors "model() and rule() agree on the rule
// set": the model's rule names match the engine's rule-spec map keys.
func TestModelRuleAgreement(t *testing.T) {
	j, m := jsonModel(t, "")

	modelNames := make([]string, 0, len(m.Rules))
	for _, r := range m.Rules {
		modelNames = append(modelNames, r.Name)
	}
	sort.Strings(modelNames)

	rsmNames := make([]string, 0)
	for name := range j.RSM() {
		rsmNames = append(rsmNames, name)
	}
	sort.Strings(rsmNames)

	if !reflect.DeepEqual(modelNames, rsmNames) {
		t.Errorf("model rules %v != engine rules %v", modelNames, rsmNames)
	}
}

// TestModelNilAltInvalid mirrors "renders a null alternate entry as
// ***INVALID*** in the alt seq": the Go counterpart of the TS null alt
// entry is a nil *AltSpec, which must render defensively rather than
// panic.
func TestModelNilAltInvalid(t *testing.T) {
	j := buildTreeGrammar(t)
	if err := j.Use(tabnasdebug.Debug, map[string]any{"print": false, "trace": false}); err != nil {
		t.Fatalf("Use(Debug) returned error: %v", err)
	}
	j.RSM()["top"].AddOpen(nil)

	m, err := tabnasdebug.Model(j)
	if err != nil {
		t.Fatalf("Model returned error on a nil alternate: %v", err)
	}
	top := modelRule(m, "top")
	if top == nil {
		t.Fatal("model missing rule top")
	}
	found := false
	for _, alt := range top.Open {
		for _, s := range alt.Seq {
			if s == "***INVALID***" {
				found = true
			}
		}
	}
	if !found {
		t.Errorf("nil alternate should render ***INVALID*** in the alt seq, got %+v", top.Open)
	}
}

// trivialPlugin is the named no-op plugin the print tests load, the Go
// counterpart of the TS `function myplugin(...)`.
func trivialPlugin(_ *tabnas.Tabnas, _ map[string]any) error { return nil }

// TestPrintLogsUseAndDescribe mirrors the TypeScript "print option" case
// "logs USE: and the describe() dump when a later plugin is used": with
// print on, a later plugin load through tabnasdebug.Use logs a USE: line
// naming the plugin and embedding the full Describe dump.
func TestPrintLogsUseAndDescribe(t *testing.T) {
	var buf bytes.Buffer
	j := tabnas.Make()
	if err := j.Use(tabnasdebug.Debug, map[string]any{
		"print": true, "trace": false, "out": &buf,
	}); err != nil {
		t.Fatalf("Use(Debug) returned error: %v", err)
	}

	if err := tabnasdebug.Use(j, trivialPlugin, map[string]any{}); err != nil {
		t.Fatalf("tabnasdebug.Use returned error: %v", err)
	}

	out := buf.String()
	if !strings.Contains(out, "USE: ") {
		t.Fatalf("print should log a USE: line, got:\n%s", out)
	}
	if !strings.Contains(out, "trivialPlugin") {
		t.Errorf("USE: log should name the plugin:\n%s", out)
	}
	if !strings.Contains(out, "========= INSTANCE ========") {
		t.Errorf("USE: log should embed the Describe dump:\n%s", out)
	}
}

// TestPrintDisabledIsSilent checks the inverse of the print case: with
// print off, a later plugin load through tabnasdebug.Use logs nothing.
func TestPrintDisabledIsSilent(t *testing.T) {
	var buf bytes.Buffer
	j := tabnas.Make()
	if err := j.Use(tabnasdebug.Debug, map[string]any{
		"print": false, "trace": false, "out": &buf,
	}); err != nil {
		t.Fatalf("Use(Debug) returned error: %v", err)
	}

	if err := tabnasdebug.Use(j, trivialPlugin, map[string]any{}); err != nil {
		t.Fatalf("tabnasdebug.Use returned error: %v", err)
	}

	if out := buf.String(); out != "" {
		t.Errorf("print disabled should log nothing, got:\n%s", out)
	}
}
