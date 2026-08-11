// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasdebug_test

// parity_test.go — cross-runtime conformance, driven by the shared
// `test/spec/*.tsv` fixtures at the repo root (see ../test/AGENTS.md).
//
// The fixture loader, the ERROR: contract and the row loop all come from
// github.com/tabnas/support/go, whose TypeScript half ts/test/parity.test.js
// uses to run the SAME files — so the two implementations cannot drift
// without one of them going red, and neither can the two loaders.
//
// What is left here is only what is specific to debug: a row names a
// GRAMMAR from the shared registry (fixture_test.go / ts/test/fixture.js),
// and the second column's header names what is reported about it.

import (
	"encoding/json"
	"fmt"
	"regexp"
	"sort"
	"strings"
	"testing"

	tabnasdebug "github.com/tabnas/debug/go"
	tabnas "github.com/tabnas/parser/go"
	support "github.com/tabnas/support/go"
)

// sectionHeader matches a describe() section banner, e.g.
// "========= TOKENS ========".
var sectionHeader = regexp.MustCompile(`^=+ .* =+$`)

// reporters is what the second column holds, keyed by its header name.
var reporters = map[string]func(*tabnas.Tabnas) (any, error){
	"abnf": func(j *tabnas.Tabnas) (any, error) {
		return tabnasdebug.Abnf(j)
	},

	"sections": func(j *tabnas.Tabnas) (any, error) {
		out, err := tabnasdebug.Describe(j)
		if err != nil {
			return nil, err
		}
		got := []any{}
		for _, line := range strings.Split(out, "\n") {
			if sectionHeader.MatchString(line) {
				got = append(got, line)
			}
		}
		return got, nil
	},

	// The grammar-structure portion of Model, as it serialises. Pins the
	// cross-runtime claim that the Go DebugModel's JSON tags match the TS
	// field names. Instance-level sections are excluded: Lexer is
	// summarised in Go and the Go fixtures need not load the debug plugin,
	// so those two legitimately differ. Tag no longer differs by design —
	// the engine now defaults an unset tag to "-" in BOTH runtimes — but it
	// stays out until go.mod moves past that engine alignment, since this
	// suite runs GOWORK=off against the pinned pre-alignment engine. See
	// ../docs/reference.md, "Engine-version note".
	"model": func(j *tabnas.Tabnas) (any, error) {
		m, err := tabnasdebug.Model(j)
		if err != nil {
			return nil, err
		}

		// The runtimes order rules/graph differently by design (TS
		// insertion order, Go by name — see ../docs/reference.md), so the
		// shared fixture compares them sorted by name.
		//
		// make+copy, NOT append to a nil slice: appending nothing to nil
		// yields nil, which would marshal as `null` and lose the empty-list
		// distinction Model deliberately preserves.
		rules := make([]tabnasdebug.DebugRuleInfo, len(m.Rules))
		copy(rules, m.Rules)
		sort.Slice(rules, func(a, b int) bool { return rules[a].Name < rules[b].Name })

		graph := make([]tabnasdebug.DebugRuleEdges, len(m.Graph))
		copy(graph, m.Graph)
		sort.Slice(graph, func(a, b int) bool { return graph[a].Name < graph[b].Name })

		return map[string]any{"rules": rules, "graph": graph}, nil
	},
}

// TestSpec runs every fixture in the spec directory. The header row's
// second name selects the reporter, which is per file — hence a runner per
// file rather than one over the directory.
func TestSpec(t *testing.T) {
	dir, err := support.FindSpecDir("")
	if err != nil {
		t.Fatal(err)
	}

	specs, err := support.LoadSpecDir(dir, nil)
	if err != nil {
		t.Fatal(err)
	}

	for _, spec := range specs {
		if 2 > len(spec.Header) {
			t.Fatalf("%s: expected at least two columns", spec.Name)
		}
		kind := spec.Header[1]
		report, ok := reporters[kind]
		if !ok {
			t.Fatalf("%s: unknown second column %q", spec.Name, kind)
		}

		support.Runner{
			Parse: func(grammar string) (any, error) {
				build, ok := grammars[grammar]
				if !ok {
					return nil, fmt.Errorf("unknown grammar fixture %q", grammar)
				}
				return report(build())
			},

			// Compare as decoded JSON so both sides are the same shape of
			// generic value and field ORDER is irrelevant.
			Normalize: jsonFlatten,

			InputName:    "grammar",
			ExpectedName: kind,
		}.Spec(t, spec)
	}
}

// jsonFlatten renders a value as JSON and reads it back as plain
// map/slice/float64/string/bool/nil. A value that will not marshal is
// returned as it is: the comparison then fails and prints it, which says
// more than a panic here would.
func jsonFlatten(v any) any {
	raw, err := json.Marshal(v)
	if err != nil {
		return v
	}
	var out any
	if err := json.Unmarshal(raw, &out); err != nil {
		return v
	}
	return out
}
