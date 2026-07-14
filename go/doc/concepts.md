# Concepts (Go)

How the Go debug package works, and why it is shaped the way it is. This
is the Go companion to the [TypeScript concepts](../../ts/doc/concepts.md);
read that first for the full picture. This document focuses on the Go
port and ends with the differences from the canonical TypeScript version.

## What the package is for

`tabnasdebug` is an introspection package. It does not change how a
grammar parses — it makes a grammar *visible*: the tokens and rules an
instance has, how the rules connect, what each alternate matches, the
grammar as ABNF, and a step-by-step trace of a parse.

It is a development and test aid. It is **never a runtime dependency**: a
grammar you ship does not need it to parse — only to inspect or describe.

## The engine relationship

The package is a thin reader over the live `*tabnas.Tabnas` engine. Its
inspection functions read the instance's exported accessors and format
the result:

- `Describe(j)` reads `j.Config()`, `j.RSM()`, `j.TokenSet(...)`,
  `j.Plugins()` and formats them as labelled text.
- `Model(j)` reads the same state and returns it as typed,
  JSON-serialisable data (`*DebugModel`).
- `Abnf(j)` reads the same config and rule specs and re-expresses the
  grammar as ABNF.

None of them parses anything or mutates the grammar; all are pure
projections of the engine's current state, taken when you call them.

Tracing is the one part that hooks the runtime: with `trace` enabled, the
plugin installs a token subscriber and a rule subscriber via `j.Sub`
(driving the `lex`, and the `step` / `stack` / `rule`, streams), a
parse-prepare hook that prints the TRACE banner, and after-open /
after-close rule state actions that drive the `parse` and `node`
streams — together covering all six TypeScript trace kinds. Each event
is formatted to the configured writer.

## The no-panic guarantee

Every error-returning entry point (`Debug`, `Describe`, `Abnf`) defers a
`recover()` that converts a panic into an `"internal"`-code
`*tabnas.TabnasError`. This mirrors the engine's own contract: a caller of
this package can never be crashed by it. Malformed grammar specs (a nil
config, a nil rule spec, a nil alternate) are rendered defensively — a nil
alternate becomes the literal `***INVALID***` — rather than dereferenced.
That is why the Go functions return `(string, error)` where the
TypeScript methods return a bare string: Go has no exceptions, so the
guarantee is expressed in the signature.

## ABNF: the round-trip

`Abnf` re-expresses the live grammar in
[ABNF](https://www.rfc-editor.org/rfc/rfc5234), the same empirical inverse
of the ABNF compiler's forward encoding as the TypeScript `abnf()`. Rules
become productions, open alternates become `/`-separated alternatives, a
token sequence plus any push/replace target becomes a space-separated
element list, and each token resolves to an ABNF terminal.

Two encodings exist to make recognition round-trip: an epsilon close
alternate marks an optional continuation, rendered as `[ ... ]`; and a
backtrack (`B`) with a push/replace marks a predictive *peek* whose tokens
the pushed rule consumes, so they are skipped here to avoid
double-counting the input. Constructs ABNF cannot express are emitted as
ABNF comments (`; /.../`) so the output stays valid text.

The Go emitter reads only the running engine. Go has no ABNF library port,
so there is nothing to depend on; the independence the TypeScript port
enforces (never importing `@tabnas/abnf`) is automatic here.

## Differences from the TS version

The TypeScript implementation (`ts/src/debug.ts`) is canonical; the Go
port tracks it as far as the Go engine API allows. The differences below
are imposed by that API, not by choice, and are also recorded in the
project's combined `docs/reference.md`.

1. **Functions, not methods, and error returns.** TypeScript attaches
   `describe()` / `model()` / `abnf()` to the instance as methods that
   return bare strings/objects. Go exposes `Describe(j)`, `Model(j)` and
   `Abnf(j)` as package functions taking the instance, returning
   `(value, error)` to uphold the no-panic guarantee.

2. **`print` lives in `tabnasdebug.Use`.** The TypeScript plugin wraps
   the instance's `use()` in place so every later plugin load prints
   `USE:` plus a description. The Go engine's `(*Tabnas).Use` is a
   concrete method that cannot be reassigned, so the wrapped form is the
   package function `tabnasdebug.Use(j, plugin, opts...)`: it delegates
   to `j.Use` and, when the instance's `print` option is active, logs the
   `USE:` line and the `Describe` dump.

3. **Trace kinds are synthesised from three engine hooks.** All six
   TypeScript kinds (`step`, `rule`, `lex`, `parse`, `node`, `stack`)
   have Go streams, but the TS engine drives them from a single `ctx.log`
   callback while the Go engine offers no such callback. The Go plugin
   instead combines the rule subscriber (`step`, `stack`, `rule` — fired
   at the same pre-step point the TS engine logs them), the lex
   subscriber (`lex`), and after-open/after-close rule state actions
   installed at parse start (`parse`, `node` — the closest post-match
   hook). Two shape gaps remain: `parse` lines say `alt` / `no-alt`
   without the TS alt *index* (the engine does not expose which alternate
   matched), and `lex` lines omit the matcher name.

4. **Trace destination.** TypeScript logs to the instance's console
   provider (`get_console()`); Go writes to `opts["out"]` (an
   `io.Writer`), defaulting to `os.Stdout`, so trace output is captured in
   tests via a `bytes.Buffer` rather than a fake console.

5. **Summarised `LEXER` section; symbol-derived plugin names.** The Go
   engine exposes only custom lexer matchers (the built-in matchers are
   not enumerable; their enable flags appear under `CONFIG`). Plugins are
   stored as bare functions, so the `PLUGIN` section and
   `Model(...).Plugins` derive each name from the function's symbol (the
   Go analogue of the TS function `name`), and per-plugin options appear
   only when registered via `Tabnas.SetPluginOptions`.

6. **Deterministic token ordering.** The Go engine exposes token sets and
   custom token names through Go maps, which do not preserve insertion
   order. The Go port orders tokens and members by tin (built-ins in their
   canonical order, then custom tins ascending) so the output is
   deterministic and diffable, rather than matching TypeScript's exact
   insertion order.

The `Describe` section headers are identical across both runtimes — pinned
by the shared `test/headers.golden` fixture that both test suites assert —
so even where the section *bodies* differ, the layout stays diffable.
