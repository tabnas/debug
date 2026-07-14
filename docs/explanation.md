# Explanation: how the plugin works

This page explains how `@tabnas/debug` hooks into the parser and why it
is shaped the way it is. It is background, not instructions — for those,
see the [how-to guides](README.md).

## A plugin, not a fork

`tabnas` parsers are extended through plugins: functions that receive the
instance and mutate it. The debug plugin uses exactly this mechanism. It
adds capabilities by attaching behaviour and subscribing to events,
rather than by changing the parser. Debugging stays entirely opt-in: a
parser with the plugin loaded but tracing off behaves like one without
it.

This is also why the plugin can introspect so much. Running as a plugin
(TypeScript) or through the engine's exported accessors (Go), it can read
the token table, the rule specs, and the lexer matchers — the things you
need to understand a grammar but that the normal parse API does not
surface.

## Two features, decoupled

The plugin offers two things you can take separately:

1. **Description** — `describe()` (TypeScript) / `Describe(j)` (Go) walks
   the live configuration and renders it as text. It is a pure read:
   call it whenever, it changes nothing. Reach for it when you want to
   know *what grammar the parser currently has*.

2. **Tracing** — when enabled, the plugin logs what the parser does as it
   runs. Reach for it when you want to know *what the parser did on this
   input*.

A third feature, **printing**, dumps the grammar after each later plugin
load. In TypeScript the plugin wraps the instance's `use` in place; the
Go engine's `Use` is a concrete method that cannot be reassigned, so the
Go plugin exposes the wrapped form as the package function
`debug.Use(j, plugin, opts...)` — later loads made through it print the
`USE:` line and the grammar dump.

## How tracing is installed

Tracing is wired when the plugin loads, not toggled per parse — so enable
it on the instance you intend to trace. Both runtimes emit the same six
kinds: `step`, `rule`, `lex`, `parse`, `node`, `stack`.

- **TypeScript** registers a parse-prepare hook that installs a logging
  function the parser calls at each event, and filters by kind before
  formatting a line. That is why filtering is cheap.
- **Go** combines three engine hooks: the `Tabnas.Sub` subscribers (the
  token stream drives `lex`; the rule stream drives `step`, `stack` and
  `rule`), a parse-prepare hook that prints the TRACE banner, and
  after-open/after-close rule state actions installed at parse start
  (driving `parse` and `node`). Filtering by kind happens before
  formatting, as in TypeScript.

## Why the output format is fixed and shared

The `describe` sections use a fixed layout with stable, identical headers
across both implementations. This is intentional: stable text can be
diffed — before vs. after a change, or one language against the other.
The format is part of the contract, not an accident of printing.

## Canonical TypeScript, tracked Go

The TypeScript implementation is the source of truth. The Go port exists
to make the same debugging available to Go users, and it tracks the
TypeScript behaviour rather than evolving on its own. Where the two
engines genuinely differ — Go's `debug.Use` form of the `print` option,
its `parse` lines without an alt index, its summarised `LEXER` section
and symbol-derived plugin names — the gaps are a
consequence of the engine APIs, and they are written down in the
[reference](reference.md#parity-and-remaining-differences-go-vs-canonical-typescript)
rather than left implicit. When behaviour could drift, TypeScript decides
and Go follows.
