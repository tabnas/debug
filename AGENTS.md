# Agents Guide — debug

## What this project is

`@tabnas/debug` is the **tracing + introspection plugin** for the
[`tabnas`](https://github.com/tabnas/parser) parsing engine. It is the
developer tool every other tabnas repo's test suite consumes, and it
provides three things:

- **`describe()`** (TS) / **`Describe(j)`** (Go) — a human-readable dump
  of a live `Tabnas` instance: its tag, tokens, token sets, rules,
  alternates, lexer matchers, config, plugins, and an ABNF rendering of
  the grammar.
- **`model()`** (TS only) — the *structured* counterpart to `describe()`:
  the same information as a typed, JSON-serialisable `DebugModel` object
  so tools and tests can consume the grammar programmatically. This is
  the surface the `debug.model()` tests in the grammar repos (json, csv,
  hoover, abnf, jsonic, …) assert against. The Go port has no `Model`
  equivalent yet — it exposes `Describe` (text) and `Abnf` only.
- **`abnf()`** (TS) / **`Abnf(j)`** (Go) — emits a re-compilable ABNF
  representation of the instance's *live* grammar.
- **parse tracing** that logs events as the parser runs.

The plugin is a developer tool, **not part of the parse path**. It is a
dev-only `file:` devDependency in (almost) every other tabnas repo — the
exception being `@tabnas/jsonic-cli`, which depends on it as a real prod
peer for its `--debug` flag.

## Repository map

| Path | What it is |
|---|---|
| [`ts/`](ts/) | **Canonical** TypeScript implementation — the `@tabnas/debug` package. Everything lives in `src/debug.ts` (plugin, `describe`/`model`/`abnf`, trace hooks, ABNF emitter). Depends on `@tabnas/parser` (peer + sibling `file:` devDep). |
| [`go/`](go/) | Go port — module `github.com/tabnas/debug/go`, all in `go/debug.go`. Tracks `ts/` as far as the Go engine API allows. |
| [`docs/`](docs/) | Cross-language docs by purpose: `tutorial.md`, `how-to/`, `reference.md`, `explanation.md` (see `docs/README.md`). |
| [`test/headers.golden`](test/headers.golden) | The 8 `========= … ========` section headers, shared by both suites as the cross-runtime diffability contract. |
| `scripts/fetch-parser.sh` | Legacy engine-fetch helper (see note below). |
| `vendor/tabnas-parser` | Symlink to the sibling `../parser` checkout (git-ignored). |

There is no shared `.tsv` fixture set here — this is a tool, not a
grammar; its parity contract is the section headers and the
`describe`/`model` output shape, not input→output pairs.

## The tabnas engine dependency

Both runtimes depend on the engine as a **sibling checkout**, the shared
tabnas development model, until `tabnas/parser` publishes tagged packages:

- TypeScript: `@tabnas/parser` is a `peerDependencies` `">=2"` and a
  `"@tabnas/parser": "file:../../parser/ts"` devDependency in
  `ts/package.json`. `node_modules/@tabnas/parser` symlinks to
  `../../parser/ts`.
- Go: `go/go.mod` requires `github.com/tabnas/parser/go` with
  `replace github.com/tabnas/parser/go => ../vendor/tabnas-parser/go`,
  where `vendor/tabnas-parser` is a **symlink** to the sibling `../parser`
  checkout. So the replace resolves to `../parser/go`.

Clone `https://github.com/tabnas/parser` as a sibling of this repo and
build its TS (`cd parser/ts && npm install && npm run build`) before
working here. CI clones the siblings and builds them first (see below).

The TS tests also reach into siblings directly, by **path, not by
dependency**:
- `ts/test/debug.test.js` loads the engine's compiled json grammar
  fixture from `@tabnas/parser`'s `dist-test/json-plugin.js` (resolved
  relative to the engine package) to exercise `describe`/`model` against
  a real grammar.
- `ts/test/abnf.test.js` round-trips `abnf()` through `@tabnas/bnf`'s
  `bnfConvert`, loaded from `../../abnf/ts/dist/bnf.js`. This is a
  **hard independence constraint**: `@tabnas/bnf` must *not* be a runtime
  dependency of the debug plugin; it is used in the test only.

`@tabnas/bnf` (the `abnf` repo) and `@tabnas/railroad` are present as
`file:` devDependencies for exactly these sibling test/diagram needs.

### Note: `scripts/fetch-parser.sh` is legacy

The build no longer fetches the engine into `vendor/` over HTTPS. The
Makefile and CI use the sibling-checkout model above, and
`vendor/tabnas-parser` is now a symlink to the local `../parser` checkout
rather than a downloaded tarball. `fetch-parser.sh` (and the
`session-start` hook that runs it) survive for ad-hoc local use; the
script's `TABNAS_PARSER_REF` / `TABNAS_PARSER_SKIP_TS_BUILD` env vars and
the `go.mod` comment still describe that older path. Don't rely on
`make build` running the fetch script — it doesn't.

## Authority and alignment rules

1. **TypeScript is canonical.** `ts/src/debug.ts` is the source of truth
   for behaviour, option names, `DEFAULTS`, output format, and section
   ordering. Change TS first, then update Go to match as far as the Go
   engine API allows.
2. The **8 section headers** in `test/headers.golden` are the parity
   contract. `describe()` (TS) and `Describe(j)` (Go) must emit them
   byte-for-byte and in order: `INSTANCE`, `TOKENS`, `RULES`, `ALTS`,
   `LEXER`, `CONFIG`, `PLUGIN`, `ABNF`. Both suites assert against the
   golden so the cross-runtime diffability claim holds. (Tracing adds a
   separate `========= TRACE ==========` header.)
3. Keep the shared semantics — option meanings, `DEFAULTS` / `Defaults`,
   and the `describe`/`abnf` output — in lockstep across runtimes, and
   record any new divergence in `docs/reference.md`. (`model()` is
   currently TS-only; the structured `DebugModel` has no Go counterpart.)
4. The two engines are **not API-identical**; some divergence is real and
   **intended**, not drift:
   - TS traces six kinds (`step`, `rule`, `lex`, `parse`, `node`,
     `stack`) via a context-log hook; the Go engine exposes two streams
     (`lex`, `rule`) via `Tabnas.Sub`.
   - TS has a `print` option (default `true`) that wraps `use()` to dump
     `describe()` when a later plugin loads; the Go engine has no such
     hook, so the Go plugin omits `print`.
   - TS attaches `describe`/`model`/`abnf` as instance methods
     (`tn.debug.describe()`); in Go they are package functions
     (`Describe(j)`, `Abnf(j)`), and there is no Go `Model`.
   - Go's `LEXER`/`PLUGIN` sections are summarised — limited to what the
     engine's exported accessors (`Config`, `RSM`, `TinName`,
     `TokenSet`, `Plugins`) expose.

## The `model()` structured contract (what other repos assert)

`model()` returns a `DebugModel` with keys: `tag`, `tokens`,
`tokenSets`, `rules`, `graph`, `lexer`, `config`, `plugins`, `abnf`. The
grammar repos' `test/debug-model.test.ts` consume this, so be careful:

- **The start rule is `m.config.start`, NOT `m.start`.** `m.start` is
  `undefined` in this engine; config lives under `model.config`
  (`start`, `finish`, `safeKey`, `lex`). For the json grammar
  `m.config.start === 'val'`.
- `m.rules` is the rule set; `m.graph` is the rule-reference graph
  (per-rule `openPush` / `openReplace` / `closePush` / `closeReplace`
  edges) — that's where downstream tests assert grammar-specific push
  edges.
- `m.plugins` lists loaded plugins by name (e.g. a grammar test asserts
  `m.plugins` includes `json`).
- `m.abnf` is the re-compilable ABNF string; `abnf.test.js` proves it
  re-compiles via `@tabnas/bnf`.

Grammar repos load `@tabnas/debug` with a **skip-if-absent guard** so
their core suite still runs when the dev sibling isn't built.

## Build & test

This repo has a top-level Makefile (`build`, `test`, `clean`,
`build-ts`/`build-go`, `test-ts`/`test-go`, `publish-ts`, `publish-go`,
`tags-go`, `reset`) driving both runtimes:

```bash
make build    # build-ts (tsc) + build-go (GOWORK=off go build)
make test     # test-ts (node --test) + test-go (GOWORK=off go test)
```

TypeScript directly (in `ts/`):

```bash
cd ts && npm install && npm run build   # tsc --build src
npm test                                # node --enable-source-maps --test test/**/*.test.js
```

Go directly (in `go/`):

```bash
cd go && GOWORK=off go build ./... && GOWORK=off go test ./...
```

This Go module is **vendor-replaced** (it consumes the engine via a
local `replace`, outside the repo-set `go.work` workspace), so all Go
commands run with **`GOWORK=off`**. Run `gofmt` and `go vet ./...`
before committing Go changes.

The Go module carries a top-level `const Version` in `go/debug.go`;
`make publish-go V=x.y.z` seds that const, commits, and tags
`go/vX.Y.Z`.

## CI

`.github/workflows/build.yml` runs the TS suite on a matrix of
ubuntu/windows/macos with Node 24, using the **sibling-checkout**
strategy:

1. Sets `git config core.autocrlf false` (CRLF corrupts fixtures and
   golden output).
2. Checks this repo out into `debug/`, then `git clone --depth 1` the
   siblings `parser`, `json`, `abnf`, `railroad`.
3. Builds in topo order (`parser`, `debug`, `json`, `abnf`, `railroad`)
   with `npm i && npm run build --if-present`, then runs `npm test` in
   `debug/ts`.

(The CI workflow currently builds/tests only the TS side; build/test the
Go side locally with the `GOWORK=off` commands above.)

## Tests mirror each other

`ts/test/debug.test.js` ↔ `go/debug_test.go`; keep them aligned. The TS
suite also has `ts/test/abnf.test.js` (the `abnf()` ↔ `@tabnas/bnf`
round-trip) and `ts/test/doc-examples.test.js` (verifies the snippets in
`docs/`). When you add a capability, extend `docs/reference.md` and add a
how-to if it introduces a new task.
