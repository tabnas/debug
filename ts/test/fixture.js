/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

// Named grammar fixtures shared by the spec runner and the in-language
// tests. Their Go counterparts are in `go/fixture_test.go` — the two
// registries must stay in step, because `test/spec/*.tsv` addresses a
// grammar by NAME and both runtimes must build the same one.
//
// The grammars are hand-written against the engine on purpose: @tabnas/abnf
// must NOT become a dependency of @tabnas/debug (the emitter reads only the
// live engine), so no fixture may be compiled from ABNF source here.

const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('../dist/debug.js')

// bare: the engine with nothing installed but the debug plugin. Pins what
// describe() emits for an instance with no grammar at all.
function bare() {
  const tn = new Tabnas()
  tn.use(Debug, { print: false, trace: false })
  return tn
}

// add: `val` pushes `add`; `add` matches #NR then optionally a #PL-replace
// back into `add`, with an epsilon close and the #ZZ end close. Exercises
// the emitter's optional folding (`[ PL add ]`) and token definitions.
function add() {
  const tn = new Tabnas({
    fixed: { token: { '#PL': '+' } },
    rule: { start: 'val' },
  })
  tn.use(Debug, { print: false, trace: false })
  tn.rule('val', (rs) => rs.clear().open([{ p: 'add' }]))
  tn.rule('add', (rs) =>
    rs
      .clear()
      .open([{ s: ['#NR'] }])
      .close([{ s: ['#PL'], r: 'add' }, {}, { s: ['#ZZ'] }]))
  return tn
}

// greet: a two-way alternation over case-sensitive literals. Exercises the
// emitter's `/` alternation and %s"…" literal rendering.
function greet() {
  const tn = new Tabnas({
    fixed: { token: { '#HI': 'hi', '#HE': 'hello' } },
    rule: { start: 'greet' },
  })
  tn.use(Debug, { print: false, trace: false })
  tn.rule('greet', (rs) =>
    rs.clear().open([{ s: ['#HI'] }, { s: ['#HE'] }]).close([{ s: ['#ZZ'] }]))
  return tn
}

const GRAMMARS = { bare, add, greet }

module.exports = { GRAMMARS }
