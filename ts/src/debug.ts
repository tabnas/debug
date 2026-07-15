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
function emitAbnf(tabnas: Tabnas): string {
  const cfg = tabnas.internal().config
  const rules = tabnas.rule() as Record<string, RuleSpec>

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

  // A rule the abnf forward-compiler synthesised for a `[...]` / `*(...)` /
  // `1*(...)` / group / chain-step: named `_gen<n>_…` or carrying a `$`.
  // These are never user-authored, so instead of emitting them as their own
  // productions we fold each back into the ABNF construct it encodes — a
  // reasonable round-trip (`tn.abnf(G)` then `debug.abnf()` reproduces `G`,
  // not the expanded internal form).
  const isSynthetic = (name: string): boolean =>
    name !== synthWrapper && (/^_gen\d/.test(name) || name.includes('$'))

  // We only fold the clean cases — `[…]` optionals plus the group / chain
  // helpers they inline through. Repetition (`_star` / `_plus`) uses a
  // probe-optimised subgraph that does not reconstruct reliably, so those
  // rules (and their `$alt…` helpers) are emitted as productions unchanged
  // (still a valid, recognition-equivalent grammar).
  const isFoldable = (name: string): boolean =>
    isSynthetic(name) && !/_star|_plus|\$alt/.test(name)

  const used = new Map<string, string>()

  const hasContent = (alt: any): boolean =>
    (Array.isArray(alt.s) ? 0 < alt.s.length : null != alt.s) ||
    'string' === typeof alt.p ||
    'string' === typeof alt.r
  const contentOpens = (rs: any): any[] => (rs.def.open || []).filter(hasContent)

  // Render one alt as an ABNF element sequence: its `.s` tokens then its
  // `.p`/`.r` target (synthetic targets are inlined). A `b`+push alt peeks
  // its `.s` tokens (the pushed rule consumes them) — skip them.
  const seqOfAlt = (alt: any, seen: Set<string>): string => {
    const els: string[] = []
    const peekOnly =
      alt.b && ('string' === typeof alt.p || 'string' === typeof alt.r)
    const seq: any[] = peekOnly
      ? []
      : Array.isArray(alt.s)
        ? alt.s
        : null == alt.s
          ? []
          : [alt.s]
    for (const item of seq) {
      if (null == item) continue
      if (Array.isArray(item)) {
        const inner = item
          .map(toTin)
          .filter((t: any): t is number => null != t && t !== endTin)
          .map((t: number) => emitAbnfTerminal(tabnas, cfg, t, used))
        if (0 < inner.length) els.push('( ' + inner.join(' / ') + ' )')
        continue
      }
      const tin = toTin(item)
      if (null == tin || tin === endTin) continue
      els.push(emitAbnfTerminal(tabnas, cfg, tin, used))
    }
    const target =
      'string' === typeof alt.p ? alt.p : 'string' === typeof alt.r ? alt.r : null
    if (target) els.push(inlineRef(target, seen))
    return els.join(' ')
  }

  // The close-alt continuation of a rule: its trailing element sequence,
  // wrapped in `[ … ]` when an epsilon (empty) close alt makes it optional.
  const closeCont = (rs: any, seen: Set<string>): string => {
    const closes: any[] = rs.def.close || []
    const isEnd = (alt: any) => {
      const first =
        Array.isArray(alt.s) && 1 === alt.s.length ? toTin(alt.s[0]) : undefined
      return null != endTin && first === endTin
    }
    const hasEpsilon = closes.some(
      (a) => !isEnd(a) && !hasContent(a),
    )
    for (const alt of closes) {
      if (isEnd(alt) || !hasContent(alt)) continue
      const cont = seqOfAlt(alt, seen)
      if (!cont) continue
      return hasEpsilon ? '[ ' + cont + ' ]' : cont
    }
    return ''
  }

  // Full ABNF for a rule body: open alternatives joined by `/`, then any
  // close continuation.
  const ruleSeq = (rs: any, seen: Set<string>): string => {
    const alts = [
      ...new Set(contentOpens(rs).map((a: any) => seqOfAlt(a, seen)).filter(Boolean)),
    ]
    return (alts.join(' / ') + ' ' + closeCont(rs, seen)).trim()
  }

  // Full production body: like ruleSeq, but PRESERVES an empty open
  // alternative (rendered as a trailing `/`) — essential for kept `*(…)`
  // repetition rules, whose empty alt is what makes them zero-or-more.
  const emitBody = (rs: any, seen: Set<string>): string => {
    const raw = (rs.def.open || []).map((a: any) => seqOfAlt(a, seen))
    const nonEmpty = [...new Set(raw.filter(Boolean))]
    const parts = raw.some((x: string) => '' === x)
      ? [...nonEmpty, '']
      : nonEmpty
    return (parts.join(' / ') + ' ' + closeCont(rs, seen)).trim()
  }

  // Inline a reference: a user rule stays a bareword; a synthetic rule folds
  // back into the ABNF construct it encodes.
  const inlineRef = (name: string, seen: Set<string>): string => {
    // A user rule or a kept (non-foldable, e.g. repetition) synthetic rule
    // stays a bareword reference; only foldable synthetics are inlined.
    if (!isFoldable(name)) return name
    if (seen.has(name)) return '' // foldable loop-back — terminates the loop
    const s2 = new Set(seen)
    s2.add(name)
    const rs: any = rules[name]
    if (!rs) return name
    if (name.includes('_opt')) {
      return '[ ' + ruleSeq(rs, s2) + ' ]'
    }
    // group / chain-step: inline the body, parenthesising a bare
    // multi-way alternation that will sit inside a larger sequence.
    const body = ruleSeq(rs, s2)
    return 1 < contentOpens(rs).length && !closeCont(rs, s2)
      ? '( ' + body + ' )'
      : body
  }

  // Order: real start first, then the remaining USER (non-synthetic) rules.
  const userRules = Object.keys(rules).filter(
    (rn) => rn !== synthWrapper && !isFoldable(rn),
  )
  const ordered: string[] = []
  const seenR = new Set<string>()
  if (startRule && rules[startRule] && !isFoldable(startRule)) {
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
    const body = emitBody(rules[rn], new Set([rn]))
    lines.push(rn + ' = ' + body)
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

// Render a token reference: every token appears by its bare NAME (e.g.
// '#PL' -> 'PL', '#NR' -> 'NR'), and its definition is recorded in `used`
// for the comment legend. A token name that is actually a rule name is a
// nonterminal reference and is returned as-is (no legend entry).
function emitAbnfTerminal(
  tabnas: Tabnas,
  cfg: Config,
  tin: number,
  used: Map<string, string>,
): string {
  const fullName: string = tabnas.token[tin]

  const rules: any = tabnas.rule()
  if (fullName && rules[fullName]) {
    return fullName
  }

  const name = (fullName || 'T' + tin).replace(/^#/, '')
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
function abnfTokenForm(cfg: Config, tin: number, fullName: string): string {
  const fixedLit = (cfg.fixed.ref as any)[tin]
  if ('string' === typeof fixedLit) {
    return /[A-Za-z]/.test(fixedLit)
      ? '%s"' + fixedLit + '"'
      : '"' + fixedLit + '"'
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
  return '<' + (desc[bare] || 'built-in ' + bare) + '>'
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

  // Anything else: keep it visible but mark it as non-round-tripping.
  return '; /' + re.source + '/' + re.flags
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

export { Debug }
