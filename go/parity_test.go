// Copyright (c) 2026 Richard Rodger and other contributors, MIT License

package tabnasdebug_test

// parity_test.go — cross-runtime conformance, driven by the shared
// `test/spec/*.tsv` fixtures at the repo root (see ../test/AGENTS.md), the
// same convention @tabnas/parser and @tabnas/abnf use.
//
// ts/test/parity.test.js discovers and runs the SAME files, so the two
// implementations cannot drift without one of them going red. A row names a
// grammar from the shared registry (fixture_test.go / ts/test/fixture.js)
// and pins what debug reports about it.

import (
	"bufio"
	"encoding/json"
	"os"
	"path/filepath"
	"reflect"
	"regexp"
	"sort"
	"strings"
	"testing"

	tabnasdebug "github.com/tabnas/debug/go"
)

type specRow struct {
	file     string
	lineNo   int
	grammar  string
	expected string
}

func specDir() string { return filepath.Join("..", "test", "spec") }

// sectionHeader matches a describe() section banner, e.g.
// "========= TOKENS ========".
var sectionHeader = regexp.MustCompile(`^=+ .* =+$`)

// loadSpec reads one fixture. The header row's SECOND column names what the
// runner reports about the grammar in the first column ("abnf", "sections"
// or "model").
func loadSpec(t *testing.T, path string) (string, []specRow) {
	t.Helper()
	f, err := os.Open(path)
	if err != nil {
		t.Fatalf("open %s: %v", path, err)
	}
	defer f.Close()

	kind := ""
	var rows []specRow
	scanner := bufio.NewScanner(f)
	scanner.Buffer(make([]byte, 0, 64*1024), 1<<20)
	lineNo := 0
	for scanner.Scan() {
		lineNo++
		// Strip the CR of a CRLF line: the TS loader splits on /\r?\n/ and
		// drops it, so keeping it here would feed the runtimes different bytes.
		line := strings.TrimSuffix(scanner.Text(), "\r")
		if lineNo == 1 {
			cols := strings.Split(line, "\t")
			if len(cols) < 2 {
				t.Fatalf("%s: header must name at least 2 columns", path)
			}
			kind = cols[1]
			continue
		}
		// A comment line starts with '#' and has no tab; a data row always
		// has at least one (grammar + expected).
		if line == "" || (strings.HasPrefix(line, "#") && !strings.Contains(line, "\t")) {
			continue
		}
		cols := strings.Split(line, "\t")
		if len(cols) < 2 {
			t.Fatalf("%s:%d: expected at least 2 tab-separated columns", path, lineNo)
		}
		rows = append(rows, specRow{
			file:     filepath.Base(path),
			lineNo:   lineNo,
			grammar:  cols[0],
			expected: cols[1],
		})
	}
	if err := scanner.Err(); err != nil {
		t.Fatalf("read %s: %v", path, err)
	}
	if len(rows) == 0 {
		t.Fatalf("%s: no cases", path)
	}
	return kind, rows
}

func runSpecFile(t *testing.T, path string) {
	kind, rows := loadSpec(t, path)
	for _, row := range rows {
		t.Run(row.grammar, func(t *testing.T) {
			build, ok := grammars[row.grammar]
			if !ok {
				t.Fatalf("%s:%d: unknown grammar fixture %q", row.file, row.lineNo, row.grammar)
			}
			j := build()

			switch kind {
			case "abnf":
				got, err := tabnasdebug.Abnf(j)
				if err != nil {
					t.Fatalf("%s:%d: Abnf: %v", row.file, row.lineNo, err)
				}
				var want string
				if err := json.Unmarshal([]byte(row.expected), &want); err != nil {
					t.Fatalf("%s:%d: bad expected JSON %q: %v", row.file, row.lineNo, row.expected, err)
				}
				if got != want {
					t.Errorf("%s:%d:\n  got  %q\n  want %q", row.file, row.lineNo, got, want)
				}

			case "sections":
				out, err := tabnasdebug.Describe(j)
				if err != nil {
					t.Fatalf("%s:%d: Describe: %v", row.file, row.lineNo, err)
				}
				got := []string{}
				for _, line := range strings.Split(out, "\n") {
					if sectionHeader.MatchString(line) {
						got = append(got, line)
					}
				}
				var want []string
				if err := json.Unmarshal([]byte(row.expected), &want); err != nil {
					t.Fatalf("%s:%d: bad expected JSON %q: %v", row.file, row.lineNo, row.expected, err)
				}
				if !reflect.DeepEqual(got, want) {
					t.Errorf("%s:%d:\n  got  %v\n  want %v", row.file, row.lineNo, got, want)
				}

			case "model":
				// The grammar-structure portion of Model, as it serialises.
				// Pins the cross-runtime claim that the Go DebugModel's JSON
				// tags match the TS field names. Instance-level sections are
				// excluded: Lexer is summarised in Go and the Go fixtures
				// need not load the debug plugin, so those two legitimately
				// differ. Tag no longer differs by design — the engine now
				// defaults an unset tag to "-" in BOTH runtimes — but it
				// stays out until go.mod moves past that engine alignment,
				// since this suite runs GOWORK=off against the pinned
				// pre-alignment engine. See ../docs/reference.md,
				// "Engine-version note".
				m, err := tabnasdebug.Model(j)
				if err != nil {
					t.Fatalf("%s:%d: Model: %v", row.file, row.lineNo, err)
				}
				// The runtimes order rules/graph differently by design (TS
				// insertion order, Go by name — see ../docs/reference.md), so
				// the shared fixture compares them sorted by name.
				// make+copy, NOT append to a nil slice: appending nothing to
				// nil yields nil, which would marshal as `null` and lose the
				// empty-list distinction Model deliberately preserves.
				rules := make([]tabnasdebug.DebugRuleInfo, len(m.Rules))
				copy(rules, m.Rules)
				sort.Slice(rules, func(a, b int) bool { return rules[a].Name < rules[b].Name })
				graph := make([]tabnasdebug.DebugRuleEdges, len(m.Graph))
				copy(graph, m.Graph)
				sort.Slice(graph, func(a, b int) bool { return graph[a].Name < graph[b].Name })

				// Compare as decoded JSON so both sides are the same shape of
				// generic value and field ORDER is irrelevant.
				raw, err := json.Marshal(map[string]any{"rules": rules, "graph": graph})
				if err != nil {
					t.Fatalf("%s:%d: marshal model: %v", row.file, row.lineNo, err)
				}
				var got, want any
				if err := json.Unmarshal(raw, &got); err != nil {
					t.Fatalf("%s:%d: decode model: %v", row.file, row.lineNo, err)
				}
				if err := json.Unmarshal([]byte(row.expected), &want); err != nil {
					t.Fatalf("%s:%d: bad expected JSON %q: %v", row.file, row.lineNo, row.expected, err)
				}
				if !reflect.DeepEqual(got, want) {
					gotJSON, _ := json.Marshal(got)
					t.Errorf("%s:%d:\n  got  %s\n  want %s",
						row.file, row.lineNo, gotJSON, row.expected)
				}

			default:
				t.Fatalf("%s: unknown second column %q", row.file, kind)
			}
		})
	}
}

// TestSpec auto-discovers every fixture: adding a .tsv runs it in both
// runtimes without touching either runner.
func TestSpec(t *testing.T) {
	files, err := filepath.Glob(filepath.Join(specDir(), "*.tsv"))
	if err != nil {
		t.Fatalf("glob spec dir: %v", err)
	}
	if len(files) == 0 {
		t.Fatalf("no spec files under %s", specDir())
	}
	for _, path := range files {
		t.Run(filepath.Base(path), func(t *testing.T) { runSpecFile(t, path) })
	}
}
