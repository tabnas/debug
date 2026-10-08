/* Copyright (c) 2021-2026 Richard Rodger, MIT License */

/*  debug.ts
 *  Debug plugin — adds tracing helpers and a `describe()` method.
 */

import type {
  Context,
  NormAltSpec,
  Config,
  AltMatch,
  Tabnas,
  Plugin,
  RuleSpec,
  Rule,
  Lex,
  Point,
  LexMatcher,
  Token,
} from '@tabnas/parser'

import { S, util, EMPTY } from '@tabnas/parser'


// TODO: custom stringify for nodes
type DebugOptions = {
  print: boolean
  trace: Record<string, boolean> & {
    step: boolean,
    rule: boolean,
    lex: boolean,
    parse: boolean,
    node: boolean,
    stack: boolean,
  }
}


const DEFAULTS: DebugOptions = {
  print: true,
  trace: {
    step: true,
    rule: true,
    lex: true,
    parse: true,
    node: true,
    stack: true,
  },
}

// ---- structured model -------------------------------------------------
// `describe()` renders the instance as printable text; `model()` returns
// the same information as a typed, JSON-serialisable object so tools and
// tests can consume the grammar/instance programmatically.

export type DebugTokenInfo = { tin: number; name: string; fixed?: string }
export type DebugTokenSet = { name: string; tins: number[] }
export type DebugAltInfo = {
  seq: (string | string[])[]          // token name(s) per lookahead position
  push?: string                       // `p` target rule (or '<fn>')
  replace?: string                    // `r` target rule (or '<fn>')
  back?: number                       // `b` token push-back
  counters?: Record<string, number>   // `n` counter ops
  groups: string[]                    // `g` group tags
  action: boolean                     // `a` present
  cond: boolean                       // `c` present
  modifier: boolean                   // `h` present
}
export type DebugRuleInfo = {
  name: string
  open: DebugAltInfo[]
  close: DebugAltInfo[]
}
export type DebugRuleEdges = {
  name: string
  openPush: string[]
  openReplace: string[]
  closePush: string[]
  closeReplace: string[]
}
export type DebugLexMatcher = { order: number; matcher: string; make: string }
export type DebugConfigInfo = {
  start: string
  finish: boolean
  safeKey: boolean
  lex: Record<string, boolean>
}
export type DebugPluginInfo = { name: string; options?: Record<string, any> }
export type DebugModel = {
  tag: string
  tokens: DebugTokenInfo[]
  tokenSets: DebugTokenSet[]
  rules: DebugRuleInfo[]
  graph: DebugRuleEdges[]
  lexer: DebugLexMatcher[]
  config: DebugConfigInfo
  plugins: DebugPluginInfo[]
  abnf: string
}


const { entries, tokenize } = util

const Debug: Plugin = (tabnas: Tabnas, options: DebugOptions) => {
  options.trace =
    true === (options.trace as any) ? { ...DEFAULTS.trace } : options.trace

  const { keys, values, entries } = tabnas.util

  tabnas.debug = {
    abnf: function(): string {
      return emitAbnf(tabnas)
    },

    describe: function(): string {
      let cfg = tabnas.internal().config
      let match = cfg.lex.match
      let rules = tabnas.rule()

      return [
        '========= INSTANCE ========',
        '  tag: ' + (tabnas.internal().merged.tag ?? ''),
        '\n',

        '========= TOKENS ========',
        Object.entries(cfg.t)
          .filter((te) => 'string' === typeof te[1])
          .map((te) => {
            return (
              '  ' +
              te[0] +
              '\t' +
              te[1] +
              '\t' +
              ((s: string | number) => (s ? '"' + s + '"' : ''))(
                cfg.fixed.ref[te[0] as string] || '',
              )
            )
          })
          .join('\n'),
        '\n',

        Object.entries(cfg.tokenSet)
          .map((te) => {
            return (
              '    ' +
              te[0] +
              '\t' +
              Object.keys(cfg.tokenSetTins[te[0]] ?? [])
            )
          })
          .join('\n'),
        '\n',

        '========= RULES =========',
        ruleTree(tabnas, keys(rules), rules),
        '\n',

        '========= ALTS =========',
        values(rules)
          .map(
            (rs: any) =>
              '  ' +
              rs.name +
              ':\n' +
              descAlt(tabnas, rs, 'open') +
              descAlt(tabnas, rs, 'close'),
          )
          .join('\n\n'),

        '\n',
        '========= LEXER =========',
        '  ' +
        (
          (match &&
            match.map(
              (m: any) =>
                m.order + ': ' + m.matcher + ' (' + m.make.name + ')',
            )) ||
          []
        ).join('\n  '),
        '\n',

        '========= CONFIG ========',
        [
          '  start: ' + cfg.rule.start,
          '  finish: ' + cfg.rule.finish,
          '  safeKey: ' + cfg.safe.key,
          '  lex.fixed: ' + cfg.fixed.lex,
          '  lex.space: ' + cfg.space.lex,
          '  lex.line: ' + cfg.line.lex,
          '  lex.text: ' + cfg.text.lex,
          '  lex.number: ' + cfg.number.lex,
          '  lex.comment: ' + cfg.comment.lex,
          '  lex.string: ' + cfg.string.lex,
          '  lex.value: ' + cfg.value.lex,
        ].join('\n'),
        '\n',

        '\n',
        '========= PLUGIN =========',
        '  ' +
        tabnas
          .internal()
          .plugins.map(
            (p: Plugin) =>
              p.name +
              (p.options
                ? entries(p.options).reduce(
                  (s: string, e: any[]) =>
                    (s += '\n    ' + e[0] + ': ' + JSON.stringify(e[1])),
                  '',
                )
                : ''),
          )
          .join('\n  '),
        '\n',

        '========= ABNF =========',
        emitAbnf(tabnas),
        '\n',
      ].join('\n')
    },

    // Structured counterpart to describe(): the instance/grammar as a
    // typed, JSON-serialisable object (token table, rules + alternates,
    // rule-reference graph, lexer matchers, config, plugins, ABNF text).
    model: function(): DebugModel {
      const cfg = tabnas.internal().config
      const rules = tabnas.rule()
      const match = (cfg.lex as any).match

      return {
        tag: tabnas.internal().merged.tag ?? '',

        tokens: Object.entries(cfg.t as Record<string, any>)
          .filter((te) => 'string' === typeof te[1])
          .map((te) => {
            const fixed = (cfg.fixed.ref as any)[te[0]]
            const info: DebugTokenInfo = { tin: Number(te[0]), name: te[1] }
            if (fixed) info.fixed = fixed
            return info
          }),

        tokenSets: Object.entries(cfg.tokenSet).map((te) => ({
          name: te[0],
          tins: Array.isArray(te[1])
            ? (te[1] as number[]).slice()
            : Object.keys((cfg.tokenSetTins as any)[te[0]] ?? {}).map(Number),
        })),

        rules: values(rules).map((rs: any) => ({
          name: rs.name,
          open: rs.def.open.map((a: any) => altInfo(tabnas, a)),
          close: rs.def.close.map((a: any) => altInfo(tabnas, a)),
        })),

        graph: keys(rules).map((n: string) => ({
          name: n,
          openPush: ruleEdges(rules, n, 'open', 'p'),
          openReplace: ruleEdges(rules, n, 'open', 'r'),
          closePush: ruleEdges(rules, n, 'close', 'p'),
          closeReplace: ruleEdges(rules, n, 'close', 'r'),
        })),

        lexer: ((match as any[]) || []).map((m: any) => ({
          order: m.order,
          matcher: String(m.matcher),
          make: (m.make && m.make.name) || '',
        })),

        config: {
          start: cfg.rule.start,
          finish: cfg.rule.finish,
          safeKey: cfg.safe.key,
          lex: {
            fixed: cfg.fixed.lex,
            space: cfg.space.lex,
            line: cfg.line.lex,
            text: cfg.text.lex,
            number: cfg.number.lex,
            comment: cfg.comment.lex,
            string: cfg.string.lex,
            value: cfg.value.lex,
          },
        },

        plugins: tabnas.internal().plugins.map((p: Plugin) => {
          const info: DebugPluginInfo = { name: p.name }
          if (p.options) info.options = p.options
          return info
        }),

        abnf: emitAbnf(tabnas),
      }
    },
  }

  // Wrap use() once per instance so repeated application or child forks
  // (the engine re-runs parent plugins on make()) do not re-stack the wrapper.
  if (!(tabnas as any).__debugUseWrapped) {
    ;(tabnas as any).__debugUseWrapped = true
    const origUse = tabnas.use.bind(tabnas)

    tabnas.use = (...args) => {
      let self = origUse(...args)
      if (options.print) {
        // use() may return a wrapper instance; describe() whichever carries it.
        const inst: any = self && (self as any).debug ? self : tabnas
        if (inst.debug && inst.debug.describe) {
          tabnas
            .internal()
            .config.debug.get_console()
            .log(
              'USE:',
              (args[0] && args[0].name) || '',
              '\n\n',
              inst.debug.describe(),
            )
        }
      }
      return self
    }
  }


  if (options.trace) {
    tabnas.options({
      parse: {
        prepare: {
          debug: (_tabnas: Tabnas, ctx: Context, _meta: any) => {
            // Call through the console provider each time so a user-supplied
            // get_console() whose log() depends on `this` is not detached.
            const con = ctx.cfg.debug.get_console()
            con.log('\n========= TRACE ==========')
            ctx.log =
              ctx.log ||
              ((kind: string, ...rest: any) => {
                if (LOGKIND[kind] && options.trace[kind]) {
                  con.log(
                    LOGKIND[kind](...rest)
                      .filter((item: any) => 'object' != typeof item)
                      .map((item: any) =>
                        'function' == typeof item ? item.name : item,
                      )
                      .join('  '),
                  )
                }
              })
          },
        },
      },
    })
  }
}

// Emit an ABNF representation of the instance's *live* grammar.
//
// This reads ONLY the running engine (config + normalised rule specs);
// it never imports @tabnas/abnf. The mapping is the empirical inverse of
// abnf's forward encoding (see the round-trip test): tabnas rules become
// ABNF productions, OPEN alts become `/`-separated alternatives, the
// token sequence (.s) plus any push/replace target (.p/.r) becomes a
// space-separated element list, and each token resolves to an ABNF
// terminal via the fixed-literal / match-regex config.
//
// Actions (a.a) carry no ABNF meaning and are ignored. Constructs that
// cannot be represented (e.g. arbitrary match regexes) are emitted as
// ABNF comments so the output stays valid and self-documenting, even
// though such rules will not round-trip.
// RFC 5234: `rulename = ALPHA *(ALPHA / DIGIT / "-")`. Engine rule names are
// not so constrained — the abnf compiler synthesises `_gen1_star_term` and
// `…$alt0`, and a regex token arrives as `RX___U0030__U0039`. Emitted
// verbatim those are rejected by every conforming ABNF tool, so map each to
// a legal name ONCE and reuse it for the production head and every reference.
//
// Returns a memoised mapper. Distinct source names can sanitise to the same
// string (`a_b` and `a-b` both give `a-b`), so collisions get a numeric
// suffix — without it the grammar would silently merge two rules.
//
// RULES AND TOKENS ARE SEPARATE NAMESPACES, and that separation is the whole
// point of the two members. A token's bare name and a rule's name are drawn
// from different sources and can coincide: `#NR` beside a rule called `NR`
// is ordinary. Sharing one cache merged them — the token looked up `NR`, hit
// the entry the reserve loop had made for the RULE, and the emitter then
// wrote the rule's own name for a terminal. A grammar whose rule `NR`
// referenced token `#NR` came out as a self-referential `NR = NR` plus a
// second `NR = <number>` definition: two definitions of one rule with `=`
// rather than incremental `=/`, and a production that recognises nothing.
//
// Keeping the caches apart while sharing `taken` is what fixes it. The token
// still sanitises the same way, but it allocates against everything already
// claimed, so it suffixes to `NR-2` instead of borrowing the rule's name.
function abnfNamer(reserve: string[]): {
  rule: (name: string) => string
  token: (bare: string) => string
} {
  const isLegal = (n: string): boolean => /^[A-Za-z][A-Za-z0-9-]*$/.test(n)
  const ruleCache = new Map<string, string>()
  const tokenCache = new Map<string, string>()

  // RFC 5234 §2.1: "ABNF rule names are case-insensitive." So `Foo-Bar` and
  // `foo-bar` ARE the same rule, and the collision check has to fold case —
  // comparing exact spellings would let a sanitised `foo_bar` land beside a
  // reserved `Foo-Bar` and emit two definitions of one rule.
  const taken = new Set<string>()
  const claim = (n: string): void => { taken.add(n.toLowerCase()) }
  const isTaken = (n: string): boolean => taken.has(n.toLowerCase())

  // Names that are ALREADY legal keep their spelling, and claim it up front:
  // otherwise a sanitised synthetic could take `foo-bar` first and rename the
  // user's real `foo-bar` rule out from under them.
  for (const n of reserve) {
    if (isLegal(n) && !isTaken(n)) {
      claim(n)
      ruleCache.set(n, n)
    }
  }

  // Sanitise to a legal rulename, then make it unique against everything
  // claimed so far — reserved rule names and previously allocated names
  // alike, since `taken` is shared by both namespaces.
  const allocate = (name: string): string => {
    let out = name.replace(/[^A-Za-z0-9-]/g, '-')
    if (!/^[A-Za-z]/.test(out)) out = 'r' + out
    if (isTaken(out)) {
      let n = 2
      while (isTaken(out + '-' + n)) n++
      out = out + '-' + n
    }
    claim(out)
    return out
  }

  const memo = (cache: Map<string, string>, name: string): string => {
    const hit = cache.get(name)
    if (undefined !== hit) return hit
    const out = allocate(name)
    cache.set(name, out)
    return out
  }

  return {
    rule: (name: string): string => memo(ruleCache, name),
    token: (bare: string): string => memo(tokenCache, bare),
  }
}

function emitAbnf(tabnas: Tabnas): string {
  const cfg = tabnas.internal().config
  const rules = tabnas.rule() as Record<string, RuleSpec>

  // Seeded with the rule names so user-authored ones are never displaced;
  // token legend names are added as they are first referenced.
  const abnfName = abnfNamer(Object.keys(rules))

  // bnf wraps grammars in a synthetic '__start__' rule (open .p -> the
  // real start, close matches #ZZ); skip it and lead with the real start.
  const synthWrapper: string | null =
    '__start__' === cfg.rule.start ? cfg.rule.start : null
  let startRule: string | null = synthWrapper ? null : cfg.rule.start
  if (synthWrapper) {
    const wrapper: any = rules[synthWrapper]
    if (wrapper) {
      for (const alt of wrapper.def.open) {
        if ('string' === typeof alt.p) { startRule = alt.p; break }
        if ('string' === typeof alt.r) { startRule = alt.r; break }
      }
    }
  }

  const nameToTin = (name: string): number | undefined => {
    const tin = (cfg.t as any)[name]
    return 'number' === typeof tin ? tin : undefined
  }
  const endTin = nameToTin('#ZZ')
  const toTin = (t: any): number | undefined =>
    'number' === typeof t ? t : (cfg.t as any)[t]

  const used = new Map<string, string>()
  const terminal = (tin: number): string =>
    emitAbnfTerminal(tabnas, cfg, tin, used, abnfName)

  const has = (name: string): boolean => null != rules[name]
  const opensOf = (name: string): any[] => (rules[name] as any)?.def?.open || []
  const closesOf = (name: string): any[] => (rules[name] as any)?.def?.close || []
  const slots = (alt: any): number[][] => abnfSlots(alt, toTin)
  const back = (alt: any): number => abnfBack(alt, slots(alt).length)
  const targets = (name: string): string[] =>
    [...opensOf(name), ...closesOf(name)]
      .map(abnfTarget)
      .filter((t): t is string => null != t)

  // A rule the abnf forward-compiler synthesised for a `[...]` / `*(...)` /
  // `1*(...)` / group / chain-step: named `_gen<n>_…` or carrying a `$`.
  // These are never user-authored, so instead of emitting them as their own
  // productions we fold each back into the ABNF construct it encodes — a
  // reasonable round-trip (`tn.abnf(G)` then `debug.abnf()` reproduces `G`,
  // not the expanded internal form).
  const isSynthetic = (name: string): boolean =>
    name !== synthWrapper && (abnfIsGenName(name) || name.includes('$'))

  // ---- The repeat loop ------------------------------------------------------
  //
  // A repetition is read by SHAPE, not by name. Since tabnas/bnf#80 the BNF
  // compiler (which abnf, ebnf and gbnf compile through) emits every `*A` as
  // a replace loop: a helper `H` whose first open alternative is the entry
  // `{ c: { 'n.rep': 0 }, n: { rep: 1 }, r: H }` (it consumes nothing,
  // pushes nothing and replaces the rule with itself under the guard,
  // allocating the node and counting the iteration), followed by the
  // continue alternatives that take one item and come back to `H`, and by
  // the exits (a FOLLOW peek `{ s: FOLLOW, b: 1 }` and `{ }`). The guard is
  // part of the shape: `s`, `b`, `p` and `r` alone also describe a user
  // rule's own non-consuming self-replace, a guarded or counted state
  // transition, which is no repetition. Such a rule is rendered wherever it
  // is referenced as `*A` / `*( a b )`, and neither it nor its iteration
  // helpers (`H$alt0` and `H$alt0$step1`, and the `$alt` / `$step` chains of
  // the foldable groups the iteration pushes, everything the iteration
  // reaches short of a kept production) is emitted as a production. The
  // older push chain (`H = A H / ε`, one frame per item) carries no such
  // entry and renders as before, as a kept production, wherever it sits.
  // `docs/reference.md`, "The repeat loop", has the whole contract; the
  // Rust port (`rs/src/abnf.rs`) implements the same, function for function.

  // `rule` is a repeat loop: its open alternatives are the compiler's whole
  // scaffold, in the compiler's order, not its entry alone. The entry comes
  // first; then at least one continue that comes back to `rule` having made
  // progress, and the empty exit `{ }`, unconditional, with no continue
  // after it, since it takes whatever comes. FOLLOW peeks may stand among
  // the exits, before the continues too, and shadow only what they peek. A
  // continue a peek before it covers never runs: it still renders, so its
  // item must be one a live continue takes too. No alternative may be one
  // a function routes, and a synthetic loop's closes, if any, do nothing.
  const isLoop = (rule: string): boolean => {
    const open = opensOf(rule)
    if (0 === open.length) return false
    const [first, ...rest] = open
    if (!abnfIsLoopEntry(first, rule, slots(first), back(first)) || abnfIsDynamic(first)) {
      return false
    }
    let continues = false
    let exit = false
    let shadowed = false
    const dead = shadowedByPeeks(open, rule)
    const liveItems: (string | null)[] = []
    const deadItems: (string | null)[] = []
    for (let index = 0; index < rest.length; index++) {
      const alt = rest[index]
      // The compiler writes one entry; a second can never be taken (the
      // first set its counter) and is neither continue nor exit.
      if (abnfIsDynamic(alt) || abnfIsLoopEntry(alt, rule, slots(alt), back(alt))) {
        return false
      }
      const s = slots(alt)
      if (!hasContentAs(alt, rule, true)) {
        // The compiler never guards an exit, and its exits give back
        // exactly what they peek: the empty exit nothing, a FOLLOW peek
        // its tokens.
        if (
          !abnfIsPlainWay(alt) ||
          back(alt) !== s.length ||
          s.some((slot) => 0 === slot.length)
        ) {
          return false
        }
        // Only the empty exit stops the rule whatever comes next, and
        // shadows every alternative after it.
        if (0 === s.length) {
          exit = true
          shadowed = true
        }
      } else if (!shadowed && continuesLoop(alt, rule)) {
        const item = continueItem(alt, rule)
        if (dead[index + 1]) {
          deadItems.push(item)
        } else {
          continues = true
          liveItems.push(item)
        }
      } else {
        return false
      }
    }
    return (
      continues &&
      exit &&
      deadItems.every((item) => liveItems.includes(item)) &&
      (!isSynthetic(rule) || closesOf(rule).every((alt) => isIdle(alt)))
    )
  }

  // The item a continue of the loop `rule` takes: the token a terminal
  // continue consumes, or the rule the iteration helper pushes.
  const continueItem = (alt: any, rule: string): string | null => {
    if (abnfReplace(alt) === rule) {
      const s = slots(alt)
      return 0 < s.length ? abnfTokenItem(s[0]) : null
    }
    const helper = abnfReplace(alt)
    if (null == helper || !has(helper)) return null
    const first = opensOf(helper)[0]
    const pushed = null == first ? null : abnfPush(first)
    return null == pushed ? null : abnfRuleItem(pushed)
  }

  // A continue in one of the compiler's two shapes, coming back to its loop
  // `rule` having taken one item. A terminal continue consumes its token
  // and replaces with `rule`, giving back any it peeked after it
  // (`{ s: [A], r: H }`, `{ s: [A, X], b: 1, r: H }`). A rule continue
  // peeks the item's first tokens, gives them all back and replaces with
  // the loop's iteration helper, which pushes the item and comes back.
  // Either sets no counter, carries no guard but the suffix-debt counter,
  // and matches no empty token slot.
  const continuesLoop = (alt: any, rule: string): boolean => {
    const s = slots(alt)
    if (
      null != abnfPush(alt) ||
      0 === s.length ||
      s.some((slot) => 0 === slot.length) ||
      0 < Object.keys(abnfCounters(alt)).length ||
      !abnfIsGuardedAsCompiled(alt)
    ) {
      return false
    }
    const target = abnfReplace(alt)
    if (null == target) return false
    if (target === rule) return s.length === back(alt) + 1
    return s.length === back(alt) && isIterationHelper(target, rule)
  }

  // `helper` is the iteration helper the compiler gives the loop `rule`
  // over a rule item, and nothing else: its one open pushes the item (a
  // rule other than `rule`, which reaches `rule` by no synthetic way of
  // its own), its one close replaces with its step, and the step's one
  // open replaces with `rule`, all matching no token and setting no counter
  // but `rep` (`H$alt0` -> `H$alt0$step1` -> `H`).
  const isIterationHelper = (helper: string, rule: string): boolean => {
    const bare = (alt: any): boolean =>
      0 === slots(alt).length &&
      0 === back(alt) &&
      null == abnfPush(alt) &&
      abnfIsHelperWay(alt)
    if (!has(helper)) return false
    const helperOpens = opensOf(helper)
    const helperCloses = closesOf(helper)
    if (1 !== helperOpens.length || 1 !== helperCloses.length) return false
    const [open] = helperOpens
    const [close] = helperCloses
    const item = abnfPush(open)
    if (null == item || item === rule) return false
    if (
      0 !== slots(open).length ||
      0 !== back(open) ||
      null != abnfReplace(open) ||
      !abnfIsHelperWay(open)
    ) {
      return false
    }
    if (!isSynthetic(helper) || !bare(close) || reaches(item, rule)) return false
    const step = abnfReplace(close)
    if (null == step || !has(step)) return false
    const stepOpens = opensOf(step)
    if (1 !== stepOpens.length) return false
    return (
      isSynthetic(step) &&
      bare(stepOpens[0]) &&
      abnfReplace(stepOpens[0]) === rule &&
      closesOf(step).every((alt) => isIdle(alt))
    )
  }

  // `from` reaches `rule`, by a push or a replace, itself or through
  // synthetic rules: an item that does repeats `rule` inside its own
  // iteration, which the repetition cannot render.
  const reaches = (from: string, rule: string): boolean => {
    const visited = new Set<string>()
    const pending = [from]
    while (0 < pending.length) {
      const name = pending.pop() as string
      if (name === rule) return true
      if ((name !== from && !isSynthetic(name)) || visited.has(name)) continue
      visited.add(name)
      if (has(name)) pending.push(...targets(name))
    }
    return false
  }

  // The iteration helpers of every loop: the synthetic rules reachable from
  // its continue alternatives without passing through a KEPT PRODUCTION,
  // which the walk neither enters nor adds: a user rule, another loop, an
  // old push-chain repetition, a `_plus` (judged for itself), a synthetic
  // rule that repeats by a cycle of its own, and one with an empty way
  // through. The bound is by kept productions, not by name.
  const findLoopHelpers = (): Set<string> => {
    const helpers = new Set<string>()
    const pending: string[] = []
    for (const name of [...loops].sort()) {
      for (const alt of opensOf(name)) {
        const target = abnfTarget(alt)
        if (null != target && target !== name) pending.push(target)
      }
    }
    while (0 < pending.length) {
      const name = pending.pop() as string
      if (
        boundsLoopHelpers(name) ||
        helpers.has(name) ||
        cycles(name) ||
        isNullableUnfolded(name)
      ) {
        continue
      }
      if (!has(name)) continue
      helpers.add(name)
      pending.push(...targets(name))
    }
    return helpers
  }

  // A synthetic rule with an empty way through its opens, which nothing but
  // a loop's inlining would inline: not an optional's own helper, nor a
  // foldable rule. It stays a production of its own, referenced by name.
  const isNullableUnfolded = (name: string): boolean =>
    !abnfIsHelper(name, 'opt') &&
    !isFoldable(name) &&
    has(name) &&
    opensOf(name).some((alt) => !hasContentAs(alt, name, false))

  // A synthetic rule that reaches itself again through synthetic rules none
  // of which bounds a loop's helpers: a repetition of its own, which no
  // loop's iteration accounts for. A loop's own `H$alt0` reaches itself
  // only through `H`, a loop, and is no cycle.
  const cycles = (name: string): boolean => {
    const visited = new Set<string>()
    const pending = targets(name)
    while (0 < pending.length) {
      const target = pending.pop() as string
      if (target === name) return true
      if (boundsLoopHelpers(target) || visited.has(target)) continue
      visited.add(target)
      pending.push(...targets(target))
    }
    return false
  }

  // A rule that is a production of its own, or decides that for itself,
  // and so bounds the iteration helpers of every loop: a user rule, a
  // loop, or a repetition helper that is no loop's own (an old push-chain
  // `_star` with its helpers, and a `_plus` in either shape). A loop's own
  // `H$alt0` and `H$alt0$step1` carry the loop's whole name and are read
  // by their own segment: the loop.
  const boundsLoopHelpers = (name: string): boolean => {
    if (!isSynthetic(name) || loops.has(name)) return true
    const kind = abnfGenKind(name)
    return ('star' === kind || 'plus' === kind) && !loops.has(abnfOwnSegment(name))
  }

  // A repetition helper in the old push-chain shape, kept as a production
  // as it always was: a `_star` that is not a loop, or one of its iteration
  // helpers, which carry its name.
  const isKeptRepetition = (name: string): boolean =>
    'star' === abnfGenKind(name) && !loops.has(abnfOwnSegment(name))

  // Only the clean cases fold: `[…]` optionals plus the group / chain
  // helpers they inline through, and a `_plus` built over a loop. A
  // repetition in the old push-chain shape (`_star` / `_plus` and their
  // `$alt…` helpers) does not reconstruct reliably, so those rules are
  // emitted as productions unchanged (still a valid, recognition-equivalent
  // grammar). A repetition in the loop shape is not decided here: see
  // `isFolded`. The kind is read from the rule's own name segment, so a
  // helper named after a repetition it merely contains is judged by what
  // it is.
  const isFoldable = (name: string): boolean => {
    if (!isSynthetic(name) || 'star' === abnfGenKind(name) || name.includes('$alt')) {
      return false
    }
    return 'plus' !== abnfGenKind(name) || plusFolds(name)
  }

  // `1*A` compiles to a `_plus` helper: `A` followed by the star of `A`. The
  // helper folds when that star is a loop and the helper is the compiler's
  // own construction over the loop's item, and its walk meets no cycle; it
  // is then written back as the `1*A` it was compiled from. With an
  // old-shape star the helper stays a production, as it always has. Its own
  // chain is the helper and its `$step` helpers, which share its own
  // segment; an old-shape star inside the item is the item's, referenced by
  // name, and neither folds nor stops the fold.
  const plusFolds = (name: string): boolean => {
    // A depth-first walk with an explicit stack: `open` holds the rules on
    // the current path, so reaching one again is a cycle; `done` those
    // whose walk has finished, which a second path may reach without one.
    const open = new Set<string>([name])
    const done = new Set<string>()
    let reachedLoop = false
    const stack: [string, string[]][] = [[name, targets(name)]]
    while (0 < stack.length) {
      const [current, pending] = stack[stack.length - 1]
      const target = pending.pop()
      if (undefined === target) {
        open.delete(current)
        done.add(current)
        stack.pop()
        continue
      }
      const own = abnfOwnSegment(current) === abnfOwnSegment(name)
      if (!has(target) || !isSynthetic(target)) continue
      if (loops.has(target) || loopHelpers.has(target)) {
        reachedLoop = reachedLoop || (own && loops.has(target))
        continue
      }
      if (isKeptRepetition(target)) {
        // The plus's own trailing star in the old shape keeps the plus a
        // production; one inside the item stays the item's production.
        if (own) return false
        continue
      }
      if (open.has(target)) return false
      if (done.has(target)) continue
      open.add(target)
      stack.push([target, targets(target)])
    }
    const plus = abnfOwnSegment(name)
    const tail = loopAfter(plus)
    return reachedLoop && null != tail && null != countedByConstruction(plus, tail)
  }

  // A rule rendered where it is referenced and never as a production of its
  // own, unless it is the start rule: a foldable synthetic, a synthetic
  // loop, or a loop's iteration helper. A loop that is a USER rule keeps
  // its production (its body is the repetition).
  const isFolded = (name: string): boolean =>
    isFoldable(name) ||
    (loops.has(name) && isSynthetic(name)) ||
    loopHelpers.has(name)

  // An alternative that does nothing, as the closes of the compiler's loops
  // do: it matches no token, pushes and replaces nothing, and carries no
  // condition.
  const isIdle = (alt: any): boolean =>
    0 === slots(alt).length &&
    0 === back(alt) &&
    null == abnfPush(alt) &&
    null == abnfReplace(alt) &&
    abnfIsPlainWay(alt)

  // `hasContent` for an alt of a rule that is a loop, or is not: only a
  // loop has an entry to skip.
  const hasContentAs = (alt: any, rule: string, ruleIsLoop: boolean): boolean =>
    abnfHasContent(alt, rule, ruleIsLoop, slots(alt), back(alt))
  const hasContent = (alt: any, name: string): boolean =>
    hasContentAs(alt, name, loops.has(name))

  // For each open alternative of the loop `rule`, whether it is a continue
  // that a FOLLOW peek before it covers, and so never runs.
  const shadowedByPeeks = (open: any[], rule: string): boolean[] => {
    const peeks: number[][][] = []
    return open.map((alt) => {
      const s = slots(alt)
      if (!hasContentAs(alt, rule, true)) {
        if (0 < s.length) peeks.push(s)
        return false
      }
      return peeks.some((peek) => abnfCovers(peek, s))
    })
  }

  // ---- Rendering ------------------------------------------------------------

  // The open alternatives with content. An empty one is dropped, because an
  // inlined construct contributes only its content.
  const contentOpens = (name: string): any[] =>
    opensOf(name).filter((alt) => hasContent(alt, name))

  // An alt whose whole sequence is the single end-of-source token.
  const isEndAlt = (alt: any): boolean => {
    if (null == endTin) return false
    const s = slots(alt)
    return 1 === s.length && 1 === s[0].length && endTin === s[0][0]
  }

  // Render one alt as an ABNF element sequence: its `.s` tokens then its
  // `.p`/`.r` target (synthetic targets are inlined).
  //
  // An alt consumes `len(.s) - .b` tokens — the engine records "matched minus
  // backtrack" (parser rules.ts, parse_alts). Tokens beyond that are LOOKAHEAD
  // only: matched to choose the alt, then pushed back. Rendering them as ABNF
  // elements claims input the alt never eats.
  //
  // Deriving the count this way also covers the FIRST-set-guarded epsilon:
  // `{ s: '#Y', b: 1 }` with no target, which @tabnas/abnf emits for the
  // skip branch of an optional, where #Y is the FOLLOW token. Rendered as a
  // consuming alternative, `top = [ X "@" ] Y` came back as
  // `top = [ X T / Y ] Y`: the optional could swallow the follow.
  const seqOfAlt = (alt: any, seen: Set<string>): string => {
    const els: string[] = []
    const s = slots(alt)
    const keep = Math.max(0, s.length - back(alt))
    for (const position of s.slice(0, keep)) {
      if (0 === position.length) continue
      if (1 === position.length) {
        if (position[0] === endTin) continue
        els.push(terminal(position[0]))
        continue
      }
      const inner = position.filter((t) => t !== endTin).map(terminal)
      if (0 < inner.length) els.push('( ' + inner.join(' / ') + ' )')
    }
    const target = abnfTarget(alt)
    if (null != target) {
      const reference = inlineRef(target, seen)
      if ('' !== reference) els.push(reference)
    }
    return els.join(' ')
  }

  // The close-alt continuation of a rule: its trailing element sequence,
  // wrapped in `[ … ]` when an epsilon (empty) close alt makes it optional.
  const closeCont = (name: string, seen: Set<string>): string => {
    const closes = closesOf(name)
    const hasEpsilon = closes.some((a) => !isEndAlt(a) && !hasContent(a, name))
    for (const alt of closes) {
      if (isEndAlt(alt) || !hasContent(alt, name)) continue
      const cont = seqOfAlt(alt, seen)
      if ('' === cont) continue
      return hasEpsilon ? '[ ' + cont + ' ]' : cont
    }
    return ''
  }

  // Open alternatives joined by `/`, then any close continuation. Unlike
  // emitBody an empty open alternative is dropped, because an inlined
  // construct contributes only its content.
  const ruleSeq = (name: string, seen: Set<string>): string => {
    const alts = [
      ...new Set(contentOpens(name).map((a) => seqOfAlt(a, seen)).filter(Boolean)),
    ]
    const cont = closeCont(name, seen)
    return (alts.join(' / ') + ' ' + cont).trim()
  }

  // A loop, as a repetition of its iteration: `*A` when the iteration is one
  // element, `*( a b )` otherwise. The iteration is the ` / `-joined
  // rendering of the continue alternatives, and its back edges render
  // nothing: the loop is in `seen`, so `r: H` terminates like any other
  // loop-back. The entry and the exits have no content and are skipped. Any
  // close continuation of the loop rule runs once, after the last item, and
  // follows the repetition; a close that re-enters a user loop is no back
  // edge but the rule again, and renders as its name.
  const repetition = (name: string, seen: Set<string>): string => {
    const parts = [
      ...new Set(contentOpens(name).map((a) => seqOfAlt(a, seen)).filter(Boolean)),
    ]
    const iteration = abnfRepeatOf(parts)
    let cont: string
    if (isSynthetic(name)) {
      cont = closeCont(name, seen)
    } else {
      const outer = new Set(seen)
      outer.delete(name)
      cont = closeCont(name, outer)
    }
    return (iteration + ' ' + cont).trim()
  }

  // Full production body: open alternatives joined by `/`, then any close
  // continuation, but PRESERVING an empty open alternative, which is
  // essential for kept `*(…)` repetition rules, whose empty alt is what makes
  // them zero-or-more.
  //
  // The empty alternative is rendered by wrapping the rest in `[ … ]`, NOT
  // as a trailing `/`. ABNF's grammar is
  //   alternation = concatenation *(*c-wsp "/" *c-wsp concatenation)
  // so every `/` must be followed by a concatenation: `x = A x /` is a
  // syntax error, and every conforming ABNF tool rejects it. `@tabnas/abnf`
  // happens to accept it, which is exactly why this went unnoticed — the
  // round-trip test passed while the output was unusable anywhere else.
  // `[ A x ]` says the same thing and is valid.
  const emitBody = (name: string, seen: Set<string>): string => {
    // A user rule that is a loop: its production IS the repetition. One
    // that repeats nothing matches exactly the empty string.
    if (loops.has(name)) {
      const body = repetition(name, seen)
      return '' === body ? '""' : body
    }
    const raw = opensOf(name).map((a) => seqOfAlt(a, seen))
    const optional = raw.some((x) => '' === x)
    const nonEmpty = [...new Set(raw.filter(Boolean))]
    const cont = closeCont(name, seen)

    // Nothing but an empty alternative. `option = "[" *c-wsp alternation
    // *c-wsp "]"` and `alternation` needs at least one concatenation, so
    // `[ ]` is not a legal option — there is nothing to make optional.
    //
    // With a continuation, the open contributes nothing and the rule IS its
    // continuation. Without one, `x = ` is not a production at all, and ABNF
    // has no epsilon terminal — but an empty char-val is legal (`char-val =
    // DQUOTE *(%x20-21 / %x23-7E) DQUOTE` permits zero chars) and matches
    // exactly the empty string, which is what this rule does.
    if (optional && 0 === nonEmpty.length) {
      return cont ? cont : '""'
    }

    const joined = nonEmpty.join(' / ')
    const body = optional ? '[ ' + joined + ' ]' : joined
    return (body + ' ' + cont).trim()
  }

  // Inline a reference: a user rule stays a bareword; a synthetic rule folds
  // back into the ABNF construct it encodes; a loop renders as its
  // repetition.
  const inlineRef = (name: string, seen: Set<string>): string => {
    if (loops.has(name)) {
      // The back edge out of the loop's own iteration.
      if (seen.has(name)) return ''
      // A user rule that is a loop keeps its production.
      if (!isSynthetic(name)) return abnfName.rule(name)
      const inner = new Set(seen)
      inner.add(name)
      return repetition(name, inner)
    }
    // A user rule, or a kept (non-foldable, e.g. old-shape repetition)
    // synthetic rule, stays a bareword reference; only foldable synthetics
    // and a loop's iteration helpers inline.
    if (!isFoldable(name) && !loopHelpers.has(name)) return abnfName.rule(name)
    if (seen.has(name)) return '' // a foldable loop-back terminates the loop
    if (!has(name)) return abnfName.rule(name)
    const inner = new Set(seen)
    inner.add(name)
    // The optional's own helper, and only that: a star over an optional is
    // named after it (`_gen3_star__gen2_opt__gen1_group`), and so are its
    // iteration helpers, which a substring test for `_opt` wrapped in
    // `[ … ]` too.
    if (abnfIsHelper(name, 'opt')) {
      return '[ ' + ruleSeq(name, inner) + ' ]'
    }
    const body = ruleSeq(name, inner)
    if (abnfIsHelper(name, 'plus') || abnfIsHelper(name, 'rep')) {
      const counted = countedRepetition(name, inner)
      if (null != counted) return counted
    }
    // group / chain-step: inline the body, parenthesising a bare multi-way
    // alternation that will sit inside a larger sequence.
    const multi = 1 < contentOpens(name).length
    return multi && '' === closeCont(name, inner) ? '( ' + body + ' )' : body
  }

  // `1*A` compiles to a `_plus` helper that is `A` followed by the star of
  // `A`, and `n*A` to a `_rep` helper that is `A` `n` times followed by it.
  // Rendered element by element those read `A *A` and `A A *A`: the same
  // language as `1*A` and `2*A`, but not the same recogniser once
  // recompiled, where `A` is nullable or its FIRST meets its FOLLOW. So a
  // helper whose body is the item of the loop it ends in, `n` times, then
  // that loop's repetition, is written back as the repetition it was
  // compiled from: `1*A`, `1*[ A ]`, `2*( a b )`.
  const countedRepetition = (name: string, seen: Set<string>): string | null => {
    const loopName = loopAfter(name)
    if (null == loopName) return null
    const count = countedByConstruction(name, loopName)
    if (null == count) return null
    const inner = new Set(seen)
    inner.add(loopName)
    const rep = repetition(loopName, inner)
    return rep.startsWith('*') ? count + rep : null
  }

  // How many times the chain of the `_plus` / `_rep` helper `name` takes the
  // item of the loop `tail` it ends in, when it is the compiler's
  // construction and nothing else: each rule of the chain has one plain open
  // alternative, consuming the loop's item token or pushing the loop's item
  // rule (then a close replace to the next step), and the last pushes
  // `tail` and ends.
  const countedByConstruction = (name: string, tail: string): number | null => {
    const item = loopItem(tail)
    if (null == item) return null
    const visited = new Set<string>()
    let current = name
    let count = 0
    for (;;) {
      if (visited.has(current)) return null
      visited.add(current)
      if (!has(current)) return null
      const currentOpens = opensOf(current)
      if (1 !== currentOpens.length) return null
      const [open] = currentOpens
      if (!abnfIsPlainWay(open) || 0 !== back(open) || null != abnfReplace(open)) {
        return null
      }
      for (const slot of slots(open)) {
        if (item !== abnfTokenItem(slot)) return null
        count++
      }
      const pushed = abnfPush(open)
      if (null != pushed && pushed === tail) {
        const ends = closesOf(current).every(
          (alt) =>
            abnfIsPlainWay(alt) &&
            0 === slots(alt).length &&
            null == abnfPush(alt) &&
            null == abnfReplace(alt),
        )
        return ends && 0 < count ? count : null
      } else if (null != pushed && item === abnfRuleItem(pushed)) {
        count++
      } else if (null != pushed) {
        return null
      }
      const currentCloses = closesOf(current)
      if (1 !== currentCloses.length) return null
      const [close] = currentCloses
      if (!abnfIsPlainWay(close) || 0 !== slots(close).length || null != abnfPush(close)) {
        return null
      }
      const next = abnfReplace(close)
      if (null == next) return null
      current = next
    }
  }

  // The one item the loop `rule` repeats, as the compiler builds it: the
  // token its terminal continues consume, or the rule its iteration helper
  // pushes. Null for a loop over anything else.
  const loopItem = (rule: string): string | null => {
    if (!has(rule)) return null
    let item: string | null = null
    for (const alt of opensOf(rule).slice(1)) {
      if (!hasContentAs(alt, rule, true)) continue
      const s = slots(alt)
      let thisItem: string
      if (abnfReplace(alt) === rule && s.length === back(alt) + 1) {
        thisItem = abnfTokenItem(s[0])
      } else if (s.length === back(alt)) {
        const helper = abnfReplace(alt)
        if (null == helper || !has(helper)) return null
        const helperOpens = opensOf(helper)
        if (1 !== helperOpens.length) return null
        const [open] = helperOpens
        if (0 !== slots(open).length || null != abnfReplace(open) || !abnfIsPlainWay(open)) {
          return null
        }
        const pushed = abnfPush(open)
        if (null == pushed) return null
        thisItem = abnfRuleItem(pushed)
      } else {
        return null
      }
      if (null == item) item = thisItem
      else if (item !== thisItem) return null
    }
    return item
  }

  // The loop a `_plus` / `_rep` helper ends in: the open target of the last
  // rule of its chain (linked by their close replaces), when that target is
  // a loop. The chain is followed by its close edges only, never into the
  // pushed item, which may hold a loop of its own.
  const loopAfter = (name: string): string | null => {
    const visited = new Set<string>()
    let current = name
    for (;;) {
      if (visited.has(current)) return null
      visited.add(current)
      if (!has(current)) return null
      const next = closesOf(current).map(abnfReplace).find((r) => null != r)
      if (null != next && has(next)) current = next
      else break
    }
    if (!has(current)) return null
    const target = opensOf(current).map(abnfTarget).find((t) => null != t)
    return null != target && loops.has(target) ? target : null
  }

  // The loops, decided by shape, then the helpers their iterations run
  // through. `loopHelpers` is empty while the helpers are found, as it is
  // in the Rust port.
  let loops = new Set<string>()
  let loopHelpers = new Set<string>()
  loops = new Set(Object.keys(rules).filter((name) => isLoop(name)))
  loopHelpers = findLoopHelpers()

  // Order: real start first, then the remaining USER (non-synthetic) rules.
  // The start rule is always a production, folded or not: nothing encloses
  // it to render it where it is referenced, and a grammar whose start is a
  // synthetic loop (a standalone `*A`) otherwise came out with no
  // production at all.
  const userRules = Object.keys(rules).filter(
    (rn) => rn !== synthWrapper && !isFolded(rn),
  )
  const ordered: string[] = []
  const seenR = new Set<string>()
  if (startRule && rules[startRule]) {
    ordered.push(startRule)
    seenR.add(startRule)
  }
  for (const rn of userRules) {
    if (!seenR.has(rn)) {
      ordered.push(rn)
      seenR.add(rn)
    }
  }

  const lines: string[] = []
  for (const rn of ordered) {
    const body = emitBody(rn, new Set([rn]))
    lines.push(abnfName.rule(rn) + ' = ' + body)
  }

  // Define each token as its own ABNF rule (named terminals), after the
  // productions, with `=` aligned for readability.
  if (0 < used.size) {
    const pad = Math.max(...[...used.keys()].map((n) => n.length))
    lines.push('')
    for (const [name, form] of used) {
      lines.push(name.padEnd(pad) + ' = ' + form)
    }
  }
  return lines.join('\n')
}

// ---- Shape predicates of the ABNF emitter, read from one alternative ------

// The token slots an alternative matches, one array of tins per position,
// as the engine normalised them (`t`): a position with several tokens
// matches any of them. Read from `s` where `t` is absent.
function abnfSlots(alt: any, toTin: (t: any) => number | undefined): number[][] {
  if (Array.isArray(alt.t)) {
    return alt.t.map((slot: any) =>
      (Array.isArray(slot) ? slot : [slot]).filter((t: any) => 'number' === typeof t),
    )
  }
  const all: any[] = Array.isArray(alt.s) ? alt.s : null == alt.s ? [] : [alt.s]
  return all.map((item: any) =>
    (Array.isArray(item) ? item : [item])
      .map(toTin)
      .filter((t: any): t is number => null != t),
  )
}

// How many matched tokens an alternative gives back: `b`, where `true`
// gives back every one. A backtrack a function decides counts as none
// here, and makes the alternative dynamic (`abnfIsDynamic`).
function abnfBack(alt: any, slotCount: number): number {
  return null == alt.b
    ? 0
    : true === alt.b
      ? slotCount
      : 'number' === typeof alt.b
        ? alt.b
        : 0
}

function abnfPush(alt: any): string | null {
  return 'string' === typeof alt.p ? alt.p : null
}

function abnfReplace(alt: any): string | null {
  return 'string' === typeof alt.r ? alt.r : null
}

// The rule an alternative hands control to: its push, else its replace.
function abnfTarget(alt: any): string | null {
  return abnfPush(alt) ?? abnfReplace(alt)
}

function abnfCounters(alt: any): Record<string, any> {
  return null != alt.n && 'object' === typeof alt.n ? alt.n : {}
}

// The one item a loop repeats, as a comparable key: a token slot its
// continue consumes, or the rule its iteration helper pushes.
function abnfTokenItem(slot: number[]): string {
  return 'token:' + slot.join(',')
}

function abnfRuleItem(name: string): string {
  return 'rule:' + name
}

// An alternative whose route, backtrack or whole shape a function decides
// when it matches, so what it pushes, replaces or consumes cannot be read
// from the spec.
function abnfIsDynamic(alt: any): boolean {
  return (
    'function' === typeof alt.p ||
    'function' === typeof alt.r ||
    'function' === typeof alt.b ||
    null != alt.h
  )
}

// The counter a condition tests against zero, when it is the engine's own
// declarative `{ 'n.<counter>': 0 }` and nothing else; null otherwise.
//
// The engine compiles a declarative condition to a closure (`ruleCond`, or
// `conjunctCond` for several) and keeps no description of it, so the guard
// is read from what the closure does, which is what makes it the guard: run
// on probe rules, it reads exactly one path, `n.<counter>`, and nothing of
// the context, and it holds when that counter is unset (an unset counter
// reads as 0) or 0, and not when it is 1 or -1. Among the declarative
// operators only `$eq 0` does all four. A conjunction and a closure that
// reads anything else are not the guard, and neither is a condition the
// grammar wrote as a function, whatever it does: the Rust port never reads
// one (`c_fn`) as the guard, since a function may decide by anything. The
// name does not tell the two apart (the engine names a grammar's anonymous
// condition `ruleCond` too), the source does: the engine's own closure is
// declared `function ruleCond(`. The Rust port reads its declarative
// `Condition` directly; this is the same test.
function abnfZeroGuardCounter(c: any): string | null {
  if (
    'function' !== typeof c ||
    'ruleCond' !== c.name ||
    !Function.prototype.toString.call(c).startsWith('function ruleCond(')
  ) {
    return null
  }
  const read: PropertyKey[] = []
  const counters = new Proxy({}, {
    get: (_t, key) => { read.push(key); return undefined },
  })
  const probe = new Proxy({}, {
    get: (_t, key) => { read.push(key); return 'n' === key ? counters : undefined },
  })
  const ctx = new Proxy({}, {
    get: (_t, key) => { read.push(key); return undefined },
  })
  let unset: any
  try {
    unset = c(probe, ctx, undefined)
  } catch (e) {
    return null
  }
  if (2 !== read.length || 'n' !== read[0] || 'string' !== typeof read[1]) {
    return null
  }
  const counter = read[1] as string
  const holds = (value: number): any => {
    try {
      return c({ n: { [counter]: value } }, {}, undefined)
    } catch (e) {
      return undefined
    }
  }
  return true === unset &&
    true === holds(0) &&
    false === holds(1) &&
    false === holds(-1)
    ? counter
    : null
}

// An alternative read from the spec alone, and taken whatever the rule's
// state, with no condition of any kind.
function abnfIsPlainWay(alt: any): boolean {
  return !abnfIsDynamic(alt) && null == alt.c
}

// A continue guarded as the compiler guards one: by nothing, or by the
// suffix-debt counter alone (`n.debt_… == 0`). Any other condition may
// contradict the state the entry leaves and keep the continue from ever
// running.
function abnfIsGuardedAsCompiled(alt: any): boolean {
  return (
    !abnfIsDynamic(alt) &&
    (null == alt.c || (abnfZeroGuardCounter(alt.c) || '').startsWith('debt_'))
  )
}

// An alternative on a loop's way back: a plain way that sets no counter but
// the loop's own `rep`.
function abnfIsHelperWay(alt: any): boolean {
  return abnfIsPlainWay(alt) && Object.keys(abnfCounters(alt)).every((k) => 'rep' === k)
}

// A repeat loop's entry, the whole of the compiler's shape: the alternative
// matches no token, not even a peeked one, pushes nothing, replaces `rule`
// with itself, is guarded by `n.rep == 0` and by nothing else, and sets that
// counter to 1 and no other. `s`, `b`, `p` and `r` alone are not enough: a
// user rule's own non-consuming self-replace has the same four and is no
// repetition.
function abnfIsLoopEntry(alt: any, rule: string, slots: number[][], back: number): boolean {
  const counters = abnfCounters(alt)
  return (
    0 === slots.length &&
    0 === back &&
    null == abnfPush(alt) &&
    abnfReplace(alt) === rule &&
    1 === Object.keys(counters).length &&
    1 === counters.rep &&
    'rep' === abnfZeroGuardCounter(alt.c)
  )
}

// An alt that contributes something to the emitted sequence of `rule`,
// decided by what it CONSUMES: it eats a token (`len(s) - b > 0`), or
// pushes a rule, or replaces with a rule, unless `rule` is a loop and this
// is its entry. `{ }`, the FOLLOW peek `{ s: FOLLOW, b: 1 }` and a loop's
// entry are all epsilon. Every other replace with `rule` itself is content:
// the close `{ s: A, b: 1, r: rule }` after an open that consumed `A` is the
// `[ rule ]` of `rule = A [ rule ]`.
function abnfHasContent(
  alt: any,
  rule: string,
  ruleIsLoop: boolean,
  slots: number[][],
  back: number,
): boolean {
  return (
    !(ruleIsLoop && abnfIsLoopEntry(alt, rule, slots, back)) &&
    (slots.length > back || null != abnfPush(alt) || null != abnfReplace(alt))
  )
}

// The token sequence `peek` matches wherever `item` does: it is no longer,
// and each of its slots holds every token `item`'s does.
function abnfCovers(peek: number[][], item: number[][]): boolean {
  return (
    peek.length <= item.length &&
    peek.every((slot, i) => item[i].every((tin) => slot.includes(tin)))
  )
}

// A rule the abnf forward-compiler synthesised, named `_gen<n>_…`.
function abnfIsGenName(name: string): boolean {
  return /^_gen\d/.test(name)
}

// The rule a synthesised name belongs to: the part before any `$`. A chain
// step (`_gen1_group$step1`) and an iteration helper (`H$alt0`) answer with
// the rule they continue.
function abnfOwnSegment(name: string): string {
  return name.split('$')[0]
}

// The construct a synthesised name encodes (`opt`, `group`, `star`, `plus`,
// `rep`), read from the rule's OWN segment: the word after `_gen<n>_` in the
// part before any `$`. A repetition's helper is named after its item, so
// `_gen3_star__gen2_opt__gen1_group` is the star over the optional over the
// group, and a substring test for `_opt` reached all of them.
function abnfGenKind(name: string): string | null {
  const own = abnfOwnSegment(name)
  if (!own.startsWith('_gen')) return null
  const rest = own.slice(4)
  const digits = /^\d+/.exec(rest)
  if (null == digits) return null
  const after = rest.slice(digits[0].length)
  if (!after.startsWith('_')) return null
  const kind = after.slice(1).split('_')[0]
  return '' === kind ? null : kind
}

// `name` is the helper of `kind` itself, not a chain step or an iteration
// helper of it, which carry a `$`.
function abnfIsHelper(name: string, kind: string): boolean {
  return !name.includes('$') && abnfGenKind(name) === kind
}

// A repetition over the ` / `-joined alternatives of an iteration: `*A` and
// `*"a"` when the iteration is one element, `*( a b )` and `*( a / b )`
// otherwise. An empty iteration is an empty repetition: nothing.
function abnfRepeatOf(parts: string[]): string {
  if (0 === parts.length) return ''
  if (1 === parts.length && abnfIsOneElement(parts[0])) return '*' + parts[0]
  return '*( ' + parts.join(' / ') + ' )'
}

// `text` is one ABNF element: a bare name or terminal, or one bracket pair
// enclosing the whole of it. A repetition is not an element
// (`repetition = [repeat] element`), so a nested `*I` has to be grouped:
// `*( *I )`, never `**I`.
function abnfIsOneElement(text: string): boolean {
  if (/^[*0-9]/.test(text)) return false
  if (!text.includes(' ')) return true
  const open = text[0]
  const close = text[text.length - 1]
  if (!(('(' === open && ')' === close) || ('[' === open && ']' === close))) {
    return false
  }
  // The opening bracket must be the one the last character closes.
  let depth = 0
  for (let index = 0; index < text.length; index++) {
    const ch = text[index]
    if ('(' === ch || '[' === ch) depth++
    else if (')' === ch || ']' === ch) {
      depth = Math.max(0, depth - 1)
      if (0 === depth && index + 1 < text.length) return false
    }
  }
  return 0 === depth
}

// Render a token reference: every token appears by its bare NAME (e.g.
// '#PL' -> 'PL', '#NR' -> 'NR'), and its definition is recorded in `used`
// for the comment legend. A token name that is actually a rule name is a
// nonterminal reference and is returned as-is (no legend entry).
function emitAbnfTerminal(
  tabnas: Tabnas,
  cfg: Config,
  tin: number,
  used: Map<string, string>,
  abnfName: { rule: (name: string) => string; token: (bare: string) => string },
): string {
  const fullName: string = tabnas.token[tin]

  const rules: any = tabnas.rule()
  if (fullName && rules[fullName]) {
    return abnfName.rule(fullName)
  }

  // Strip the '#' sigil first so '#NR' asks for 'NR' rather than being
  // sanitised to '-NR' and then prefixed.
  //
  // This goes through the TOKEN namespace. A rule may already hold this
  // spelling — a grammar with a rule `NR` and the `#NR` number token is
  // perfectly ordinary — and the token must not borrow it: that emitted a
  // self-referential `NR = NR` plus a duplicate definition. Asking the token
  // namespace suffixes to `NR-2` instead, leaving both definitions distinct.
  const name = abnfName.token((fullName || 'T' + tin).replace(/^#/, ''))
  if (!used.has(name)) {
    used.set(name, abnfTokenForm(cfg, tin, fullName))
  }
  return name
}

// The legend definition for a token — what it matches:
//   - fixed literal       -> %s"<lit>" (letters) / "<lit>" (punctuation)
//   - /^<lit>/i (letters) -> "<lit>"     (case-insensitive literal)
//   - /^[\uXXXX-\uYYYY]/  -> %xXX-YY     (char range)
//   - built-in matcher    -> <number> / <string> / ...   (lexer-provided)
//
// The `%s` prefix is RFC 7405, which updates RFC 5234 — it is the only
// construct emitted here that RFC 5234 alone does not define. It is kept
// because it is what every current ABNF tool implements and it stays
// readable; `%x48.69` would be pure RFC 5234 but unreadable, and the
// alternative (a bare char-val) would silently lose case-sensitivity.
function abnfTokenForm(cfg: Config, tin: number, fullName: string): string {
  const fixedLit = (cfg.fixed.ref as any)[tin]
  if ('string' === typeof fixedLit) {
    // RFC 5234: char-val = DQUOTE *(%x20-21 / %x23-7E) DQUOTE — so a
    // literal holding a control character, a `"`, or anything above %x7E
    // CANNOT go inside quotes. A token fixed to CRLF used to emit
    // `CRLF = "<CR>"`, an unterminated char-val. Fall back to the numeric
    // form, which has no such restriction (and is case-sensitive already,
    // so it carries the `%s` meaning too).
    return /^[\x20\x21\x23-\x7E]*$/.test(fixedLit)
      ? (/[A-Za-z]/.test(fixedLit) ? '%s"' + fixedLit + '"' : '"' + fixedLit + '"')
      : numericVal(fixedLit)
  }

  const re = (cfg.match.token as any)[tin] ?? (cfg.match.token as any)['' + tin]
  if (re instanceof RegExp) {
    return regexToAbnf(re)
  }

  // Built-in lexer token: describe it (it is lexer-provided, so a grammar
  // using it does not round-trip through bnf).
  const bare = (fullName || '' + tin).replace(/^#/, '')
  const desc: Record<string, string> = {
    NR: 'number',
    ST: 'string',
    TX: 'text',
    VL: 'value',
    SP: 'space',
    LN: 'line',
    CM: 'comment',
    AA: 'any',
    UK: 'unknown',
    BD: 'bad',
    ZZ: 'end-of-source',
  }
  return proseVal(desc[bare] || 'built-in ' + bare)
}

// A literal as an ABNF num-val: `%x0D`, or dot-concatenated for several
// characters (`%x0D.0A`). RFC 5234 gives this no character restriction, so
// it is the safe rendering for anything char-val cannot hold.
function numericVal(lit: string): string {
  return (
    '%x' +
    [...lit]
      .map((ch) => {
        const cp = ch.codePointAt(0) as number
        const hex = cp.toString(16).toUpperCase()
        return hex.length % 2 ? '0' + hex : hex
      })
      .join('.')
  )
}

// Translate the anchored RegExp bnf installs for a match token back to
// ABNF, covering the two shapes bnf actually emits.
function regexToAbnf(re: RegExp): string {
  // Drop the leading anchor bnf always prepends.
  let src = re.source
  if ('^' === src[0]) src = src.slice(1)

  // Single char-class range: [\uXXXX-\uYYYY]  ->  %xXX-YY
  const range = src.match(
    /^\[\\u([0-9A-Fa-f]{4})-\\u([0-9A-Fa-f]{4})\]$/,
  )
  if (range) {
    const lo = parseInt(range[1], 16).toString(16).toUpperCase()
    const hi = parseInt(range[2], 16).toString(16).toUpperCase()
    return '%x' + lo + '-' + hi
  }

  // Single char-class range with bare hex escapes: [\xXX-\xYY].
  const range2 = src.match(/^\[\\x([0-9A-Fa-f]{2})-\\x([0-9A-Fa-f]{2})\]$/)
  if (range2) {
    return (
      '%x' +
      parseInt(range2[1], 16).toString(16).toUpperCase() +
      '-' +
      parseInt(range2[2], 16).toString(16).toUpperCase()
    )
  }

  // Case-insensitive literal: bnf encodes a bare ABNF string `"foo"`
  // that contains at least one letter as `/^<escaped-foo>/i`, where
  // <escaped-foo> escapes the regex metacharacters \ ^ $ . * + ? ( ) [
  // ] { } |. Recover the literal by unescaping, then verify the
  // round-trip so we never misread a genuine regex as a literal.
  if (re.flags.includes('i')) {
    // Unescape both the metacharacters bnf escapes and the forward
    // slash that RegExp.prototype.source escapes automatically.
    const lit = src.replace(/\\([\\^$.*+?()[\]{}|/])/g, '$1')
    // Validate: re-encode the candidate exactly as bnf would and confirm
    // the resulting RegExp source matches, so a real regex is never
    // mistaken for a literal.
    const reEncoded = new RegExp('^' + escapeRegExpLike(lit), 'i').source
    if (reEncoded === re.source && isAbnfQuotable(lit)) {
      return '"' + lit + '"'
    }
  }

  // Anything else: no ABNF construct expresses this regex, so say so in the
  // one the grammar provides for exactly that — RFC 5234 §4 prose-val, "a
  // last resort" for describing a rule in prose. It does not round-trip, and
  // it is not meant to; it is a legal element that names what the token
  // matches.
  //
  // This returned `'; /' + source + '/' + flags` before: a bare comment. `;`
  // runs to end of line, so the legend entry it produced (`T = ; /…/`) held
  // no elements at all — not merely non-round-tripping but unparseable, and
  // one such token made the WHOLE emitted grammar invalid rather than just
  // that rule.
  return proseVal('regex /' + re.source + '/' + re.flags)
}

// RFC 5234 §4: `prose-val = "<" *(%x20-3D / %x3F-7E) ">"`. A `>` would close
// the value early and anything outside printable ASCII is not permitted, so
// both are escaped rather than dropped — the text is here to say what the
// token matches, and silently losing characters from it would defeat that.
function proseVal(text: string): string {
  let out = ''
  for (const ch of text) {
    const cp = ch.codePointAt(0) as number
    out +=
      (0x20 <= cp && cp <= 0x3d) || (0x3f <= cp && cp <= 0x7e)
        ? ch
        : '\\u' + cp.toString(16).toUpperCase().padStart(4, '0')
  }
  return '<' + out + '>'
}

// Mirror of bnf's escapeRegExp, used only to validate that an unescaped
// candidate literal re-escapes to exactly the observed regex source.
function escapeRegExpLike(s: string): string {
  return s.replace(/[\\^$.*+?()[\]{}|]/g, '\\$&')
}

// An ABNF char-val (quoted string) may hold printable ASCII except the
// double quote: %x20-21 / %x23-7E.
function isAbnfQuotable(s: string): boolean {
  if (0 === s.length) return false
  for (let i = 0; i < s.length; i++) {
    const c = s.charCodeAt(i)
    if (0x22 === c) return false // double quote
    if (c < 0x20 || c > 0x7e) return false
  }
  return true
}

function descAlt(tabnas: Tabnas, rs: RuleSpec, kind: 'open' | 'close') {
  const { entries } = tabnas.util

  return 0 === rs.def[kind].length
    ? ''
    : '    ' +
    kind.toUpperCase() +
    ':\n' +
    rs.def[kind]
      .map(
        (a: any, i: number) =>
          '      ' +
          ('' + i).padStart(5, ' ') +
          ' ' +
          (
            '[' +
            (a.s || [])
              .map((tin: any) =>
                null == tin
                  ? '***INVALID***'
                  : 'number' === typeof tin
                    ? tabnas.token[tin]
                    : Array.isArray(tin) ? '[' + tin.map((t: any) => tabnas.token[t]) + ']'
                      : ('' + tin),
              )
              .join(' ') +
            '] '
          ).padEnd(32, ' ') +
          (a.r ? ' r=' + ('string' === typeof a.r ? a.r : '<F>') : '') +
          (a.p ? ' p=' + ('string' === typeof a.p ? a.p : '<F>') : '') +
          (!a.r && !a.p ? '\t' : '') +
          '\t' +
          (null == a.b ? '' : 'b=' + a.b) +
          '\t' +
          (null == a.n
            ? ''
            : 'n=' +
            entries(a.n).map(([k, v]: [string, any]) => k + ':' + v)) +
          '\t' +
          (null == a.a ? '' : 'A') +
          (null == a.c ? '' : 'C') +
          (null == a.h ? '' : 'H') +
          '\t' +
          (null == a.c?.n
            ? '\t'
            : ' CN=' +
            entries(a.c.n).map(([k, v]: [string, any]) => k + ':' + v)) +
          (null == a.c?.d ? '' : ' CD=' + a.c.d) +
          // a.g is normalised by the engine to a (possibly empty) string[].
          (a.g && a.g.length ? '\tg=' + a.g.join(',') : ''),
      )
      .join('\n') +
    '\n'
}

function ruleTree(tabnas: Tabnas, rn: string[], rsm: any) {
  const { values, omap } = tabnas.util

  return rn.reduce(
    (a: any, n: string) => (
      (a +=
        '  ' +
        n +
        ':\n    ' +
        values(
          omap(
            {
              op: ruleTreeStep(rsm, n, 'open', 'p'),
              or: ruleTreeStep(rsm, n, 'open', 'r'),
              cp: ruleTreeStep(rsm, n, 'close', 'p'),
              cr: ruleTreeStep(rsm, n, 'close', 'r'),
            },
            // Drop only truly-empty categories (ruleTreeStep returns '' for
            // those); 0 < length keeps single-character rule-name targets.
            ([n, d]: [string, string]) => [
              0 < d.length ? n : undefined,
              n + ': ' + d,
            ],
          ),
        ).join('\n    ') +
        '\n'),
      a
    ),
    '',
  )
}

function ruleTreeStep(
  rsm: any,
  name: string,
  state: 'open' | 'close',
  step: 'p' | 'r',
) {
  return [
    ...new Set(
      rsm[name].def[state]
        .filter((alt: any) => alt[step])
        .map((alt: any) => alt[step])
        .map((step: any) => ('string' === typeof step ? step : '<F>')),
    ),
  ].join(' ')
}

// Structured form of a single alternate (the data behind descAlt's text).
function altInfo(tabnas: Tabnas, a: any): DebugAltInfo {
  const seq: (string | string[])[] = (a.s || []).map((tin: any) =>
    null == tin
      ? '***INVALID***'
      : 'number' === typeof tin
        ? tabnas.token[tin]
        : Array.isArray(tin)
          ? tin.map((t: any) => ('number' === typeof t ? tabnas.token[t] : String(t)))
          : String(tin),
  )
  const info: DebugAltInfo = {
    seq,
    groups: a.g && a.g.length ? a.g.slice() : [],
    action: null != a.a,
    cond: null != a.c,
    modifier: null != a.h,
  }
  if ('string' === typeof a.p) info.push = a.p
  else if (a.p) info.push = '<fn>'
  if ('string' === typeof a.r) info.replace = a.r
  else if (a.r) info.replace = '<fn>'
  if (null != a.b) info.back = a.b
  if (null != a.n) info.counters = a.n
  return info
}

// The distinct push/replace rule targets of a rule's open/close alts —
// the structured form of ruleTreeStep (array instead of a joined string).
function ruleEdges(
  rsm: any,
  name: string,
  state: 'open' | 'close',
  step: 'p' | 'r',
): string[] {
  return [
    ...new Set(
      rsm[name].def[state]
        .filter((alt: any) => alt[step])
        .map((alt: any) => ('string' === typeof alt[step] ? alt[step] : '<fn>')),
    ),
  ] as string[]
}

function descTokenState(ctx: Context) {
  return (
    '[' +
    (ctx.NOTOKEN === ctx.t0 ? '' : ctx.F(ctx.t0.src)) +
    (ctx.NOTOKEN === ctx.t1 ? '' : ' ' + ctx.F(ctx.t1.src)) +
    ']~[' +
    (ctx.NOTOKEN === ctx.t0 ? '' : tokenize(ctx.t0.tin, ctx.cfg)) +
    (ctx.NOTOKEN === ctx.t1 ? '' : ' ' + tokenize(ctx.t1.tin, ctx.cfg)) +
    ']'
  )
}

function descParseState(ctx: Context, rule: Rule, lex: Lex) {
  return (
    ctx.F(ctx.src().substring(lex.pnt.sI, lex.pnt.sI + 16)).padEnd(18, ' ') +
    ' ' +
    descTokenState(ctx).padEnd(34, ' ') +
    ' ' +
    ('' + rule.d).padStart(4, ' ')
  )
}

function descRuleState(ctx: Context, rule: Rule) {
  let en = entries(rule.n)
  let eu = entries(rule.u)
  let ek = entries(rule.k)

  return (
    '' +
    (0 === en.length
      ? ''
      : ' N<' +
      en
        .filter((n: any) => n[1])
        .map((n: any) => n[0] + '=' + n[1])
        .join(';') +
      '>') +
    (0 === eu.length
      ? ''
      : ' U<' + eu.map((u: any) => u[0] + '=' + ctx.F(u[1])).join(';') + '>') +
    (0 === ek.length
      ? ''
      : ' K<' + ek.map((k: any) => k[0] + '=' + ctx.F(k[1])).join(';') + '>')
  )
}

function descAltSeq(alt: NormAltSpec, cfg: Config) {
  return (
    '[' +
    (alt.s || [])
      .map((tin: any) =>
        'number' === typeof tin
          ? tokenize(tin, cfg)
          : Array.isArray(tin)
            ? '[' + tin.map((t: any) => tokenize(t, cfg)) + ']'
            : '',
      )
      .join(' ') +
    '] '
  )
}

const LOG = {
  RuleState: {
    o: S.open.toUpperCase(),
    c: S.close.toUpperCase(),
  },
}

const LOGKIND: any = {
  step: (...rest: any[]) => rest,

  stack: (ctx: Context, rule: Rule, lex: Lex) => [
    S.logindent + S.stack,
    descParseState(ctx, rule, lex),

    // S.indent.repeat(Math.max(rule.d + ('o' === rule.state ? -1 : 1), 0)) +
    S.indent.repeat(rule.d) +
    '/' +
    ctx.rs
      // .slice(0, ctx.rsI)
      .slice(0, rule.d)
      .map((r: Rule) => r.name + '~' + r.i)
      .join('/'),

    '~',

    '/' +
    ctx.rs
      // .slice(0, ctx.rsI)
      .slice(0, rule.d)
      .map((r: Rule) => ctx.F(r.node))
      .join('/'),

    // 'd=' + rule.d,
    //'rsI=' + ctx.rsI,

    ctx,
    rule,
    lex,
  ],

  rule: (ctx: Context, rule: Rule, lex: Lex) => [
    rule,
    ctx,
    lex,

    S.logindent + S.rule + S.space,
    descParseState(ctx, rule, lex),

    S.indent.repeat(rule.d) +
    (rule.name + '~' + rule.i + S.colon + LOG.RuleState[rule.state]).padEnd(
      16,
    ),

    (
      'prev=' +
      rule.prev.i +
      ' parent=' +
      rule.parent.i +
      ' child=' +
      rule.child.i
    ).padEnd(28),

    descRuleState(ctx, rule),
  ],

  node: (ctx: Context, rule: Rule, lex: Lex, next: Rule) => [
    rule,
    ctx,
    lex,
    next,

    S.logindent + S.node + S.space,
    descParseState(ctx, rule, lex),

    S.indent.repeat(rule.d) +
    ('why=' + next.why + S.space + '<' + ctx.F(rule.node) + '>').padEnd(46),

    descRuleState(ctx, rule),
  ],

  parse: (
    ctx: Context,
    rule: Rule,
    lex: Lex,
    match: boolean,
    cond: boolean,
    altI: number,
    alt: NormAltSpec | null,
    out: AltMatch,
  ) => {
    let ns = match && out.n ? entries(out.n) : null
    let us = match && out.u ? entries(out.u) : null
    let ks = match && out.k ? entries(out.k) : null

    return [
      ctx,
      rule,
      lex,

      S.logindent + S.parse,
      descParseState(ctx, rule, lex),
      S.indent.repeat(rule.d) + (match ? 'alt=' + altI : 'no-alt'),

      match && alt ? descAltSeq(alt, ctx.cfg) : '',

      match && out.g && out.g.length ? 'g:' + out.g.join(',') + ' ' : '',
      (match && out.p ? 'p:' + out.p + ' ' : '') +
      (match && out.r ? 'r:' + out.r + ' ' : '') +
      (match && out.b ? 'b:' + out.b + ' ' : ''),

      alt && alt.c ? 'c:' + cond : EMPTY,
      null == ns ? '' : 'n:' + ns.map((p: any) => p[0] + '=' + p[1]).join(';'),

      null == us ? '' : 'u:' + us.map((p: any) => p[0] + '=' + p[1]).join(';'),

      null == ks ? '' : 'k:' + ks.map((p: any) => p[0] + '=' + p[1]).join(';'),
    ]
  },

  lex: (
    ctx: Context,
    rule: Rule,
    lex: Lex,
    pnt: Point,
    sI: number,
    match: LexMatcher | undefined,
    tkn: Token,
    alt?: NormAltSpec,
    altI?: number,
    tI?: number,
  ) => [
      S.logindent + S.lex + S.space + S.space,
      descParseState(ctx, rule, lex),
      S.indent.repeat(rule.d) +
      // S.indent.repeat(rule.d) + S.lex, // Log entry prefix.

      // Name of token from tin (token identification numer).
      tokenize(tkn.tin, ctx.cfg),

      ctx.F(tkn.src), // Format token src for log.
      pnt.sI, // Current source index.
      pnt.rI + ':' + pnt.cI, // Row and column.
      match?.name || '',

      alt
        ? 'on:alt=' +
        altI +
        ';' +
        (alt.g || []).join(',') +
        ';t=' +
        tI +
        ';' +
        descAltSeq(alt, ctx.cfg)
        : '',

      ctx.F(lex.src.substring(sI, sI + 16)),

      ctx,
      rule,
      lex,
    ],
}

Debug.defaults = DEFAULTS as DebugOptions

// VERSION is this package's version. It MUST equal package.json "version":
// the release orchestrator rewrites both, and the version test fails the
// build if they drift. Mirrors `const VERSION` in go/debug.go.
const VERSION = '0.3.13'

export { VERSION, Debug }
