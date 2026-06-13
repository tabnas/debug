# Explanation: how the plugin works

This page explains how `@tabnas/debug` hooks into the parser and why it
is shaped the way it is. It is background, not instructions — for those,
see the [how-to guides](README.md).

## A plugin, not a fork

`tabnas` parsers are extended through plugins: functions that receive the
instance and mutate it. The debug plugin uses exactly this mechanism. It
adds capabilities by attaching a method and wrapping existing ones,
rather than by changing the parser. That keeps debugging entirely
opt-in: a parser with the plugin loaded but its options off behaves like
one without it.

This is also why the plugin can reach so deeply into the parser. Because
it runs as a plugin with full access to the instance, it can read
internal configuration (`internal().config`), the rule table, and the
lexer's matcher list — the things you need to understand a grammar but
that the normal parse API does not surface.

## Three independent features

The plugin offers three things, deliberately decoupled so you can take
only what you need:

1. **Description** — `describe()` walks the live configuration and
   renders it as text. It is a pure read: call it whenever, it changes
   nothing. This is the feature you reach for when you want to know *what
   grammar the parser currently has*.

2. **Printing** — when `print` is on, the plugin wraps `use` so that the
   grammar is dumped after every plugin load. This answers *how did the
   grammar change as I composed plugins?* It is separate from description
   because the automatic dump is noisy; you often want `describe()`
   without it.

3. **Tracing** — when any trace kind is on, the plugin registers a hook
   that runs as a parse begins and installs a logging function the parser
   calls at each event. This answers *what did the parser do on this
   input?*

Keeping these independent is why the options are two separate switches
(`print` and `trace`) rather than one verbosity level.

## Why tracing is wired at load time

The trace logger is installed through a parse-prepare hook registered
when the plugin loads — not toggled per parse. The parser calls into the
logger as it lexes and applies rules, and the logger decides per event
whether that kind is enabled before formatting anything. Two consequences
follow:

- Enable tracing on the instance you intend to trace, at load time. The
  how-to guides use a fresh instance for a traced run for this reason.
- Filtering by kind is cheap: a disabled kind is rejected before its line
  is ever built, so leaving the plugin loaded with most kinds off costs
  little.

## Why the output format is fixed and shared

The `describe()` sections and the trace line fields use a fixed layout
with stable headers. This is intentional. Stable, column-aligned text can
be diffed: before vs. after a change, or one implementation against
another. The TypeScript and Go ports emit identical section headers
specifically so that their output can be compared as a parity check —
the format is part of the contract, not an accident of printing.

## Canonical TypeScript, mirrored Go

The TypeScript implementation is the source of truth. The Go port exists
to make the same debugging available to Go users of the parser, and it
tracks the TypeScript behaviour rather than evolving on its own. When the
two could drift — a new trace kind, a changed default, a reordered
section — the TypeScript side decides and the Go side follows. The shared
output format is what makes that parity checkable in practice.
