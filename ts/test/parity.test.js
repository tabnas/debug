/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

// Cross-runtime conformance, driven by the shared `test/spec/*.tsv` fixtures
// at the repo root (see ../../test/AGENTS.md).
//
// The fixture loader, the `ERROR:` contract and the row loop all come from
// @tabnas/support, whose Go half `go/parity_test.go` uses to run the SAME
// files — so the two implementations cannot drift without one of them
// going red, and neither can the two loaders.
//
// What is left here is only what is specific to debug: a row names a
// GRAMMAR from the shared registry (fixture.js / go/fixture_test.go), and
// the second column's header names what is reported about it.

const { findSpecDir, loadSpecDir, makeRunner } = require('@tabnas/support')

const { GRAMMARS } = require('./fixture')

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

// The header row's second name selects the reporter, which is per file —
// hence a runner per file rather than one over the directory.
for (const spec of loadSpecDir(findSpecDir(__dirname))) {
  const kind = spec.header[1]
  const report = REPORTERS[kind]
  if (!report) {
    throw new Error(
      `${spec.file}: unknown second column ${JSON.stringify(kind)}`)
  }

  makeRunner({
    parse: (grammar, row) => {
      const build = GRAMMARS[grammar]
      if (!build) {
        throw new Error(
          `${row.where()}: unknown grammar fixture ${JSON.stringify(grammar)}`)
      }
      return report(build())
    },

    input: 'grammar',
    expected: kind,
  }).spec(spec)
}
