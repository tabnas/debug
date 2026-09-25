# Explanation: how the plugin works

This page explains how `@tabnas/debug` hooks into the parser and why it
is shaped the way it is. It is background, not instructions; for those,
see the [how-to guides](README.md).

## A plugin, not a fork

`tabnas` parsers are extended through plugins: functions that receive the
instance and mutate it. The debug plugin uses exactly this mechanism. It
adds capabilities by attaching behaviour and subscribing to events,
rather than by changing the parser. Debugging stays entirely opt-in: a
parser with the plugin loaded but tracing off behaves like one without
it.

This is also why the plugin can introspect so much. Running as a plugin
(TypeScript) or through the engine's exported accessors (Go and Rust), it
can read the token table, the rule specs, and the lexer matchers: the
things you need to understand a grammar but that the normal parse API
does not surface.

## Two features, decoupled

The plugin offers two things you can take separately:

1. **Description**. `describe()` (TypeScript), `Describe(j)` (Go) or
   `describe(&parser)` (Rust) walks
   the live configuration and renders it as text. It is a pure read:
   call it whenever, it changes nothing. Reach for it when you want to
   know *what grammar the parser currently has*. Outside TypeScript it is
   a free function, so it needs no plugin installed to call.

2. **Tracing**. When enabled, the plugin logs what the parser does as it
   runs. Reach for it when you want to know *what the parser did on this
   input*.

A third feature, **printing**, dumps the grammar after each later plugin
load. In TypeScript the plugin wraps the instance's `use` in place;
neither the Go engine's `Use` nor Rust's `Tabnas::use_plugin` is a
field that can be reassigned, so both ports expose the wrapped form as a
function, `debug.Use(j, plugin, opts...)` and
`tabnas_debug::use_plugin(&mut parser, plugin, options)`. Later loads
made through it print the `USE:` line and the grammar dump.

## How tracing is installed

Tracing is wired when the plugin loads, not toggled per parse, so enable
it on the instance you intend to trace. The six kind names are the same
everywhere: `step`, `rule`, `lex`, `parse`, `node`, `stack`.

- **TypeScript** registers a parse-prepare hook that installs a logging
  function the parser calls at each event, and filters by kind before
  formatting a line. That is why filtering is cheap.
- **Go** combines three engine hooks: the `Tabnas.Sub` subscribers (the
  token stream drives `lex`; the rule stream drives `step`, `stack` and
  `rule`), a parse-prepare hook that prints the TRACE banner, and
  after-open/after-close rule state actions installed at parse start
  (driving `parse` and `node`). Filtering by kind happens before
  formatting, as in TypeScript.
- **Rust** uses the engine's typed subscribers: `subscribe_lex` drives
  `lex`, `subscribe_rules` drives `rule` and `stack`, and
  `subscribe_rule_done` drives `parse` and `node`. A named parse-prepare
  hook prints the TRACE banner. Five of the six kinds have a source;
  `step` has none, because the Rust engine has no `ctx.log` and emits no
  per-step event, so selecting it logs nothing. The Rust engine
  accumulates subscribers, so the plugin registers its callbacks once per
  instance and later installs update a shared selection rather than
  stacking a second set.

## Why the output format is fixed and shared

The `describe` sections use a fixed layout with stable, identical headers
across all three implementations. This is intentional: stable text can be
diffed: before vs. after a change, or one language against the other.
The format is part of the contract, not an accident of printing: the
eight headers are pinned byte-for-byte by the shared
`test/spec/sections.tsv` fixture that every runtime runs.

## Canonical TypeScript, tracked ports

The TypeScript implementation is the source of truth. The Go and Rust
ports exist to make the same debugging available to those users, and they
track the TypeScript behaviour rather than evolving on their own. Where
an engine genuinely differs (the function form of the `print` option,
`parse` lines without an alt index, a summarised `LEXER` section, Go's
symbol-derived plugin names, Rust's silent `step` kind), the gaps are a
consequence of the engine APIs, and they are written down in the
reference, per port
([Go](reference.md#parity-and-remaining-differences-go-vs-canonical-typescript),
[Rust](reference.md#parity-and-remaining-differences-rust-vs-canonical-typescript)),
rather than left implicit. When behaviour could drift, TypeScript decides
and the ports follow.
