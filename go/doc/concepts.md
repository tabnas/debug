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
two text functions read the instance's exported accessors and format the
result:

- `Describe(j)` reads `j.Config()`, `j.RSM()`, `j.TokenSet(...)`,
  `j.Plugins()` and formats them as labelled text.
- `Abnf(j)` reads the same config and rule specs and re-expresses the
  grammar as ABNF.

Neither parses anything or mutates the grammar; both are pure projections
of the engine's current state, taken when you call them.

Tracing is the one part that hooks the runtime: with `trace` enabled, the
plugin installs two engine subscribers via `j.Sub` — a token subscriber
(`[lex]`) and a rule subscriber (`[rule]`) — and formats each event to the
configured writer.

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

1. **No structured `model()`.** The biggest difference. The TypeScript
   port exposes `model()`, returning the whole grammar as a typed,
   JSON-serialisable object (tokens, rules, the rule-reference graph,
   config, plugins, ABNF) so a test or tool can consume the grammar as
   data. The Go engine's introspection API does not support an equivalent
   typed projection, so the Go package offers only the text forms,
   `Describe` and `Abnf`. In Go you assert on the `Describe` text.

2. **Functions, not methods, and `(string, error)`.** TypeScript attaches
   `describe()` / `model()` / `abnf()` to the instance as methods that
   return bare strings/objects. Go exposes `Describe(j)` and `Abnf(j)` as
   package functions taking the instance, returning `(string, error)` to
   uphold the no-panic guarantee.

3. **No `print` option.** The TypeScript plugin wraps `use()` to print a
   description after each later `use()`. The Go engine exposes no `use`
   hook to wrap, so the `print` behaviour is absent.

4. **Two trace kinds, not six.** The Go engine's `Sub` API surfaces only
   token (`lex`) and rule streams. The finer TypeScript kinds (`step`,
   `parse`, `node`, `stack`) have no Go equivalent. A per-kind trace
   object is accepted but simply turns both streams on.

5. **Trace destination.** TypeScript logs to the instance's console
   provider (`get_console()`); Go writes to `opts["out"]` (an
   `io.Writer`), defaulting to `os.Stdout`, so trace output is captured in
   tests via a `bytes.Buffer` rather than a fake console.

6. **Summarised `LEXER` and `PLUGIN` sections.** The Go engine exposes
   only custom lexer matchers (the built-in matchers are not enumerable;
   their enable flags appear under `CONFIG`) and stores plugins as bare
   functions (so the `PLUGIN` section reports a count, not per-plugin names
   and options).

7. **Deterministic token ordering.** The Go engine exposes token sets and
   custom token names through Go maps, which do not preserve insertion
   order. The Go port orders tokens and members by tin (built-ins in their
   canonical order, then custom tins ascending) so the output is
   deterministic and diffable, rather than matching TypeScript's exact
   insertion order.

The `Describe` section headers are identical across both runtimes — pinned
by the shared `test/headers.golden` fixture that both test suites assert —
so even where the section *bodies* differ, the layout stays diffable.
