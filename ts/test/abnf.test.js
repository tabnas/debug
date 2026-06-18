/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */
'use strict'

/*  abnf.test.js
 *  Round-trip test for the debug plugin's `abnf()` emitter.
 *
 *  HARD INDEPENDENCE CONSTRAINT: @tabnas/abnf must NOT be a dependency of
 *  @tabnas/debug. The emitter (src/debug.ts) reads ONLY the live engine
 *  and never imports abnf. abnf is used HERE, in the test only, and is
 *  loaded by SIBLING PATH (never via package.json) so it stays out of
 *  the dependency graph.
 *
 *  Round-trip criterion: for a sample ABNF A0,
 *    G1 = abnfConvert(A0) installed on a Tabnas instance;
 *    A1 = thatInstance.debug.abnf();
 *    G2 = abnfConvert(A1) on a fresh instance;
 *  G1 and G2 must RECOGNISE the sample inputs identically — same parse
 *  success/failure and same top `.rule` name. (ABNF has no actions, so
 *  parse-output values are out of scope.)
 */

const { describe, it } = require('node:test')
const assert = require('node:assert')
const path = require('node:path')

const { Tabnas } = require('@tabnas/parser')
const { Debug } = require('..')

// abnf, loaded by sibling PATH only — NOT a package dependency. The debug
// repo sits beside the abnf repo (`@tabnas/abnf`, in the `abnf` directory)
// in the tabnas multi-repo layout; resolve its built dist relative to this
// test file so the path stays correct regardless of where the repos are
// checked out.
const { abnfConvert } = require(
  require('path').resolve(__dirname, '..', '..', '..', 'abnf', 'ts', 'dist', 'abnf.js'),
)

// Recognise `input` with a compiled grammar `spec`. Returns a normalised
// result: { ok, rule } where `ok` reflects parse success and `rule` is
// the top node's grammar-rule tag (undefined when absent).
function recognise(spec, input) {
  try {
    const tn = new Tabnas().grammar(spec)
    const out = tn.parse(input)
    return { ok: true, rule: out && out.rule }
  } catch (e) {
    return { ok: false }
  }
}

// Assert the full round trip for one sample grammar over a set of inputs.
function assertRoundTrip(abnf0, inputs) {
  const g1 = abnfConvert(abnf0)

  const tn = new Tabnas()
  tn.use(Debug, { print: false, trace: false })
  tn.grammar(g1)
  const abnf1 = tn.debug.abnf()

  assert.strictEqual(typeof abnf1, 'string', 'abnf() returns a string')
  assert.ok(abnf1.length > 0, 'abnf() output is non-empty')

  let g2
  try {
    g2 = abnfConvert(abnf1)
  } catch (e) {
    assert.fail(
      'emitted ABNF did not re-compile:\n' + abnf1 + '\n' + e.message,
    )
  }

  for (const input of inputs) {
    const r1 = recognise(g1, input)
    const r2 = recognise(g2, input)
    assert.deepStrictEqual(
      r2,
      r1,
      'recognition mismatch for ' +
      JSON.stringify(input) +
      '\n  A0 = ' +
      JSON.stringify(abnf0) +
      '\n  A1 = ' +
      JSON.stringify(abnf1),
    )
  }
}

describe('abnf', () => {
  it('decorates an instance with abnf()', () => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    assert.strictEqual(typeof tn.debug.abnf, 'function')
  })

  it('round-trips alternation', () => {
    assertRoundTrip('greet = "hi" / "hello"', ['hi', 'hello', 'nope', ''])
  })

  it('round-trips concatenation', () => {
    assertRoundTrip('pair = "a" "b"', ['ab', 'a', 'ba', ''])
  })

  it('round-trips a rule reference', () => {
    assertRoundTrip('top = greet\ngreet = "hi"', ['hi', 'no', ''])
  })

  it('round-trips a case-sensitive literal', () => {
    assertRoundTrip('g = %s"Hi"', ['Hi', 'hi', 'HI', ''])
  })

  it('round-trips a char-range', () => {
    assertRoundTrip('g = %x30-39', ['5', '0', 'a', ''])
  })

  // Extra coverage beyond the required minimum: these all round-trip.
  it('round-trips ref-only alternation (FIRST-set peek)', () => {
    assertRoundTrip(
      'top = a / b\na = "x"\nb = "y"',
      ['x', 'y', 'z', ''],
    )
  })

  it('round-trips repetition (star and plus)', () => {
    assertRoundTrip('rep = *"a"', ['', 'a', 'aa', 'b'])
    assertRoundTrip('rep = 1*"a"', ['', 'a', 'aa', 'b'])
  })

  it('round-trips optional (group and prefix)', () => {
    assertRoundTrip('opt = ["a"]', ['', 'a', 'aa'])
    assertRoundTrip('m = ["x"] "y"', ['y', 'xy', 'x', 'xx'])
  })

  it('round-trips a grouped alternation', () => {
    assertRoundTrip('g = ("a" / "b") "c"', ['ac', 'bc', 'c'])
  })

  it('round-trips a multi-rule grammar with mixed terminals', () => {
    assertRoundTrip(
      'uri = scheme ":" path\n' +
      'scheme = "http" / "https"\n' +
      'path = "/a" / "/b"',
      ['http:/a', 'https:/b', 'ftp:/a', ':'],
    )
  })

  it('describe() includes an ABNF section', () => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    tn.grammar(abnfConvert('greet = "hi" / "hello"'))
    const desc = tn.debug.describe()
    assert.ok(desc.includes('========= ABNF ========='), 'has ABNF header')
    assert.ok(desc.includes('greet = HI / HELLO'), 'has emitted ABNF rule')
    assert.ok(/\bHI\b\s*=\s*"hi"/.test(desc), 'has token definition')
  })
})
