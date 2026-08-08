/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root — the same convention @tabnas/parser and @tabnas/abnf use
// (see ../../test/AGENTS.md).
//
// `go/parity_test.go` discovers and runs the SAME files, so the two
// implementations cannot drift without one of them going red. A row names a
// grammar from the shared registry (fixture.js / go/fixture_test.go) and
// pins what debug reports about it.

const { describe, it } = require('node:test')
const assert = require('node:assert')
const Fs = require('node:fs')
const Path = require('node:path')

const { GRAMMARS } = require('./fixture')

const specDir = Path.join(__dirname, '..', '..', 'test', 'spec')

// A describe() section banner, e.g. "========= TOKENS ========".
const SECTION_HEADER = /^=+ .* =+$/

// Rules and graph entries carry a `name`; the two runtimes order them
// differently by design (TS insertion order, Go sorted by name — see
// ../../docs/reference.md), so a shared fixture compares them sorted.
const byName = (list) =>
  [...list].sort((a, b) => (a.name < b.name ? -1 : a.name > b.name ? 1 : 0))

// What the second column holds, keyed by its header name.
const REPORTERS = {
  abnf: (tn) => tn.debug.abnf(),
  sections: (tn) =>
    tn.debug.describe().split('\n').filter((l) => SECTION_HEADER.test(l)),
  // The grammar-structure portion of model(), as it serialises. Pins the
  // cross-runtime claim that the Go DebugModel's JSON tags match the TS
  // field names. Instance-level sections are excluded: `lexer` is
  // summarised in Go and the Go fixtures need not load the debug plugin,
  // so those two legitimately differ. `tag` no longer differs by design
  // — the engine now defaults an unset tag to '-' in BOTH runtimes — but
  // it stays out until go/go.mod moves past that engine alignment, since
  // the Go suite runs GOWORK=off against the pinned pre-alignment
  // engine. See ../../docs/reference.md, "Engine-version note".
  model: (tn) => {
    const m = tn.debug.model()
    // Round-trip through JSON so absent optionals drop out on both sides.
    return JSON.parse(
      JSON.stringify({ rules: byName(m.rules), graph: byName(m.graph) }),
    )
  },
}

// The header row's second name selects the reporter run over the grammar.
function loadSpec(file) {
  const body = Fs.readFileSync(Path.join(specDir, file), 'utf8')
  const lines = body.split(/\r?\n/)
  const kind = (lines[0] ?? '').split('\t')[1]
  if (!REPORTERS[kind]) {
    throw new Error(`${file}: unknown second column ${JSON.stringify(kind)}`)
  }
  const rows = []
  for (let i = 1; i < lines.length; i++) {
    const raw = lines[i]
    // A comment line starts with '#' and has no tab; a data row always has
    // at least one (grammar + expected).
    if ('' === raw || (raw.startsWith('#') && !raw.includes('\t'))) continue
    const cols = raw.split('\t')
    if (cols.length < 2) {
      throw new Error(`${file}:${i + 1}: expected at least 2 tab-separated columns`)
    }
    rows.push({ line: i + 1, grammar: cols[0], expected: cols[1] })
  }
  return { kind, rows }
}

function runSpec(file) {
  const { kind, rows } = loadSpec(file)
  describe('spec: ' + file, () => {
    assert.ok(0 < rows.length, file + ': no cases')
    const report = REPORTERS[kind]
    for (const row of rows) {
      it(`row ${row.line}: ${row.grammar}`, () => {
        const build = GRAMMARS[row.grammar]
        assert.ok(build, `${file}:${row.line}: unknown grammar fixture ` +
          JSON.stringify(row.grammar))
        assert.deepStrictEqual(report(build()), JSON.parse(row.expected),
          `${file}:${row.line}`)
      })
    }
  })
}

// Auto-discover every fixture: adding a .tsv runs it in both runtimes
// without touching either runner.
for (const file of Fs.readdirSync(specDir).sort()) {
  if (file.endsWith('.tsv')) runSpec(file)
}
