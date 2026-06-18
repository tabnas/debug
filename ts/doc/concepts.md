# Concepts

How the debug plugin works, and why it is shaped the way it is. For the
API, see the [reference](reference.md); for tasks, the [guide](guide.md).

## What the plugin is for

`@tabnas/debug` is an introspection plugin. It does not change how a
grammar parses — it makes a grammar *visible*. It answers questions you
have while authoring or reviewing a grammar:

- What tokens and rules does this instance actually have?
- How do the rules connect (which rule pushes into which)?
- What does each alternate match, and what does it do?
- What does this grammar look like as ABNF?
- What is the parser doing, step by step, on a given input?

It is a development and test aid. It is **never a runtime dependency**: a
grammar you ship does not need the debug plugin to parse — only to inspect
or describe.

## The engine relationship

The plugin is a thin reader over the live `Tabnas` engine. When you call
`tn.use(Debug, ...)`, the plugin reaches into the instance's internals
(`tabnas.internal().config`, `tabnas.rule()`, `tabnas.internal().plugins`)
and exposes them through three read-only methods:

- `describe()` formats that state as labelled text.
- `model()` returns the same state as typed data.
- `abnf()` re-expresses the grammar as ABNF.

None of these methods parse anything or mutate the grammar. They are pure
projections of the engine's current configuration, taken at the moment
you call them. Add a rule, change a token, and the next call reflects it.

Tracing is the one method that hooks the engine's runtime: with
`trace: true`, the plugin installs a `parse.prepare` hook that gives the
parse context a `log` callback. The engine calls that callback at each
step (`lex`, `rule`, `parse`, `node`, `stack`), and the plugin formats and
prints the event.

## `describe()` versus `model()`

These two methods carry the *same* information in different shapes:

- `describe()` is for a human reading a terminal. It is text, organised
  into sections with fixed headers, and is deliberately diffable.
- `model()` is for a program. It is a typed, JSON-serialisable object, so
  a test can assert on it and a tool can transform it without parsing
  text.

The split exists because text is great for eyeballing and terrible for
asserting. Before `model()`, a test that wanted to check "does `val` push
into `map`?" had to scrape the `describe()` string. With `model()` it
reads `m.graph.find(g => g.name === 'val').openPush`. The `describe()`
output stays free to evolve its layout because tests target the model.

## What the model captures

The model is a faithful, structured snapshot of the grammar:

- **tokens / tokenSets** — the token table (tin, name, fixed literal) and
  the named token sets (`IGNORE`, `VAL`, `KEY`, …).
- **rules** — every rule, with its `open` and `close` alternates. Each
  alternate (`DebugAltInfo`) records its lookahead token sequence, any
  push/replace target, backtrack, counters, group tags, and whether it
  carries an action / condition / modifier.
- **graph** — the rule-reference graph: for each rule, the distinct rules
  it can push into or replace with, split by open/close phase. This is the
  alternates' targets de-duplicated — a quick map of how the grammar's
  rules connect.
- **lexer** — the ordered list of lexer matchers.
- **config** — the start rule, the finish flag, the safe-key setting, and
  which built-in lexers are enabled.
- **plugins** — the applied plugins and their options.
- **abnf** — the grammar as ABNF text.

The `rules` and `graph` fields are two views of the same thing: `rules`
is the full alternate detail, `graph` is the connectivity summary derived
from it.

## ABNF: the round-trip

`abnf()` re-expresses the live grammar in [ABNF](https://www.rfc-editor.org/rfc/rfc5234).
Rules become ABNF productions, open alternates become `/`-separated
alternatives, a token sequence plus any push/replace target becomes a
space-separated element list, and each token resolves to an ABNF terminal
(a quoted literal, a char-range, or a prose value for built-in lexer
tokens).

The interesting design point is **independence**: the emitter reads only
the running engine. It never imports `@tabnas/abnf`. This keeps the debug
plugin out of the ABNF library's dependency graph (the two repositories
sit side by side, and `@tabnas/abnf` must not depend on `@tabnas/debug`,
nor the reverse).

The emitter is the *empirical inverse* of the ABNF compiler's forward
encoding. The contract, exercised by the round-trip test, is: take an
ABNF source `A0`, compile it to a grammar, install it, call `abnf()` to
get `A1`, recompile `A1` to a second grammar — and the two grammars must
*recognise the same inputs identically* (same parse success/failure, same
top rule). ABNF has no actions, so output *values* are out of scope; only
recognition round-trips.

Several encodings exist precisely to make that round-trip hold:

- An epsilon close alternate marks an optional continuation, which the
  emitter renders as `[ ... ]` so repetition/optional shapes survive.
- A backtrack (`b`) together with a push/replace means the token
  sequence is a predictive *peek* — matched to choose the alternate but
  consumed by the pushed rule. Emitting those tokens as terminals would
  double-count the input, so the emitter skips them.

Constructs ABNF cannot express — an arbitrary match regex — are emitted as
ABNF comments (`; /.../`) rather than dropped. The output stays valid
ABNF text and self-documents what was lost, but such a grammar does not
round-trip.

## Design trade-offs

- **Read-only, defensive.** The introspection methods never mutate and
  never assume the grammar is well-formed. A null entry in an alternate's
  sequence renders as `***INVALID***` rather than throwing, so the plugin
  stays useful while you are mid-edit and the grammar is broken.
- **Text and data both.** Keeping `describe()` *and* `model()` costs some
  duplication (each section is produced twice), but it serves the two
  audiences — humans and tooling — without forcing one to consume the
  other's format.
- **Diffable layout.** The `describe()` section headers are fixed and
  shared with the Go port via a golden fixture, so dumps from either
  runtime can be diffed line for line.
- **No runtime cost.** Because the plugin is dev-only and its readers are
  pure projections, it adds nothing to a shipped grammar's parse path.
