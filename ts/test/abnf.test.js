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

// Assert the emitted text obeys the parts of RFC 5234 that were previously
// violated silently. @tabnas/abnf is lenient about both, so re-compiling is
// NOT evidence of validity — the trailing-`/` bug survived this whole suite
// because every check downstream went through the tolerant parser.
//
//   rulename    = ALPHA *(ALPHA / DIGIT / "-")
//   alternation = concatenation *(*c-wsp "/" *c-wsp concatenation)
//
// so `_gen1_star_x` is not a legal name, and `x = A x /` is not a legal body.
function assertRfc5234Shape(abnf1) {
  for (const line of abnf1.split('\n')) {
    if ('' === line.trim() || line.trim().startsWith(';')) continue

    assert.ok(
      !/\/\s*$/.test(line),
      'dangling `/` — every `/` needs a concatenation after it:\n' +
      line + '\n--- in ---\n' + abnf1,
    )

    const head = line.match(/^([^\s=]+)\s*=/)
    if (head) {
      assert.match(
        head[1],
        /^[A-Za-z][A-Za-z0-9-]*$/,
        'rulename is not ALPHA *(ALPHA / DIGIT / "-"):\n' +
        line + '\n--- in ---\n' + abnf1,
      )
    }
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
  assertRfc5234Shape(abnf1)

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

  // `char-val = DQUOTE *(%x20-21 / %x23-7E) DQUOTE`, so a token fixed to a
  // control character cannot be quoted — it has to come back as a num-val.
  // Quoting it produced `CRLF = "<CR>"`, an unterminated char-val.
  it('round-trips a control-character literal as %x, not a quoted char-val', () => {
    // `emit` is defined further down the describe body; `it` callbacks run
    // after that body completes, so it is initialised by the time this runs.
    const out = emit('csv = row *( CR row )\nrow = "x"\nCR = %x0D')
    assert.match(out, /^CR\s+= %x0D$/m, 'control char emitted as num-val:\n' + out)
    assertRfc5234Shape(out)
    assertRoundTrip(
      'csv = row *( CR row )\nrow = "x"\nCR = %x0D',
      ['x', 'x\rx', 'x\rx\rx', '', 'y'],
    )
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

  // Repetition inside a group is what produces the deepest synthetic names
  // (`_gen2_star__gen1_group$alt0$step1`) — the shape most likely to emit
  // an illegal rulename.
  it('round-trips repetition inside a group', () => {
    assertRoundTrip(
      'list = "[" *( "," item ) "]"\nitem = "x"',
      ['[]', '[,x]', '[,x,x]', '[x]', '['],
    )
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

  // Explicit structural checks on the emitted ABNF (the round-trip tests
  // above verify recognition equivalence; these pin the exact folded shape,
  // matching the Go TestAbnfFoldsSyntheticOptional / TestAbnfKeepsRepetition
  // tests). emit() compiles A0 via abnf and returns what debug re-emits.
  const emit = (abnf0) => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    tn.grammar(abnfConvert(abnf0))
    return tn.debug.abnf()
  }

  it('folds a synthetic optional back to [ … ] (no _gen leaks)', () => {
    const out = emit('add = NR [ PL add ]\nPL = "+"')
    assert.ok(
      out.split('\n').includes('add = NR [ PL add ]'),
      'optional folded to `NR [ PL add ]`:\n' + out,
    )
    // Matches the sanitised spelling too: `_gen1_star_x` now emits as
    // `r-gen1-star-x`, so a bare /_gen/ would pass without testing anything.
    assert.ok(
      !/[_-]gen\d/.test(out),
      'no synthetic gen production leaked:\n' + out,
    )
  })

  it('keeps repetition as a production (does not fold *)', () => {
    const out = emit('rep = *PL\nPL = "+"')
    assert.ok(
      /^r-gen\d+-star-PL = /m.test(out),
      'star kept as its own production:\n' + out,
    )
    // Zero-or-more IS the empty alternative, and it is rendered as `[ … ]`.
    // NOT as a trailing `/`: RFC 5234's `alternation` requires a
    // concatenation after every `/`, so `x = PL x /` is a syntax error that
    // @tabnas/abnf happens to accept and other ABNF tools do not.
    assert.ok(
      /^r-gen\d+-star-PL = \[ PL r-gen\d+-star-PL \]$/m.test(out),
      'zero-or-more empty alternative rendered as `[ … ]`:\n' + out,
    )
    assert.ok(!/\/\s*$/m.test(out), 'no dangling `/`:\n' + out)
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
