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
  // matching the Go TestAbnfFoldsSyntheticOptional /
  // TestAbnfKeepsRepetitionProduction tests). emit() compiles A0 via abnf
  // and returns what debug re-emits.
  //
  // The second name was written a word short. A prefix grep still found
  // the Go test, which is why it survived; an exact one did not, and a
  // cross-port citation is exactly the kind a reader checks exactly.
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

  // tabnas-bnf compiles `*A` to a replace loop (tabnas/bnf#80), and the
  // emitter writes the loop back as the repetition it was compiled from:
  // the loop's helpers get no production of their own, and nothing of
  // them leaks. A grammar in the OLD push-chain shape still renders that
  // star as its own production; "abnf repeat loops" below builds both
  // shapes on the engine directly.
  it('writes a compiled repetition back as `*` and `1*`', () => {
    for (const [abnf0, want] of [
      ['rep = *PL\nPL = "+"', 'rep = *PL\n\nPL = "+"'],
      ['rep = 1*PL\nPL = "+"', 'rep = 1*PL\n\nPL = "+"'],
    ]) {
      const out = emit(abnf0)
      assert.strictEqual(out, want, 'repetition written back:\n' + out)
      assert.ok(!/[_-]gen\d|\$|-alt\d/.test(out), 'no loop helper leaked:\n' + out)
      assertRfc5234Shape(out)
    }
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

// Two shapes the ABNF compiler does not produce, so they are built directly
// on the engine. Both were raised in review on PR #21 and both emitted
// invalid ABNF.
describe('abnf edge shapes', () => {
  const emitOf = (build) => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    build(tn)
    return tn.debug.abnf()
  }

  // `option = "[" *c-wsp alternation *c-wsp "]"`, and `alternation` needs at
  // least one concatenation — so `[ ]` is not a legal option. A rule whose
  // only open alternative is empty but which HAS a close continuation used
  // to emit `x = [  ] Y`.
  it('emits the continuation alone, not an empty option', () => {
    const out = emitOf((tn) => {
      tn.options({ fixed: { token: { '#XA': 'a', '#XB': 'b' } }, rule: { start: 'top' } })
      tn.token('#XA')
      tn.token('#XB')
      tn.rule('top', (rs) => rs
        .open([{ p: 'inner' }])
        .close([{ s: '#ZZ' }]))
      // inner: an empty open alternative, plus a close continuation.
      tn.rule('inner', (rs) => rs
        .open([{}])
        .close([{ s: '#XA' }, { s: '#ZZ' }]))
    })

    assert.ok(!/\[\s*\]/.test(out), 'no empty `[ ]` option:\n' + out)
    assertRfc5234Shape(out)
  })

  // RFC 5234 §2.1: rule names are case-insensitive, so `Foo-Bar` and
  // `foo-bar` are ONE rule. Sanitising `foo_bar` beside a reserved
  // `Foo-Bar` used to emit two definitions of the same rule.
  it('resolves rulename collisions case-insensitively', () => {
    const out = emitOf((tn) => {
      tn.options({ fixed: { token: { '#XA': 'a', '#XB': 'b' } }, rule: { start: 'top' } })
      tn.token('#XA')
      tn.token('#XB')
      tn.rule('top', (rs) => rs
        .open([{ s: '#XA', p: 'Foo-Bar' }, { s: '#XB', p: 'foo_bar' }])
        .close([{ s: '#ZZ' }]))
      tn.rule('Foo-Bar', (rs) => rs.open([{ s: '#XA' }]).close([{}]))
      tn.rule('foo_bar', (rs) => rs.open([{ s: '#XB' }]).close([{}]))
    })

    const heads = out
      .split('\n')
      .map((l) => (l.match(/^([^\s=]+)\s*=/) || [])[1])
      .filter(Boolean)
      .map((n) => n.toLowerCase())

    assert.strictEqual(
      new Set(heads).size,
      heads.length,
      'two productions define the same rule (case-insensitively):\n' + out,
    )
    assertRfc5234Shape(out)
  })
})

// The repeat loop, as tabnas-bnf compiles every `*A` since tabnas/bnf#80:
//
//   H             open   { c: {'n.rep': 0}, n: {rep: 1}, r: H }   entry: allocate, count
//                        { s: FIRST(A), b: 1, r: H$alt0 } …       continue (a ref item)
//                        { s: A, r: H }                           continue (a terminal item)
//                        { s: FOLLOW(H), b: 1 }  { }              exits
//   H$alt0        open   { p: A, n: {rep: 0} }                    push the item
//                 close  { r: H$alt0$step1, n: {rep: 1} }         capture it
//   H$alt0$step1  open   { r: H }                                 back to the loop
//
// built on the engine directly, the way the Rust port's tests build it
// (rs/tests/abnf_test.rs), and pinned to the same text, so the rendering is
// tested without the compiler and both runtimes are held to one answer.
// The tests mirror the Rust set one for one, under its names
// (`abnf_renders_a_terminal_loop_as_a_star` is "renders a terminal loop as
// a star"), except the old-shape star, which is Rust's
// `abnf_keeps_a_repetition_production`, and the last, on reading the
// entry's guard from a compiled closure, which only this engine needs. The
// two sets move together, and become shared `test/spec` fixtures once the
// Go port renders the loop too.
describe('abnf repeat loops', () => {
  const emitOf = (fixed, build) => {
    const tn = new Tabnas()
    tn.use(Debug, { print: false, trace: false })
    tn.options({ fixed: { token: fixed } })
    Object.keys(fixed).forEach((name) => tn.token(name))
    build(tn)
    return tn.debug.abnf()
  }

  // The text a test pins, and the two RFC 5234 shapes.
  const assertAbnf = (out, want) => {
    assert.strictEqual(out, want, 'abnf mismatch:\n--- got ---\n' + out + '\n--- want ---\n' + want)
    assertRfc5234Shape(out)
  }

  // A rendered loop also leaks no synthetic: neither `_gen` / `r-gen` nor
  // an iteration helper's `$alt` / `-alt` may reach the output.
  const assertLoopAbnf = (out, want) => {
    assertAbnf(out, want)
    for (const leak of ['_gen', 'r-gen', '$', '-alt', 'step1']) {
      assert.ok(!out.includes(leak), 'a synthetic ' + leak + ' leaked:\n' + out)
    }
  }

  // The `__start__` wrapper the compiler puts round every grammar. The
  // emitter sees through it and leads with the real start.
  const wrapStart = (tn, start) => {
    tn.options({ rule: { start: '__start__' } })
    tn.rule('__start__', (rs) => rs.open([{ p: start }]).close([{ s: '#ZZ' }]))
  }

  // The entry consumes nothing and replaces the rule with itself, guarded
  // by the counter it bumps. The exits peek FOLLOW, then take anything.
  const loopEntry = (name) => ({ c: { 'n.rep': 0 }, n: { rep: 1 }, r: name })
  const loopExits = (follow) => [{ s: follow, b: 1 }, {}]

  // `*A` over a terminal item: the continue takes the token and re-enters.
  const terminalLoop = (tn, name, item, follow) =>
    tn.rule(name, (rs) => rs.open([loopEntry(name), { s: item, r: name }, ...loopExits(follow)]))

  // `*A` over a rule item: one continue per FIRST token of the item.
  const refLoop = (tn, name, firsts, item, follow) => {
    const iteration = name + '$alt0'
    const step = iteration + '$step1'
    tn.rule(name, (rs) => rs.open([
      loopEntry(name),
      ...firsts.map((first) => ({ s: first, b: 1, r: iteration })),
      ...loopExits(follow),
    ]))
    tn.rule(iteration, (rs) => rs
      .open([{ p: item, n: { rep: 0 } }])
      .close([{ r: step, n: { rep: 1 } }]))
    tn.rule(step, (rs) => rs.open([{ r: name }]))
  }

  // A rule of one open alternative and, optionally, one close.
  const simpleRule = (tn, name, open, close) =>
    tn.rule(name, (rs) => (close ? rs.open([open]).close([close]) : rs.open([open])))

  // `rep = *"a"`: a terminal item. This is THE defect shape: with the
  // entry counted as an alternative the loop rendered as
  // `r-gen1-star-term = [ r-gen1-star-term / A ]`, a production that
  // names itself and, recompiled, accepts less than the original.
  it('renders a terminal loop as a star', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      simpleRule(tn, 'rep', { p: '_gen1_star_term' }, {})
      terminalLoop(tn, '_gen1_star_term', '#A', '#ZZ')
      wrapStart(tn, 'rep')
    })
    assertLoopAbnf(out, 'rep = *A\n\nA = %s"a"')
  })

  // `rep = 1*"a"`: the `_plus` helper is `A` followed by the star of `A`.
  // With the star a loop the helper folds, and is written back as the
  // `1*A` it was compiled from — not the `A *A` it is element by element.
  // The two recognise the same language, but the abnf compiler does not
  // compile them to the same recogniser: where the item is nullable
  // (`1*( [ "+" "e" ] )`) or its FIRST meets its FOLLOW (`1*item` with
  // `item = "]" "e" / [ "d" ]`), the recompiled `A *A` rejected `+e` and
  // `]e`, which the original accepts. This pinned `rep = A *A` until that
  // round trip failed. (With an old-shape star the helper stays a
  // production, as it always did: see "keeps an old-shape plus
  // production".)
  it('renders a plus over a loop as one or more', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      simpleRule(tn, 'rep', { p: '_gen1_plus_term' }, {})
      terminalLoop(tn, '_gen1_star_term', '#A', '#ZZ')
      simpleRule(tn, '_gen1_plus_term', { s: '#A', p: '_gen1_star_term' }, {})
      wrapStart(tn, 'rep')
    })
    assertLoopAbnf(out, 'rep = 1*A\n\nA = %s"a"')
  })

  // `doc = *item` with `item = "x" "y"`: a rule item, so the loop goes
  // through the iteration helpers. `item` keeps its production; `H$alt0`
  // and `H$alt0$step1` do not get one, and their back edges render
  // nothing. The helpers are installed BEFORE the loop, as the compiler
  // installs them, so a production order that merely followed insertion
  // would have leaked them first.
  it('renders a ref loop as a star of the rule', () => {
    const out = emitOf({ '#X': 'x', '#Y': 'y' }, (tn) => {
      simpleRule(tn, 'doc', { p: '_gen1_star_item' }, {})
      simpleRule(tn, 'item', { s: ['#X', '#Y'] })
      refLoop(tn, '_gen1_star_item', ['#X'], 'item', '#ZZ')
      wrapStart(tn, 'doc')
    })
    assertLoopAbnf(out, 'doc = *item\nitem = X Y\n\nX = %s"x"\nY = %s"y"')
  })

  // `list = "[" *( "," item ) "]"` with `item = "x"`: a loop inside a
  // sequence, over a group. The group folds into the iteration, the
  // loop's FOLLOW is the closing bracket (peeked by the exit, never
  // rendered), and the `"]"` comes from `list`'s own chain step.
  it('renders a group loop inside a sequence', () => {
    const out = emitOf({ '#T': '[', '#T1': ']', '#T2': ',', '#X': 'x' }, (tn) => {
      simpleRule(tn, 'list', { s: '#T', p: '_gen2_star__gen1_group' }, { r: 'list$step1' })
      simpleRule(tn, 'list$step1', { s: '#T1' })
      simpleRule(tn, '_gen1_group', { s: '#T2', p: 'item' })
      refLoop(tn, '_gen2_star__gen1_group', ['#T2'], '_gen1_group', '#T1')
      simpleRule(tn, 'item', { s: '#X' })
      wrapStart(tn, 'list')
    })
    assertLoopAbnf(out, [
      'list = T *( T2 item ) T1',
      'item = X',
      '',
      'T  = "["',
      'T2 = ","',
      'T1 = "]"',
      'X  = %s"x"',
    ].join('\n'))
  })

  // `s = *( "a" / "b" ) ";"`: a loop over a two-way group has one
  // continue per FIRST token, both replacing with the same iteration. The
  // iteration renders once, as the parenthesised alternation, and the
  // repetition wraps it exactly once: `*( A / B )`, not `*( ( A / B ) )`.
  it('renders a loop over alternatives once', () => {
    const out = emitOf({ '#T': ';', '#A': 'a', '#B': 'b' }, (tn) => {
      simpleRule(tn, 's', { p: '_gen2_star__gen1_group' }, { r: 's$step1' })
      simpleRule(tn, 's$step1', { s: '#T' })
      tn.rule('_gen1_group', (rs) => rs.open([{ s: '#A' }, { s: '#B' }]))
      refLoop(tn, '_gen2_star__gen1_group', ['#A', '#B'], '_gen1_group', '#T')
      wrapStart(tn, 's')
    })
    assertLoopAbnf(out, 's = *( A / B ) T\n\nA = %s"a"\nB = %s"b"\nT = ";"')
  })

  // `outer = *( "<" *"i" ">" )`: a loop nested in a loop's iteration. The
  // inner loop is a loop of its own, rendered as `*I` inside the outer
  // iteration; the outer's exit peeks `#ZZ`, the inner's peeks `">"`.
  it('renders a loop nested in a loop', () => {
    const out = emitOf({ '#I': 'i', '#T1': '>', '#T': '<' }, (tn) => {
      simpleRule(tn, 'outer', { p: '_gen3_star__gen2_group' }, {})
      terminalLoop(tn, '_gen1_star_term', '#I', '#T1')
      simpleRule(tn, '_gen2_group', { s: '#T', p: '_gen1_star_term' }, { r: '_gen2_group$step1' })
      simpleRule(tn, '_gen2_group$step1', { s: '#T1' })
      refLoop(tn, '_gen3_star__gen2_group', ['#T'], '_gen2_group', '#ZZ')
      wrapStart(tn, 'outer')
    })
    assertLoopAbnf(out, 'outer = *( T *I T1 )\n\nT  = "<"\nI  = %s"i"\nT1 = ">"')
  })

  // `rep = 2*"a"`: a counted repetition compiles to a `_rep` helper, the
  // item `n` times then the star of the item, in one alternative for a
  // terminal item. It is written back as `2*A` for the reason `1*A` is.
  it('renders a counted repetition over a loop with its count', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      simpleRule(tn, 'rep', { p: '_gen1_rep_term' }, {})
      terminalLoop(tn, '_gen1_star_term', '#A', '#ZZ')
      simpleRule(tn, '_gen1_rep_term', { s: ['#A', '#A'], p: '_gen1_star_term' }, {})
      wrapStart(tn, 'rep')
    })
    assertLoopAbnf(out, 'rep = 2*A\n\nA = %s"a"')
  })

  // `n = 2*4"z"`: a bounded repetition compiles to a `_rep` helper too,
  // but one that ends in nested optionals rather than a loop. Its body is
  // not an item then a repetition, so it renders as it is; the count
  // rewrite must not reach for a `_rep` name alone.
  it('leaves a bounded repetition as its optionals', () => {
    const out = emitOf({ '#Z': 'z' }, (tn) => {
      simpleRule(tn, 'n', { p: '_gen1_rep_term' }, {})
      simpleRule(tn, '_gen1_group', { s: '#Z' })
      tn.rule('_gen2_opt__gen1_group', (rs) => rs
        .open([{ s: '#Z', b: 1, p: '_gen1_group' }, { s: '#ZZ', b: 1 }, {}])
        .close([{}]))
      simpleRule(tn, '_gen3_group', { s: '#Z', p: '_gen2_opt__gen1_group' }, {})
      tn.rule('_gen4_opt__gen3_group', (rs) => rs
        .open([{ s: '#Z', b: 1, p: '_gen3_group' }, { s: '#ZZ', b: 1 }, {}])
        .close([{}]))
      simpleRule(tn, '_gen1_rep_term', { s: ['#Z', '#Z'], p: '_gen4_opt__gen3_group' }, {})
      wrapStart(tn, 'n')
    })
    assertLoopAbnf(out, 'n = Z Z [ Z [ Z ] ]\n\nZ = %s"z"')
  })

  // `top = *[ "," ]`: a loop over an optional. The star's helper is named
  // after its item, `_gen3_star__gen2_opt__gen1_group`, and so are its
  // iteration helpers, and deciding the `[ … ]` wrap by a substring test
  // for `_opt` reached all three: the option was wrapped again, and the
  // step, whose only content is the back edge, became the empty option in
  // `*[ [ T ] [  ] ]`. RFC 5234 has no room for it: an option holds an
  // alternation, and an alternation at least one concatenation. Read from
  // the rule's own segment, only the optional's helper is an optional.
  // (The optional's exits peek the item's own token as well as the end:
  // inside a loop, the item is its own FOLLOW.)
  it('renders a loop over an optional without an empty option', () => {
    const out = emitOf({ '#T': ',' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_star__gen2_opt__gen1_group' }, {})
      simpleRule(tn, '_gen1_group', { s: '#T' })
      tn.rule('_gen2_opt__gen1_group', (rs) => rs
        .open([{ s: '#T', b: 1, p: '_gen1_group' }, { s: '#T', b: 1 }, { s: '#ZZ', b: 1 }, {}])
        .close([{}]))
      refLoop(tn, '_gen3_star__gen2_opt__gen1_group', ['#T'], '_gen2_opt__gen1_group', '#ZZ')
      wrapStart(tn, 'top')
    })
    assertLoopAbnf(out, 'top = *[ T ]\n\nT = ","')
  })

  // `top = 1*[ "a" ]`: the `_plus` helper over an optional is named after
  // it too (`_gen3_plus__gen2_opt__gen1_group`), as is its chain step. The
  // same substring test rendered `[ [ A ] [ *[ [ A ] [  ] ] ] ]`; the
  // helper is a plus, its step a step, and the whole is the `1*[ A ]` it
  // came from.
  it('renders a plus over an optional loop as one or more', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_plus__gen2_opt__gen1_group' }, {})
      simpleRule(tn, '_gen1_group', { s: '#A' })
      tn.rule('_gen2_opt__gen1_group', (rs) => rs
        .open([{ s: '#A', b: 1, p: '_gen1_group' }, { s: '#A', b: 1 }, { s: '#ZZ', b: 1 }, {}])
        .close([{}]))
      refLoop(tn, '_gen3_star__gen2_opt__gen1_group', ['#A'], '_gen2_opt__gen1_group', '#ZZ')
      simpleRule(tn, '_gen3_plus__gen2_opt__gen1_group', { p: '_gen2_opt__gen1_group' },
        { r: '_gen3_plus__gen2_opt__gen1_group$step1' })
      simpleRule(tn, '_gen3_plus__gen2_opt__gen1_group$step1',
        { p: '_gen3_star__gen2_opt__gen1_group' }, {})
      wrapStart(tn, 'top')
    })
    assertLoopAbnf(out, 'top = 1*[ A ]\n\nA = %s"a"')
  })

  // `top = 1*( "a" "b" )` and `top = 1*( "a" / "b" )`: a plus over a
  // group. The loop writes the sequence as `*( A B )` and the alternation,
  // already one parenthesised element, as `*( A / B )`; the plus is the
  // item as the loop wrote it, once, then the loop, in either spelling,
  // and comes back as `1*( A B )` and `1*( A / B )`.
  it('renders a plus over a group loop as one or more', () => {
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        simpleRule(tn, 'top', { p: '_gen2_plus__gen1_group' }, {})
        simpleRule(tn, '_gen1_group', { s: ['#A', '#B'] })
        refLoop(tn, '_gen2_star__gen1_group', ['#A'], '_gen1_group', '#ZZ')
        simpleRule(tn, '_gen2_plus__gen1_group', { p: '_gen1_group' },
          { r: '_gen2_plus__gen1_group$step1' })
        simpleRule(tn, '_gen2_plus__gen1_group$step1', { p: '_gen2_star__gen1_group' }, {})
        wrapStart(tn, 'top')
      })
      assertLoopAbnf(out, 'top = 1*( A B )\n\nA = %s"a"\nB = %s"b"')
    }
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        simpleRule(tn, 'top', { p: '_gen2_plus__gen1_group' }, {})
        tn.rule('_gen1_group', (rs) => rs.open([{ s: '#A' }, { s: '#B' }]))
        refLoop(tn, '_gen2_star__gen1_group', ['#A', '#B'], '_gen1_group', '#ZZ')
        simpleRule(tn, '_gen2_plus__gen1_group', { p: '_gen1_group' },
          { r: '_gen2_plus__gen1_group$step1' })
        simpleRule(tn, '_gen2_plus__gen1_group$step1', { p: '_gen2_star__gen1_group' }, {})
        wrapStart(tn, 'top')
      })
      assertLoopAbnf(out, 'top = 1*( A / B )\n\nA = %s"a"\nB = %s"b"')
    }
  })

  // The OLD shape, each item pushing the rule again, renders as it always
  // did: the star is its own production, its empty alternative written
  // `[ … ]` (never a trailing `/`, which RFC 5234's `alternation` forbids),
  // its name sanitised to a legal rulename.
  it('keeps an old-shape star as a production', () => {
    const out = emitOf({ '#T': '+' }, (tn) => {
      tn.options({ rule: { start: 'rep' } })
      simpleRule(tn, 'rep', { p: '_gen1_star_T' }, {})
      // The empty alternative is what makes it zero-or-more.
      tn.rule('_gen1_star_T', (rs) => rs.open([{ s: '#T' }, {}]).close([{}]))
    })
    assertAbnf(out, 'rep = r-gen1-star-T\nr-gen1-star-T = [ T ]\n\nT = "+"')
  })

  // A grammar in the OLD shape renders exactly as before
  // (`docs/reference.md`, "The repeat loop", point 5). `1*"a"` compiled
  // to a `_plus` helper pushing a push-chain star; the star is a kept
  // production, so the helper stays one too, and the output is what the
  // emitter gave before the loop shape existed: the productions in
  // installation order (the compiler installs the star before the plus),
  // the star's empty alternative as `[ … ]`.
  it('keeps an old-shape plus production', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      tn.options({ rule: { start: 'rep' } })
      simpleRule(tn, 'rep', { p: '_gen1_plus_A' }, {})
      tn.rule('_gen1_star_A', (rs) => rs.open([{ s: '#A', p: '_gen1_star_A' }, {}]).close([{}]))
      simpleRule(tn, '_gen1_plus_A', { s: '#A', p: '_gen1_star_A' }, {})
    })
    assertAbnf(out, [
      'rep = r-gen1-plus-A',
      'r-gen1-star-A = [ A r-gen1-star-A ]',
      'r-gen1-plus-A = A r-gen1-star-A',
      '',
      'A = %s"a"',
    ].join('\n'))
  })

  // A user rule with a non-consuming self-replace open alternative that is
  // NOT a loop entry: a guarded or counted state transition, such as
  // `{ c: [n.mode == 0], n: {mode: 1}, r: st }`. Its `s`, `b`, `p` and
  // `r` are the entry's, and it is no repetition: the entry's own guard
  // `n.rep == 0` and its counter set to 1 are part of the shape, and each
  // is tried without the other here too. Read as a loop, the whole rule
  // was rewritten as `st = *( A / B )`, accepting the empty input and any
  // number of items where the original takes one. It renders as the
  // emitter always rendered it, a reference to the rule among its
  // alternatives, `st = st / A / B`, and never as `*…`.
  it('does not read an unguarded self-replace as a loop', () => {
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        tn.options({ rule: { start: 'st' } })
        tn.rule('st', (rs) => rs
          .open([{ c: { 'n.mode': 0 }, r: 'st', n: { mode: 1 } }, { s: '#A' }, { s: '#B' }])
          .close([{}]))
      })
      assertLoopAbnf(out, 'st = st / A / B\n\nA = %s"a"\nB = %s"b"')
    }
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        tn.options({ rule: { start: 'st' } })
        tn.rule('st', (rs) => rs.open([{ r: 'st' }, { s: '#A' }, { s: '#B' }]).close([{}]))
      })
      assertLoopAbnf(out, 'st = st / A / B\n\nA = %s"a"\nB = %s"b"')
    }
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        tn.options({ rule: { start: 'st' } })
        tn.rule('st', (rs) => rs
          .open([{ c: { 'n.rep': 0 }, r: 'st' }, { s: '#A' }, { s: '#B' }])
          .close([{}]))
      })
      assertLoopAbnf(out, 'st = st / A / B\n\nA = %s"a"\nB = %s"b"')
    }
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        tn.options({ rule: { start: 'st' } })
        tn.rule('st', (rs) => rs
          .open([{ r: 'st', n: { rep: 1 } }, { s: '#A' }, { s: '#B' }])
          .close([{}]))
      })
      assertLoopAbnf(out, 'st = st / A / B\n\nA = %s"a"\nB = %s"b"')
    }
  })

  // `one = A [ one ]`, hand-built as a guarded close continuation: the
  // open consumes `A`, and the closes are `{ s: A, b: 1, r: one }`, which
  // peeks the next `A` and re-enters the rule, and `{ }`. The continuation
  // replaces with the rule being rendered and is not its loop entry (a
  // close, and unguarded), so it keeps its content. Calling it empty for
  // the self-replace alone skipped it and emitted `one = A`: exactly one
  // where the rule takes one or more. It renders as it always has.
  it('keeps a guarded self-replacing close continuation', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      tn.options({ rule: { start: 'one' } })
      tn.rule('one', (rs) => rs.open([{ s: '#A' }]).close([{ s: '#A', b: 1, r: 'one' }, {}]))
    })
    assertLoopAbnf(out, 'one = A [ one ]\n\nA = %s"a"')
  })

  // The entry alone does not make a loop: the whole scaffold does, the
  // entry first, continues that each come back to the rule having taken
  // something, and an exit that shadows none of them. Each rule
  // here has the entry and not the rest, and none of them repeats
  // anything:
  //
  // - the continue `{ s: A }` never comes back, so with the exit the rule
  //   takes one `A` or nothing;
  // - `{ s: A, r: once }` comes back and `{ s: B }` does not, so a `B`
  //   ends the rule;
  // - the continue comes back and there is no exit, so the rule never
  //   stops;
  // - the continue pushes as well as replacing, and the engine takes the
  //   push and not the replace;
  // - the continue only peeks and comes back, taking nothing;
  // - the empty exit `{ }` comes before the continue, which it shadows;
  // - the entry comes after a continue, not first as the compiler puts
  //   it;
  // - an empty exit with a condition comes before the continue, which it
  //   shadows whenever the condition holds;
  // - a function decides the entry's route;
  // - a second entry follows the continue, with an exit after it or
  //   none: the compiler writes one, and a second is neither a continue
  //   nor an exit;
  // - the only exit carries a condition, which may never hold, and the
  //   compiler never guards an exit;
  // - the only exit is a FOLLOW peek, with no empty exit, so the rule
  //   stops only where that token comes next;
  // - the entry carries a further condition, which may keep it from
  //   setting the counter the continue is guarded on;
  // - the entry peeks a token, and so sets the counter only where that
  //   token comes next;
  // - a continue is guarded on `n.rep == 0`, which the entry has already
  //   left behind, and a continue may carry no guard but the compiler's
  //   suffix-debt counter;
  // - a FOLLOW peek before the only continue covers it, so it never runs;
  // - a peek covers one continue, whose item no live continue takes, so
  //   rendering it would offer an alternative the rule never takes;
  // - a continue consumes an empty token slot, which takes any token and
  //   renders as nothing;
  // - the entry sets a further counter, which turns off a continue
  //   guarded on it;
  // - a function decides the continue's route.
  //
  // Read as a loop on its entry alone, all but one rendered as `*A`, which
  // takes any number, and the one that takes nothing as `once = `, which
  // RFC 5234 has no room for. Each renders as it always has, the entry a
  // reference to the rule among its alternatives, with no repetition. The
  // whole scaffold, last, still reads as the loop it is.
  it('does not read an entry without its scaffold as a loop', () => {
    // The rule `once` of the given open alternatives, over `A` and `B`. The
    // engine normalises an alternative in place, so each case builds its
    // own.
    const onceOf = (opens) => emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      tn.options({ rule: { start: 'once' } })
      tn.rule('once', (rs) => rs.open(opens()))
    })
    const entry = () => loopEntry('once')
    const back = (tin) => ({ s: tin, r: 'once' })
    const more = 'once = [ once / A once ]\n\nA = %s"a"'
    const cases = [
      ['a continue that never comes back',
        () => [entry(), { s: '#A' }, {}], 'once = [ once / A ]\n\nA = %s"a"'],
      ['one continue of two that does not come back',
        () => [entry(), back('#A'), { s: '#B' }, {}],
        'once = [ once / A once / B ]\n\nA = %s"a"\nB = %s"b"'],
      ['no exit',
        () => [entry(), back('#A')], 'once = once / A once\n\nA = %s"a"'],
      ['a continue that pushes as well as replacing, which the engine takes as a push',
        () => [entry(), { ...back('#A'), p: 'once' }, {}], more],
      ['a continue that only peeks and comes back, taking nothing',
        () => [entry(), { ...back('#A'), b: 1 }, {}], 'once = [ once ]'],
      ['the empty exit before the continue, which it shadows',
        () => [entry(), {}, back('#A')], more],
      ['the entry after a continue',
        () => [back('#A'), entry(), {}], 'once = [ A once / once ]\n\nA = %s"a"'],
      ['an empty exit with a condition before the continue, '
        + 'which shadows it when the condition holds',
        () => [entry(), { c: { 'n.x': 0 } }, back('#A'), {}], more],
      // A modifier may change the route an alternative names: TypeScript's
      // spelling of the Rust `r_fn` beside a static `r`.
      ['an entry whose route a function decides',
        () => [{ ...entry(), h: (rule, ctx, next) => next }, back('#A'), {}], more],
      ['a second entry after the continue, with no exit',
        () => [entry(), back('#A'), entry()], 'once = once / A once\n\nA = %s"a"'],
      ['a second entry after the continue, before the exit',
        () => [entry(), back('#A'), entry(), {}], more],
      ['an exit under a condition that may never hold',
        () => [entry(), back('#A'), { c: { 'n.never': 1 } }], more],
      ['a FOLLOW peek and no empty exit',
        () => [entry(), back('#A'), { s: '#B', b: 1 }], more],
      ['an entry under a further condition, before a continue guarded on the counter it sets',
        () => [
          { c: { 'n.rep': 0, 'n.never': 1 }, n: { rep: 1 }, r: 'once' },
          { c: { 'n.rep': 1 }, ...back('#A') },
          {},
        ], more],
      ['an entry that peeks a token, before a continue guarded on the counter it sets',
        () => [{ ...entry(), s: '#B', b: 1 }, { c: { 'n.rep': 1 }, ...back('#A') }, {}], more],
      ['a continue guarded on the state the entry has already left',
        () => [entry(), { c: { 'n.rep': 0 }, ...back('#A') }, {}], more],
      ['a FOLLOW peek before the only continue, covering it',
        () => [entry(), { s: '#A', b: 1 }, back('#A'), {}], more],
      ['an entry that sets a further counter a continue is guarded on',
        () => [
          { ...entry(), n: { debt_x: 1, rep: 1 } },
          { c: { 'n.debt_x': 0 }, ...back('#A') },
          {},
        ], more],
      ['a dead continue with an item no live continue takes',
        () => [entry(), { s: '#A', b: 1 }, back('#A'), back('#B'), {}],
        'once = [ once / A once / B once ]\n\nA = %s"a"\nB = %s"b"'],
      ['a continue that consumes an empty token slot',
        () => [entry(), { s: [[]], r: 'once' }, {}], 'once = [ once ]'],
      ['a continue whose route a function decides',
        () => [entry(), { ...back('#A'), h: (rule, ctx, next) => next }, {}], more],
      // TypeScript's other spelling: the route itself a function.
      ['an entry whose route is a function',
        () => [{ ...entry(), r: () => 'once' }, back('#A'), {}], 'once = [ A once ]\n\nA = %s"a"'],
      ['a continue whose route is a function',
        () => [entry(), { s: '#A', r: () => 'once' }, {}], 'once = [ once / A ]\n\nA = %s"a"'],
    ]
    for (const [what, opens, want] of cases) {
      const out = onceOf(opens)
      assert.strictEqual(out, want, what + ':\n' + out)
      assert.ok(!out.includes('*'), what + ', read as a repetition:\n' + out)
      assertRfc5234Shape(out)
    }
    assertLoopAbnf(onceOf(() => [entry(), back('#A'), {}]), 'once = *A\n\nA = %s"a"')
  })

  // A rule item's loop comes back through its helpers: the continue
  // replaces with `H$alt0`, which pushes the item and on close replaces
  // with `H$alt0$step1`, which replaces with `H`. With the step replacing
  // with nothing, the iteration ends after one item, and the rule is no
  // loop. It renders as it always has, with no repetition.
  it('does not read a loop whose helpers never come back as one', () => {
    const out = emitOf({ '#X': 'x' }, (tn) => {
      simpleRule(tn, 'doc', { p: '_gen1_star_item' }, {})
      simpleRule(tn, 'item', { s: '#X' })
      tn.rule('_gen1_star_item', (rs) => rs
        .open([
          loopEntry('_gen1_star_item'),
          { s: '#X', b: 1, r: '_gen1_star_item$alt0' },
          { s: '#ZZ', b: 1 },
          {},
        ]))
      simpleRule(tn, '_gen1_star_item$alt0', { p: 'item', n: { rep: 0 } },
        { r: '_gen1_star_item$alt0$step1', n: { rep: 1 } })
      simpleRule(tn, '_gen1_star_item$alt0$step1', {})
      wrapStart(tn, 'doc')
    })
    assertAbnf(out, [
      'doc = r-gen1-star-item',
      'item = X',
      'r-gen1-star-item = [ r-gen1-star-item / r-gen1-star-item-alt0 ]',
      'r-gen1-star-item-alt0 = item r-gen1-star-item-alt0-step1',
      'r-gen1-star-item-alt0-step1 = ""',
      '',
      'X = %s"x"',
    ].join('\n'))
  })

  // A rule item's loop whose way back is guarded is no loop either: the
  // step replaces with the loop only under a condition, `n.never == 1`,
  // which nothing sets, so the iteration can end after one item. The
  // compiler puts conditions on a loop's own continues, never on its
  // helpers; a helper's way back is read only when it is taken whatever
  // the state. It renders as it always has, with no repetition.
  it('does not read a loop whose way back is guarded as one', () => {
    const out = emitOf({ '#X': 'x' }, (tn) => {
      simpleRule(tn, 'doc', { p: '_gen1_star_item' }, {})
      simpleRule(tn, 'item', { s: '#X' })
      tn.rule('_gen1_star_item', (rs) => rs
        .open([
          loopEntry('_gen1_star_item'),
          { s: '#X', b: 1, r: '_gen1_star_item$alt0' },
          { s: '#ZZ', b: 1 },
          {},
        ]))
      simpleRule(tn, '_gen1_star_item$alt0', { p: 'item', n: { rep: 0 } },
        { r: '_gen1_star_item$alt0$step1', n: { rep: 1 } })
      simpleRule(tn, '_gen1_star_item$alt0$step1', { c: { 'n.never': 1 }, r: '_gen1_star_item' })
      wrapStart(tn, 'doc')
    })
    assertAbnf(out, [
      'doc = r-gen1-star-item',
      'item = X',
      'r-gen1-star-item = [ r-gen1-star-item / r-gen1-star-item-alt0 ]',
      'r-gen1-star-item-alt0 = item r-gen1-star-item-alt0-step1',
      'r-gen1-star-item-alt0-step1 = r-gen1-star-item',
      '',
      'X = %s"x"',
    ].join('\n'))
  })

  // A `_plus` helper that repeats by a cycle of its own, not through a
  // loop, keeps its production: here it re-enters itself on `A` and exits
  // on anything else, which takes any number of `A`. Folded, as a helper
  // that reaches no kept repetition was, its back edge rendered as
  // nothing and the rule came out as one `A`. It renders as it always
  // has.
  it('keeps a plus helper that repeats by its own cycle', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      simpleRule(tn, 'rep', { p: '_gen1_plus_term' }, {})
      tn.rule('_gen1_plus_term', (rs) => rs.open([{ s: '#A', r: '_gen1_plus_term' }, {}]))
      wrapStart(tn, 'rep')
    })
    assertAbnf(out, [
      'rep = r-gen1-plus-term',
      'r-gen1-plus-term = [ A r-gen1-plus-term ]',
      '',
      'A = %s"a"',
    ].join('\n'))
  })

  // A grammar whose start rule is a synthetic loop, named directly or
  // through the `__start__` wrapper. A synthetic loop is rendered where
  // it is referenced, and nothing references the start: it came out with
  // no production at all. The start is always a production.
  it('keeps a synthetic loop that is the start as a production', () => {
    for (const wrapped of [false, true]) {
      const out = emitOf({ '#A': 'a' }, (tn) => {
        terminalLoop(tn, '_gen1_star_A', '#A', '#ZZ')
        if (wrapped) wrapStart(tn, '_gen1_star_A')
        else tn.options({ rule: { start: '_gen1_star_A' } })
      })
      assert.strictEqual(out, 'r-gen1-star-A = *A\n\nA = %s"a"', 'wrapped: ' + wrapped)
      assertRfc5234Shape(out)
    }
  })

  // A synthetic rule with the loop's whole open scaffold whose close
  // replaces with a synthetic helper, and that helper's route a function
  // decides: it may come back to the rule by a way the spec does not
  // show. The rule is no loop, and renders as it always has.
  it('does not read a synthetic loop whose close helper is dynamic as one', () => {
    const out = emitOf({ '#B': 'b', '#A': 'a' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen1_star_A' }, {})
      // The function here comes back to the rule; nothing in the spec says so.
      simpleRule(tn, '_gen2_group', { s: '#B', r: () => '_gen1_star_A' })
      tn.rule('_gen1_star_A', (rs) => rs
        .open([loopEntry('_gen1_star_A'), { s: '#A', r: '_gen1_star_A' }, {}])
        .close([{ r: '_gen2_group' }]))
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen1-star-A',
      'r-gen1-star-A = [ r-gen1-star-A / A r-gen1-star-A ] B',
      '',
      'A = %s"a"',
      'B = %s"b"',
    ].join('\n'))
  })

  // A synthetic rule with the loop's whole open scaffold whose close
  // replaces with a synthetic helper that takes a `B` and whose own close
  // pushes a user rule: the helper runs its closes again each time the
  // pushed rule ends, pushing `item` over and over. The rule is no loop,
  // and renders as it always has.
  it('does not read a synthetic loop whose close helper pushes as one', () => {
    const out = emitOf({ '#C': 'c', '#B': 'b', '#A': 'a' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen1_star_A' }, {})
      simpleRule(tn, 'item', { s: '#C' })
      simpleRule(tn, '_gen2_group', { s: '#B' }, { p: 'item' })
      tn.rule('_gen1_star_A', (rs) => rs
        .open([loopEntry('_gen1_star_A'), { s: '#A', r: '_gen1_star_A' }, {}])
        .close([{ r: '_gen2_group' }]))
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen1-star-A',
      'item = C',
      'r-gen1-star-A = [ r-gen1-star-A / A r-gen1-star-A ] B item',
      '',
      'C = %s"c"',
      'A = %s"a"',
      'B = %s"b"',
    ].join('\n'))
  })

  // A loop whose iteration pushes `_gen2_group$alt0`, a synthetic rule
  // that repeats by a cycle of its own, `A _gen2_group$alt0 / B`, and
  // comes back to the loop. The cycle is no iteration of the loop's:
  // read as a helper of the loop, the rule was inlined with its way back
  // to itself rendered as nothing, `*( A / B )` for a loop that takes
  // `*( *A B )`. It stays a production of its own, referenced by name.
  it('keeps a cyclic helper a loop pushes as a production', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_star__gen2_group' }, {})
      tn.rule('_gen2_group$alt0', (rs) => rs
        .open([{ s: '#A', p: '_gen2_group$alt0' }, { s: '#B' }])
        .close([{}]))
      refLoop(tn, '_gen3_star__gen2_group', ['#A', '#B'], '_gen2_group$alt0', '#ZZ')
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = *r-gen2-group-alt0',
      'r-gen2-group-alt0 = A r-gen2-group-alt0 / B',
      '',
      'A = %s"a"',
      'B = %s"b"',
    ].join('\n'))
  })

  // A synthetic rule with the loop's whole open scaffold and a close
  // that pushes a user rule. A push in a close comes back to the close
  // phase when the pushed rule ends and runs the closes again, so the
  // rule pushes `item` over and over, where the loop inlined as a
  // repetition rendered `*A item`, one `item`. It is no loop, and renders
  // as it always has.
  it('does not read a synthetic loop whose close pushes as one', () => {
    const out = emitOf({ '#B': 'b', '#A': 'a' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen1_star_A' }, {})
      simpleRule(tn, 'item', { s: '#B' })
      tn.rule('_gen1_star_A', (rs) => rs
        .open([loopEntry('_gen1_star_A'), { s: '#A', r: '_gen1_star_A' }, {}])
        .close([{ p: 'item' }]))
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen1-star-A',
      'item = B',
      'r-gen1-star-A = [ r-gen1-star-A / A r-gen1-star-A ] item',
      '',
      'B = %s"b"',
      'A = %s"a"',
    ].join('\n'))
  })

  // A `_plus` over `( B *A C )`, its `*A` in the old push-chain shape,
  // whose own trailing star is a loop over the same group, as the
  // compiler builds `1*X`. The old star is the item's, a production
  // referenced by name, and the plus folds round it as it would round any
  // rule the item names, written back as the `1*X` it was compiled from.
  it('folds a plus round an old-shape star inside its item', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b', '#C': 'c' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen6_plus__gen2_group' }, {})
      tn.rule('_gen1_star_A', (rs) => rs.open([{ s: '#A', p: '_gen1_star_A' }, {}]).close([{}]))
      simpleRule(tn, '_gen2_group', { s: '#B', p: '_gen1_star_A' }, { r: '_gen2_group$step1' })
      simpleRule(tn, '_gen2_group$step1', { s: '#C' })
      refLoop(tn, '_gen3_star__gen2_group', ['#B'], '_gen2_group', '#ZZ')
      simpleRule(tn, '_gen6_plus__gen2_group', { p: '_gen2_group' },
        { r: '_gen6_plus__gen2_group$step1' })
      simpleRule(tn, '_gen6_plus__gen2_group$step1', { p: '_gen3_star__gen2_group' }, {})
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = 1*( B r-gen1-star-A C )',
      'r-gen1-star-A = [ A r-gen1-star-A ]',
      '',
      'B = %s"b"',
      'C = %s"c"',
      'A = %s"a"',
    ].join('\n'))
  })

  // A `_plus` helper whose chain pushes `_gen5_group` and ends in a loop
  // over `_gen2_group`, a different rule that renders the same. The
  // compiler's `1*X` pushes the very item its loop repeats; this chain
  // merely reads `X *X`, and `X *X` and `1*X` compile to different
  // recognisers, so writing it back as `1*X` would claim a construction
  // it is not. It keeps its production, and its step, as they are.
  it('keeps a plus over a different item as a production', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b', '#C': 'c' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen6_plus__gen5_group' }, {})
      tn.rule('_gen1_star_A', (rs) => rs.open([{ s: '#A', p: '_gen1_star_A' }, {}]).close([{}]))
      simpleRule(tn, '_gen2_group', { s: '#B', p: '_gen1_star_A' }, { r: '_gen2_group$step1' })
      simpleRule(tn, '_gen2_group$step1', { s: '#C' })
      simpleRule(tn, '_gen5_group', { s: '#B', p: '_gen1_star_A' }, { r: '_gen5_group$step1' })
      simpleRule(tn, '_gen5_group$step1', { s: '#C' })
      refLoop(tn, '_gen3_star__gen2_group', ['#B'], '_gen2_group', '#ZZ')
      simpleRule(tn, '_gen6_plus__gen5_group', { p: '_gen5_group' },
        { r: '_gen6_plus__gen5_group$step1' })
      simpleRule(tn, '_gen6_plus__gen5_group$step1', { p: '_gen3_star__gen2_group' }, {})
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen6-plus--gen5-group',
      'r-gen1-star-A = [ A r-gen1-star-A ]',
      'r-gen6-plus--gen5-group = B r-gen1-star-A C r-gen6-plus--gen5-group-step1',
      'r-gen6-plus--gen5-group-step1 = *( B r-gen1-star-A C )',
      '',
      'A = %s"a"',
      'B = %s"b"',
      'C = %s"c"',
    ].join('\n'))
  })

  // A `_plus` whose chain pushes `item` and ends in a loop whose
  // iteration pushes `item` and then takes a `B` on its way back. Read as
  // the plus over a loop it was written `1*( item B )`, which requires
  // the `B`, where the live chain takes `item` alone. A way back that
  // takes a token is no iteration helper the compiler writes, so the loop
  // is no loop, and it and the plus render as they always have.
  it('keeps a plus whose loop takes more than its item as a production', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen4_plus_item' }, {})
      simpleRule(tn, 'item', { s: '#A' })
      tn.rule('_gen3_star_item', (rs) => rs
        .open([
          loopEntry('_gen3_star_item'),
          { s: '#A', b: 1, r: '_gen3_star_item$alt0' },
          { s: '#ZZ', b: 1 },
          {},
        ]))
      simpleRule(tn, '_gen3_star_item$alt0', { p: 'item', n: { rep: 0 } },
        { r: '_gen3_star_item$alt0$step1', n: { rep: 1 } })
      simpleRule(tn, '_gen3_star_item$alt0$step1', { s: '#B', r: '_gen3_star_item' })
      simpleRule(tn, '_gen4_plus_item', { p: 'item' }, { r: '_gen4_plus_item$step1' })
      simpleRule(tn, '_gen4_plus_item$step1', { p: '_gen3_star_item' }, {})
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen4-plus-item',
      'item = A',
      'r-gen3-star-item = [ r-gen3-star-item / r-gen3-star-item-alt0 ]',
      'r-gen3-star-item-alt0 = item r-gen3-star-item-alt0-step1',
      'r-gen3-star-item-alt0-step1 = B r-gen3-star-item',
      'r-gen4-plus-item = item r-gen4-plus-item-step1',
      'r-gen4-plus-item-step1 = r-gen3-star-item',
      '',
      'A = %s"a"',
      'B = %s"b"',
    ].join('\n'))
  })

  // A loop whose continue replaces with a helper of a shape the compiler
  // never writes: one that pushes the loop itself, and one with an empty
  // way through beside a consuming one. The first rendered the pushed
  // loop as the iteration's back edge, nothing at all; the second dropped
  // the empty way, `*( A B )` for a rule that also takes `A` alone.
  // Neither is a loop, and each renders as it always has.
  it('does not read a loop through a helper of another shape as one', () => {
    {
      const out = emitOf({ '#A': 'a' }, (tn) => {
        simpleRule(tn, 'top', { p: '_gen1_star_x' }, {})
        tn.rule('_gen1_star_x', (rs) => rs
          .open([
            loopEntry('_gen1_star_x'),
            { s: '#A', r: '_gen1_star_x$alt0' },
            { s: '#ZZ', b: 1 },
            {},
          ]))
        simpleRule(tn, '_gen1_star_x$alt0', { p: '_gen1_star_x' }, { r: '_gen1_star_x$alt0$step1' })
        simpleRule(tn, '_gen1_star_x$alt0$step1', { r: '_gen1_star_x' })
        wrapStart(tn, 'top')
      })
      assertAbnf(out, [
        'top = r-gen1-star-x',
        'r-gen1-star-x = [ r-gen1-star-x / A r-gen1-star-x-alt0 ]',
        'r-gen1-star-x-alt0 = r-gen1-star-x r-gen1-star-x-alt0-step1',
        'r-gen1-star-x-alt0-step1 = r-gen1-star-x',
        '',
        'A = %s"a"',
      ].join('\n'))
    }
    {
      const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
        simpleRule(tn, 'top', { p: '_gen1_star_x' }, {})
        tn.rule('_gen1_star_x', (rs) => rs
          .open([
            loopEntry('_gen1_star_x'),
            { s: '#A', r: '_gen1_star_x$alt0' },
            { s: '#ZZ', b: 1 },
            {},
          ]))
        tn.rule('_gen1_star_x$alt0', (rs) => rs
          .open([{}, { s: '#B' }])
          .close([{ r: '_gen1_star_x$alt0$step1' }]))
        simpleRule(tn, '_gen1_star_x$alt0$step1', { r: '_gen1_star_x' })
        wrapStart(tn, 'top')
      })
      assertAbnf(out, [
        'top = r-gen1-star-x',
        'r-gen1-star-x = [ r-gen1-star-x / A r-gen1-star-x-alt0 ]',
        'r-gen1-star-x-alt0 = [ B ] r-gen1-star-x-alt0-step1',
        'r-gen1-star-x-alt0-step1 = r-gen1-star-x',
        '',
        'A = %s"a"',
        'B = %s"b"',
      ].join('\n'))
    }
  })

  // A loop whose item is `_gen2_group$alt0`, a `$alt` rule that takes an
  // `A` or nothing and then a `B`: the iteration takes `A B` or `B`.
  // Inlined as a helper of the loop its empty way was dropped, `*( A B )`;
  // nothing but a loop's inlining would inline a `$alt` rule, and one
  // with an empty way stays a production of its own, as it always was.
  it('keeps a nullable helper inside a loop as a production', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_star_x' }, {})
      tn.rule('_gen2_group$alt0', (rs) => rs.open([{ s: '#A' }, {}]).close([{ s: '#B' }]))
      refLoop(tn, '_gen3_star_x', ['#A', '#B'], '_gen2_group$alt0', '#ZZ')
      wrapStart(tn, 'top')
    })
    assertAbnf(out, 'top = *r-gen2-group-alt0\nr-gen2-group-alt0 = [ A ] B\n\nA = %s"a"\nB = %s"b"')
  })

  // A user loop over `*A` whose close phase takes a `B` and replaces the
  // rule with itself, or ends: `H = *A [ B H ]`. The iteration's own back
  // edges render as nothing, but the close's `r: H` is the rule again,
  // and dropping it rendered `H = *A [ B ]`, which stops after one `B`.
  it("keeps a loop's close that re-enters it", () => {
    const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      tn.options({ rule: { start: 'once' } })
      tn.rule('once', (rs) => rs
        .open([loopEntry('once'), { s: '#A', r: 'once' }, {}])
        .close([{ s: '#B', r: 'once' }, {}]))
    })
    assertLoopAbnf(out, 'once = *A [ B once ]\n\nA = %s"a"\nB = %s"b"')
  })

  // The same scaffold on a synthetic rule, which is inlined where it is
  // referenced and has no production of its own to name from its close:
  // it is no loop, and renders as it always has, a production.
  it('does not read a synthetic loop whose close re-enters it as one', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen1_star_A' }, {})
      tn.rule('_gen1_star_A', (rs) => rs
        .open([loopEntry('_gen1_star_A'), { s: '#A', r: '_gen1_star_A' }, {}])
        .close([{ s: '#B', r: '_gen1_star_A' }, {}]))
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = r-gen1-star-A',
      'r-gen1-star-A = [ r-gen1-star-A / A r-gen1-star-A ] [ B r-gen1-star-A ]',
      '',
      'A = %s"a"',
      'B = %s"b"',
    ].join('\n'))
  })

  // `top = *( "b" *"a" "c" )` with the outer star in the loop shape and
  // the inner star in the OLD push-chain shape, a mix a hand-built grammar
  // can carry. A loop's helpers are the synthetic rules its iteration
  // reaches short of a kept production, and the old star is a kept
  // production: it stays a bareword reference inside the repetition, with
  // its production and its epsilon branch. Finding the helpers by
  // reachability alone added the old star to them, suppressed its
  // production and inlined it without its epsilon branch and back edge:
  // `*( B A C )`, exactly one `A` where the original takes any number.
  it('keeps an old-shape star inside a loop group', () => {
    const out = emitOf({ '#A': 'a', '#B': 'b', '#C': 'c' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_star__gen2_group' }, {})
      tn.rule('_gen1_star_A', (rs) => rs.open([{ s: '#A', p: '_gen1_star_A' }, {}]).close([{}]))
      simpleRule(tn, '_gen2_group', { s: '#B', p: '_gen1_star_A' }, { r: '_gen2_group$step1' })
      simpleRule(tn, '_gen2_group$step1', { s: '#C' })
      refLoop(tn, '_gen3_star__gen2_group', ['#B'], '_gen2_group', '#ZZ')
      wrapStart(tn, 'top')
    })
    assertAbnf(out, [
      'top = *( B r-gen1-star-A C )',
      'r-gen1-star-A = [ A r-gen1-star-A ]',
      '',
      'B = %s"b"',
      'C = %s"c"',
      'A = %s"a"',
    ].join('\n'))
  })

  // `top = *( *"a" "b" / "c" ) "d"`: a loop over a group that has a
  // `$alt` / `$step` chain of its own, because one of its alternatives
  // starts with a rule. That chain is the loop's to inline, like the
  // group and the loop's own `H$alt0` and `H$alt0$step1`: the helpers of
  // a loop are everything its iteration reaches short of a kept
  // production. Finding them by name (`H$…`) missed the group's chain,
  // which `isFoldable` refuses for its `$alt`, and it surfaced as three
  // kept productions, `top = *( r-gen2-group-alt0 / r-gen2-group-alt1 ) D`
  // with `r-gen2-group-alt0 = *A r-gen2-group-alt0-step1`. The inner loop
  // renders as `*A` inside the alternative.
  it('renders a loop over a group with a star alternative', () => {
    const out = emitOf({ '#D': 'd', '#A': 'a', '#B': 'b', '#C': 'c' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_star__gen2_group' }, { r: 'top$step1' })
      simpleRule(tn, 'top$step1', { s: '#D' })
      terminalLoop(tn, '_gen1_star_term', '#A', '#B')
      simpleRule(tn, '_gen2_group$alt0', { p: '_gen1_star_term' }, { r: '_gen2_group$alt0$step1' })
      simpleRule(tn, '_gen2_group$alt0$step1', { s: '#B' })
      simpleRule(tn, '_gen2_group$alt1', { s: '#C' })
      tn.rule('_gen2_group', (rs) => rs
        .open([
          { s: '#A', b: 1, p: '_gen2_group$alt0' },
          { s: '#B', b: 1, p: '_gen2_group$alt0' },
          { s: '#C', b: 1, p: '_gen2_group$alt1' },
        ])
        .close([{}]))
      refLoop(tn, '_gen3_star__gen2_group', ['#A', '#B', '#C'], '_gen2_group', '#D')
      wrapStart(tn, 'top')
    })
    assertLoopAbnf(out, 'top = *( *A B / C ) D\n\nA = %s"a"\nB = %s"b"\nC = %s"c"\nD = %s"d"')
  })

  // `top = 1*( *"a" "b" / "c" ) "d"`: the `_plus` helper over the same
  // group pushes it directly and, on close, steps into the loop. The
  // group's chain folds, so the helper folds too, and is written back as
  // the `1*( … )` it was compiled from. With the chain refused as kept
  // productions the helper was refused as well and came out as
  // `top = r-gen3-plus--gen2-group D` over a body of `X *X`, the spelling
  // that does not round-trip on a nullable item — and this item is
  // nullable: `*A B` can start with the `B`.
  it('renders a plus over a group with a star alternative', () => {
    const out = emitOf({ '#D': 'd', '#A': 'a', '#B': 'b', '#C': 'c' }, (tn) => {
      simpleRule(tn, 'top', { p: '_gen3_plus__gen2_group' }, { r: 'top$step1' })
      simpleRule(tn, 'top$step1', { s: '#D' })
      terminalLoop(tn, '_gen1_star_term', '#A', '#B')
      simpleRule(tn, '_gen2_group$alt0', { p: '_gen1_star_term' }, { r: '_gen2_group$alt0$step1' })
      simpleRule(tn, '_gen2_group$alt0$step1', { s: '#B' })
      simpleRule(tn, '_gen2_group$alt1', { s: '#C' })
      tn.rule('_gen2_group', (rs) => rs
        .open([
          { s: '#A', b: 1, p: '_gen2_group$alt0' },
          { s: '#B', b: 1, p: '_gen2_group$alt0' },
          { s: '#C', b: 1, p: '_gen2_group$alt1' },
        ])
        .close([{}]))
      refLoop(tn, '_gen3_star__gen2_group', ['#A', '#B', '#C'], '_gen2_group', '#D')
      simpleRule(tn, '_gen3_plus__gen2_group', { p: '_gen2_group' },
        { r: '_gen3_plus__gen2_group$step1' })
      simpleRule(tn, '_gen3_plus__gen2_group$step1', { p: '_gen3_star__gen2_group' }, {})
      wrapStart(tn, 'top')
    })
    assertLoopAbnf(out, 'top = 1*( *A B / C ) D\n\nA = %s"a"\nB = %s"b"\nC = %s"c"\nD = %s"d"')
  })

  // `odd = A [ odd ]`, hand-built with the close continuation carrying the
  // loop entry's whole shape: `{ c: [n.rep == 0], n: {rep: 1}, s: A,
  // b: 1, r: odd }`, then `{ }`. A loop is decided by its OPEN
  // alternatives, and `odd` has no entry among them, so it is no loop, and
  // a rule that is not a loop has no entry to skip: every alternative is
  // content. Skipping the entry's shape wherever it appeared emitted
  // `odd = A`, exactly one where the rule takes one or more. It renders
  // as it always has.
  it('keeps an entry-shaped close continuation of a rule that is no loop', () => {
    const out = emitOf({ '#A': 'a' }, (tn) => {
      tn.options({ rule: { start: 'odd' } })
      tn.rule('odd', (rs) => rs
        .open([{ s: '#A' }])
        .close([{ c: { 'n.rep': 0 }, s: '#A', b: 1, r: 'odd', n: { rep: 1 } }, {}]))
    })
    assertLoopAbnf(out, 'odd = A [ odd ]\n\nA = %s"a"')
  })

  // The engine keeps no declarative condition, only the closure it compiled
  // it to, so the entry's guard is read by what that closure does. Only
  // `{ 'n.rep': 0 }` is the guard: not another comparison that also holds
  // at 0, not a second condition beside it, not another counter, and not a
  // function of the grammar's own however it behaves, which the Rust port
  // never reads as the guard either. With any of those the loop's whole
  // scaffold is still there, and the rule renders as itself.
  it('reads the loop entry guard from the compiled condition alone', () => {
    const entryOf = (c) => ({ c, n: { rep: 1 }, r: '_gen1_star_A' })
    const emitLoop = (entry) => emitOf({ '#A': 'a' }, (tn) => {
      tn.options({ rule: { start: 'top' } })
      tn.rule('top', (rs) => rs.open([{ p: '_gen1_star_A' }]).close([{}]))
      tn.rule('_gen1_star_A', (rs) => rs.open([
        entry,
        { s: '#A', r: '_gen1_star_A' },
        ...loopExits('#ZZ'),
      ]))
    })

    assertLoopAbnf(emitLoop(entryOf({ 'n.rep': 0 })), 'top = *A\n\nA = %s"a"')

    for (const c of [
      { 'n.rep': { $lte: 0 } },
      { 'n.rep': { $lt: 1 } },
      { 'n.rep': { $ne: 1 } },
      { 'n.rep': 0, 'n.mode': 0 },
      { 'n.mode': 0 },
      (rule) => 0 === (rule.n.rep || 0),
      function (rule) { return 0 === (rule.n.rep || 0) },
    ]) {
      const out = emitLoop(entryOf(c))
      assert.strictEqual(
        out,
        'top = r-gen1-star-A\nr-gen1-star-A = [ r-gen1-star-A / A r-gen1-star-A ]\n\nA = %s"a"',
        'not the loop entry guard: ' + ('function' === typeof c ? c.toString() : JSON.stringify(c)),
      )
      assertRfc5234Shape(out)
    }
  })
})
