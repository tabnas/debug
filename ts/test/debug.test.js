/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

const { describe, it } = require('node:test')
const assert = require('node:assert')
const fs = require('node:fs')
const path = require('node:path')

const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('..')

// The json grammar fixture is compiled alongside the engine in its
// dist-test dir. Resolve it relative to the engine package so the path
// stays correct regardless of where the symlinked engine lives.
// The json grammar fixture lives in the engine's published dist-test dir.
// It can be unresolvable in some CI dependency topologies; tests that need it
// skip gracefully (guarded by SKIP_JSON) rather than crashing the whole file.
let json = null
try {
  const PARSER_MAIN = require.resolve('@tabnas/parser')
  ;({ json } = require(
    path.resolve(path.dirname(PARSER_MAIN), '..', 'dist-test', 'json-plugin.js'),
  ))
} catch { /* fixture unavailable; dependent tests skip */ }
const SKIP_JSON = json ? false : 'parser dist-test fixture unavailable'

// The seven canonical section headers are shared with the Go suite via a
// single golden fixture; both suites assert their headers match it so the
// cross-runtime diffability claim is enforced.
const HEADERS_GOLDEN = path.resolve(__dirname, '..', '..', 'test', 'headers.golden')

// A console stand-in that records each log() line (its arguments joined
// with a space, matching how console.log renders them) for assertions.
function makeFakeConsole() {
  const lines = []
  return {
    lines,
    console: {
      log: (...args) => lines.push(args.join(' ')),
      dir: () => {},
      error: () => {},
    },
  }
}

// Build a minimal, grammar-free instance with a single fixed token and a
// `top` rule whose alternates carry NO group tags, so trace `parse` lines
// have no `g` field. Used to prove the absence of a spurious empty `g`.
function makeMinimal(getConsole) {
  const tn = new Tabnas(
    getConsole ? { debug: { get_console: getConsole } } : undefined,
  ).make({ fixed: { token: { Ta: 'a' } }, rule: { start: 'top' } })
  // Drop the engine's default rules so only `top` is exercised.
  const rules = tn.rule()
  Object.keys(rules).forEach((rn) => tn.rule(rn, null))
  tn.rule('top', (rs) => rs.open([{ s: ['Ta'] }]).close([{ s: '#ZZ' }]))
  return tn
}

describe('debug', () => {
  it('loads', () => {
    assert.ok(Debug != null)
  })

  it('decorates an instance with describe()', () => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    assert.equal(typeof tn.debug.describe, 'function')
    assert.equal(typeof tn.debug.describe(), 'string')

    const out = tn.debug.describe()
    for (const header of [
      '========= INSTANCE ========',
      '========= TOKENS ========',
      '========= RULES =========',
      '========= ALTS =========',
      '========= LEXER =========',
      '========= CONFIG ========',
      '========= PLUGIN =========',
      '========= ABNF =========',
    ]) {
      assert.ok(out.includes(header), 'describe() missing section ' + header)
    }
  })

  it('reports the instance tag and config in describe()', () => {
    const tn = new Tabnas({ tag: 'demo' })
    tn.use(Debug, { print: false, trace: false })
    const out = tn.debug.describe()
    assert.ok(out.includes('tag: demo'), 'describe() should report the instance tag')
    assert.ok(out.includes('  start: '), 'describe() should report the rule start')
  })

  // The eight section headers describe() emits must match, in order, the
  // shared golden fixture the Go suite also reads. This locks the two
  // runtimes to the same diffable layout.
  it('emits exactly the shared golden section headers, in order', () => {
    const golden = fs
      .readFileSync(HEADERS_GOLDEN, 'utf8')
      .split('\n')
      .filter((line) => line.length > 0)
    assert.equal(golden.length, 8, 'golden fixture should hold 8 headers')

    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    const out = tn.debug.describe()

    // Headers appear in the documented order.
    let cursor = -1
    for (const header of golden) {
      const at = out.indexOf(header, cursor + 1)
      assert.ok(at > cursor, 'header out of order or missing: ' + header)
      cursor = at
    }
  })

  describe('trace', () => {
    it('emits trace lines and no spurious empty group field on parse lines', () => {
      // The just-fixed bug appended an empty `g: ` to every parse line.
      // With a grammar whose alts carry no group tags, parse lines must
      // contain neither `g:` nor the spurious `g: ` (g, colon, space).
      const fake = makeFakeConsole()
      const tn = makeMinimal(() => fake.console)
      tn.use(Debug, { print: false, trace: true })
      tn.parse('a')

      assert.ok(
        fake.lines.some((l) => l.includes('========= TRACE')),
        'trace should emit the TRACE banner',
      )
      const parseLines = fake.lines.filter((l) => l.includes('  parse'))
      assert.ok(parseLines.length > 0, 'trace should emit parse lines')
      for (const l of parseLines) {
        assert.ok(
          !/g:/.test(l),
          'parse line must not carry a group field when alts have no g: ' +
            JSON.stringify(l),
        )
      }
      // No line anywhere should carry the spurious empty `g: ` (g + colon +
      // space with nothing following on a group-less alt).
      for (const l of fake.lines) {
        assert.ok(
          !/\bg: (\s|$)/.test(l),
          'spurious empty group field leaked: ' + JSON.stringify(l),
        )
      }
    })

    it('traces a json parse and keeps group tags on parse lines', { skip: SKIP_JSON }, () => {
      const fake = makeFakeConsole()
      const tn = new Tabnas({ debug: { get_console: () => fake.console } })
      tn.use(json)
      tn.use(Debug, { print: false, trace: true })
      const out = tn.parse('{"a":1}')
      assert.deepEqual(out, { a: 1 }, 'json parse should still succeed under trace')

      const parseLines = fake.lines.filter((l) => l.includes('  parse'))
      assert.ok(parseLines.length > 0, 'json parse should emit parse lines')
      // The json grammar tags every alt with a group, so the group field
      // must be present here (and correctly formatted as `g:tags`, never
      // the spurious empty `g: `).
      assert.ok(
        parseLines.some((l) => /g:[a-z]/.test(l)),
        'json parse lines should carry populated group tags',
      )
      for (const l of fake.lines) {
        assert.ok(
          !/\bg: (\s|$)/.test(l),
          'spurious empty group field leaked: ' + JSON.stringify(l),
        )
      }
    })

    it('honours per-kind trace flags (rule on, lex off)', () => {
      // The engine deep-merges Debug.defaults (all kinds true) with the
      // supplied options, so a partial { rule: true } cannot turn other
      // kinds off implicitly. Disable the rest explicitly to exercise the
      // LOGKIND[kind] && options.trace[kind] per-kind gate.
      const fake = makeFakeConsole()
      const tn = makeMinimal(() => fake.console)
      tn.use(Debug, {
        print: false,
        trace: {
          rule: true,
          lex: false,
          parse: false,
          node: false,
          stack: false,
          step: false,
        },
      })
      tn.parse('a')

      const ruleLines = fake.lines.filter((l) => l.includes('  rule'))
      const lexLines = fake.lines.filter((l) => l.includes('  lex'))
      assert.ok(ruleLines.length > 0, 'rule trace lines should appear')
      assert.equal(lexLines.length, 0, 'lex trace lines should be suppressed')
    })

    it('does not throw and still traces on a parse that errors', () => {
      const fake = makeFakeConsole()
      const tn = makeMinimal(() => fake.console)
      tn.use(Debug, { print: false, trace: true })

      // 'b' is not accepted by the grammar: the engine throws a parse
      // error, but the debug plugin itself must not crash, and tracing
      // must have emitted output before the failure.
      assert.throws(() => tn.parse('b'), /unexpected|b/)
      assert.ok(
        fake.lines.some((l) => l.includes('========= TRACE')),
        'tracing should emit output even when the parse errors',
      )
    })

    it('does not throw and still traces on empty source', () => {
      const fake = makeFakeConsole()
      const tn = makeMinimal(() => fake.console)
      tn.use(Debug, { print: false, trace: true })

      let result
      assert.doesNotThrow(() => {
        result = tn.parse('')
      })
      assert.equal(result, undefined, 'empty source yields no value')
      assert.ok(
        fake.lines.some((l) => l.includes('========= TRACE')),
        'tracing should emit output even on empty source',
      )
    })
  })

  describe('describe() bodies', () => {
    // A non-trivial grammar: a `top` rule that pushes to a single-character
    // rule name `x` plus alternates with group tags. Exercises TOKENS rows,
    // ALTS content, and the single-char transition edge.
    function makeTreeGrammar() {
      const tn = new Tabnas().make({
        fixed: { token: { Ta: 'a', Tx: 'x' } },
        rule: { start: 'top' },
      })
      const rules = tn.rule()
      Object.keys(rules).forEach((rn) => tn.rule(rn, null))
      tn.rule('top', (rs) =>
        rs
          .open([{ s: ['Ta'], p: 'x', g: 'topgrp' }])
          .close([{ s: '#ZZ' }]),
      )
      tn.rule('x', (rs) => rs.open([{ s: ['Tx'] }]).close([{ s: '#ZZ' }]))
      tn.use(Debug, { print: false, trace: false })
      return tn
    }

    it('lists custom tokens in the TOKENS section', () => {
      const tn = makeTreeGrammar()
      const out = tn.debug.describe()
      assert.ok(/\bTa\b/.test(out), 'TOKENS should list custom token Ta')
      assert.ok(/\bTx\b/.test(out), 'TOKENS should list custom token Tx')
      assert.ok(out.includes('"a"'), 'TOKENS should show the fixed source "a"')
    })

    it('renders ALTS bodies with token sequence and actions', () => {
      const tn = makeTreeGrammar()
      const out = tn.debug.describe()
      const altsIdx = out.indexOf('========= ALTS =========')
      const lexIdx = out.indexOf('========= LEXER =========')
      const alts = out.substring(altsIdx, lexIdx)

      assert.ok(alts.includes('top:'), 'ALTS should name the top rule')
      assert.ok(alts.includes('OPEN:'), 'ALTS should show the OPEN phase')
      assert.ok(alts.includes('CLOSE:'), 'ALTS should show the CLOSE phase')
      assert.ok(/\[Ta\]/.test(alts), 'ALTS should show the matched token sequence')
      assert.ok(/p=x\b/.test(alts), 'ALTS should show the push action p=x')
      assert.ok(/g=topgrp/.test(alts), 'ALTS should show the group tag g=topgrp')
    })

    it('keeps the single-character push target in the RULES tree (off-by-one regression)', () => {
      const tn = makeTreeGrammar()
      const out = tn.debug.describe()
      const rulesIdx = out.indexOf('========= RULES =========')
      const altsIdx = out.indexOf('========= ALTS =========')
      const rules = out.substring(rulesIdx, altsIdx)

      // The open-push edge from `top` to the single-char rule `x` must be
      // present: a previous off-by-one dropped single-character targets.
      assert.ok(
        /op:\s+x\b/.test(rules),
        'RULES tree should contain the single-char push edge op: x\n' + rules,
      )
    })
  })

  describe('print option', () => {
    it('logs USE: and the describe() dump when a later plugin is used', () => {
      const fake = makeFakeConsole()
      const tn = new Tabnas({ debug: { get_console: () => fake.console } })
      // The first use() installs the print wrapper, so it is the SECOND
      // (trivial) plugin's use() that triggers the USE: + describe() log.
      tn.use(Debug, { print: true, trace: false })
      const trivial = function myplugin(_tn, _opts) {}
      tn.use(trivial, {})

      const useLog = fake.lines.find((l) => l.startsWith('USE:'))
      assert.ok(useLog, 'print should log a USE: line')
      assert.ok(useLog.includes('myplugin'), 'USE: log should name the plugin')
      assert.ok(
        useLog.includes('========= INSTANCE ========'),
        'USE: log should embed the describe() dump',
      )
    })
  })

  describe('malformed-rules guard', () => {
    it('renders a null alternate entry as ***INVALID*** without throwing', () => {
      const tn = makeMinimal()
      tn.use(Debug, { print: false, trace: false })
      // Inject a null entry into an alt's token sequence: describe() must
      // render it as ***INVALID*** rather than dereferencing it. This is
      // the TS counterpart to the Go nil-alternate guard.
      const rs = tn.rule('top')
      rs.def.open[0].s = [null]

      let out
      assert.doesNotThrow(() => {
        out = tn.debug.describe()
      })
      assert.ok(
        out.includes('***INVALID***'),
        'describe() should render a null alternate entry as ***INVALID***',
      )
    })
  })

  describe('model() structured output', { skip: SKIP_JSON }, () => {
    function jsonModel(tag) {
      const tn = new Tabnas(tag ? { tag } : undefined)
      tn.use(json)
      tn.use(Debug, { print: false, trace: false })
      return { tn, model: tn.debug.model() }
    }

    it('returns an object with every documented section', () => {
      const { model: m } = jsonModel('demo')
      assert.equal(typeof m, 'object')
      for (const key of [
        'tag', 'tokens', 'tokenSets', 'rules', 'graph', 'lexer', 'config', 'plugins', 'abnf',
      ]) {
        assert.ok(key in m, 'model() missing section ' + key)
      }
      assert.equal(m.tag, 'demo')
      assert.equal(typeof m.abnf, 'string')
    })

    it('describes rules and alternates as structured data', () => {
      const { model: m } = jsonModel()
      assert.deepEqual(m.rules.map((r) => r.name).sort(), ['elem', 'list', 'map', 'pair', 'val'])
      const val = m.rules.find((r) => r.name === 'val')
      assert.ok(Array.isArray(val.open) && val.open.length >= 2, 'val should have several open alts')
      const toMap = val.open.find((a) => a.push === 'map')
      assert.ok(toMap, 'val should have an alt that pushes map')
      assert.ok(Array.isArray(toMap.seq) && toMap.seq.length > 0, 'alt seq carries the lookahead token(s)')
      assert.equal(typeof toMap.action, 'boolean')
      assert.ok(Array.isArray(toMap.groups))
    })

    it('exposes the rule-reference graph (push/replace edges)', () => {
      const { model: m } = jsonModel()
      const val = m.graph.find((g) => g.name === 'val')
      assert.deepEqual(val.openPush.slice().sort(), ['list', 'map'])
      assert.deepEqual(val.openReplace, [])
      const map = m.graph.find((g) => g.name === 'map')
      assert.ok(map.openPush.includes('pair'), 'map should push pair')
    })

    it('reports config and plugins structurally', () => {
      const { model: m } = jsonModel()
      assert.equal(m.config.start, 'val')
      assert.equal(typeof m.config.finish, 'boolean')
      assert.equal(typeof m.config.lex.fixed, 'boolean')
      assert.ok(m.plugins.some((p) => p.name === 'json'), 'plugins should list json')
      assert.ok(m.plugins.some((p) => p.name === 'Debug'), 'plugins should list Debug')
    })

    it('lists tokens with tin, name, and fixed literals', () => {
      const { model: m } = jsonModel()
      assert.ok(m.tokens.length > 0)
      for (const t of m.tokens) {
        assert.equal(typeof t.tin, 'number')
        assert.equal(typeof t.name, 'string')
      }
      assert.ok(
        m.tokens.some((t) => 'string' === typeof t.fixed && t.fixed.length > 0),
        'at least one token should carry a fixed literal (json punctuation)',
      )
    })

    it('the grammar portion is JSON-serialisable and round-trips', () => {
      const { model: m } = jsonModel()
      const grammar = {
        tag: m.tag, tokens: m.tokens, tokenSets: m.tokenSets,
        rules: m.rules, graph: m.graph, config: m.config, abnf: m.abnf,
      }
      const round = JSON.parse(JSON.stringify(grammar))
      assert.deepEqual(round.rules, m.rules)
      assert.equal(round.abnf, m.abnf)
    })

    it('model() and rule() agree on the rule set', () => {
      const { tn, model: m } = jsonModel()
      assert.deepEqual(m.rules.map((r) => r.name).sort(), Object.keys(tn.rule()).sort())
    })

    it('renders a null alternate entry as ***INVALID*** in the alt seq', () => {
      const tn = makeMinimal()
      tn.use(Debug, { print: false, trace: false })
      tn.rule('top').def.open[0].s = [null]
      let m
      assert.doesNotThrow(() => { m = tn.debug.model() })
      const top = m.rules.find((r) => r.name === 'top')
      assert.ok(top.open[0].seq.includes('***INVALID***'))
    })
  })
})
