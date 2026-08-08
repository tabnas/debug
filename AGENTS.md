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
- **`model()`** (TS) / **`Model(j)`** (Go) — the *structured* counterpart
  to `describe()`: the same information as a typed, JSON-serialisable
  `DebugModel` object so tools and tests can consume the grammar
  programmatically. This is the surface the `debug.model()` tests in the
  grammar repos (json, csv, hoover, abnf, jsonic, …) assert against. Both
  runtimes now export the full type set, and the Go structs' JSON tags
  match the TS field names, so decoded models are comparable across
  runtimes (pinned by `test/spec/model.tsv`).
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
| [`go/`](go/) | Go port — module `github.com/tabnas/debug/go`: `debug.go` (plugin, `Describe`, `Abnf`, ABNF emitter), `model.go` (`Model` + the `Debug*` types), `trace.go` (the six trace kinds). Tracks `ts/` as far as the Go engine API allows. |
| [`docs/`](docs/) | Cross-language docs by purpose: `tutorial.md`, `how-to/`, `reference.md`, `explanation.md` (see `docs/README.md`). |
| [`test/spec/`](test/spec/) | The shared `.tsv` conformance fixtures both suites run — emitted ABNF, the `========= … ========` section headers, and the structured model's rules/graph, per named grammar. See [`test/AGENTS.md`](test/AGENTS.md). |
| `scripts/fetch-parser.sh` | Legacy engine-fetch helper (see note below). |
| `vendor/tabnas-parser` | Symlink to the sibling `../parser` checkout (git-ignored). |

The shared `.tsv` fixture set is deliberately narrow — this is a tool, not a
grammar; its parity contract is the section headers and the
`describe`/`model` output shape, not input→output pairs.

## The tabnas engine dependency

The two runtimes resolve the engine **differently**, and the difference
matters when you are chasing a discrepancy:

- TypeScript: `@tabnas/parser` is a `peerDependencies` `">=0"` and a
  `"*"` devDependency in `ts/package.json`. Locally,
  `ts/node_modules/@tabnas/parser` is a **symlink to the sibling
  `../../parser/ts` checkout**, wired by `admin/scripts/link.sh` — so the
  TS suite tests against sibling `main`. Do not run `npm ci` or delete
  `node_modules`: that replaces the symlink with a registry copy.
- Go: `go/go.mod` requires `github.com/tabnas/parser/go` at a **pinned
  published version** and carries **no `replace`**. So `GOWORK=off go
  test` resolves the engine from the module proxy, not from the sibling
  checkout. The repo-set `../go.work` *does* list `./debug/go`, so a plain
  `go test` (workspace on) resolves the sibling instead. Both currently
  pass; see [`go/AGENTS.md`](go/AGENTS.md) for why that gap has bitten
  before.

Clone `https://github.com/tabnas/parser` as a sibling of this repo and
build its TS (`cd parser/ts && npm install && npm run build`) before
working here. CI clones the siblings and builds them first (see below).

The TS tests also reach into siblings directly, by **path, not by
dependency**:
- `ts/test/debug.test.js` loads the engine's compiled json grammar
  fixture from `@tabnas/parser`'s `dist-test/json-plugin.js` (resolved
  relative to the engine package) to exercise `describe`/`model` against
  a real grammar.
- `ts/test/abnf.test.js` round-trips `abnf()` through `@tabnas/abnf`'s
  `abnfConvert`, loaded from `../../abnf/ts/dist/abnf.js`. This is a
  **hard independence constraint**: `@tabnas/abnf` must *not* be a runtime
  dependency of the debug plugin; it is used in the test only.

`@tabnas/abnf` (the `abnf` repo) and `@tabnas/railroad` are present as
`file:` devDependencies for exactly these sibling test/diagram needs.

### Note: `scripts/fetch-parser.sh` is legacy

The build no longer fetches the engine into `vendor/` over HTTPS, and
**nothing in the build or test path reads `vendor/` any more**: `go.mod`
has no `replace` pointing at it and `ts/package.json` has no `file:` dep
on it. Whatever is in `vendor/tabnas-parser` (git-ignored) is a stale
download, not a live dependency — deleting it changes nothing.

`fetch-parser.sh` and the `.claude/hooks/session-start.sh` that runs it
survive for ad-hoc use on a fresh remote container. Don't rely on `make
build` running the fetch script — it doesn't. Note that running it
re-downloads over the `vendor/` directory without affecting the build,
so it is not a way to test against engine `main`; for that, see the
`go mod edit -replace` recipe in [`go/AGENTS.md`](go/AGENTS.md).

## Authority and alignment rules

1. **TypeScript is canonical.** `ts/src/debug.ts` is the source of truth
   for behaviour, option names, `DEFAULTS`, output format, and section
   ordering. Change TS first, then update Go to match as far as the Go
   engine API allows.
2. The **8 section headers** pinned by `test/spec/sections.tsv` are the
   parity contract. `describe()` (TS) and `Describe(j)` (Go) must emit them
   byte-for-byte and in order: `INSTANCE`, `TOKENS`, `RULES`, `ALTS`,
   `LEXER`, `CONFIG`, `PLUGIN`, `ABNF`. Both suites run that fixture, for
   every grammar in the shared registry, so the cross-runtime diffability
   claim holds. (Tracing adds a separate `========= TRACE ==========`
   header.)
3. Keep the shared semantics — option meanings, `DEFAULTS` / `Defaults`,
   the `describe`/`abnf` output, and the `model`/`Model` shape — in
   lockstep across runtimes, and record any new divergence in
   `docs/reference.md`. That file is the authoritative divergence
   register; this section only summarises it.
4. The two engines are **not API-identical**; some divergence is real and
   **intended**, not drift. The Go port has closed most of the gaps that
   earlier revisions of this guide described as permanent — it now has
   `Model`, the `print` option, and all six trace kinds. What remains:
   - Both runtimes trace the same six kinds (`step`, `rule`, `lex`,
     `parse`, `node`, `stack`), individually selectable. But Go `parse`
     lines omit the TS alt *index* and `lex` lines omit the matcher name,
     because the Go engine does not expose them.
   - Both have a `print` option (default `true`). In TS it wraps
     `tabnas.use`; the Go engine's `(*Tabnas).Use` is a concrete method
     that cannot be reassigned, so Go exposes the wrapper as the package
     function `tabnasdebug.Use(j, plugin, opts...)`. Plugins loaded
     directly via `j.Use` do not trigger the `USE:` log.
   - TS attaches `describe`/`model`/`abnf` as instance methods
     (`tn.debug.describe()`); in Go they are package functions
     (`Describe(j)`, `Model(j)`, `Abnf(j)`) returning `(value, error)`,
     upholding the engine's no-panic guarantee.
   - Go's `LEXER`/`PLUGIN` sections are summarised — limited to what the
     engine's exported accessors (`Config`, `RSM`, `TinName`,
     `TokenSet`, `Plugins`) expose.
   - Ordering: Go sorts rules by name and token-set members by tin, where
     TS uses insertion order.

   The unset-instance-`tag` divergence that earlier revisions listed here
   is **no longer a Go-port limitation**: the engine now exports
   `tabnas.DefaultTag = "-"` and `Make` applies it, so both runtimes
   report `-` for an untagged instance. The only thing left is an engine
   *version* boundary — `go/go.mod` still pins the pre-alignment
   `github.com/tabnas/parser/go v0.6.1`, and `make test` / CI use
   `GOWORK=off`, so under that resolution Go still prints a bare `tag:`.
   Run `cd go && go test -count=1 ./...` with the repo `go.work` active
   to see the aligned behaviour. See `docs/reference.md` §"Engine-version
   note: the unset instance `tag`".

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
  re-compiles via `@tabnas/abnf`.

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

The Makefile runs all Go commands with **`GOWORK=off`**, which pins the
engine to the published version in `go/go.mod`. A plain `go test`
(workspace on, since `../go.work` lists `./debug/go`) builds against the
sibling `../parser/go` instead.

**CI uses the second one.** `polyglot-ci.yml` clones the sibling repos
and generates a `go.work` over every module that lacks a
`replace => ../vendor/` — `debug/go` has no `replace` at all, so it is
included — then runs a plain `go test`. So CI tests against parser
`main`, *not* the pinned release. Both resolutions pass today; run both
before pushing, since that is the cheapest way to catch either an engine
change that is on `main` but not yet released, or a fixture that only
holds against one of them. Run `gofmt -l .` and `go vet ./...` before
committing Go changes.

The Go module carries a top-level `const VERSION` in `go/debug.go`;
`make publish-go V=x.y.z` seds that const, commits, and tags
`go/vX.Y.Z`. The TypeScript package exports a matching `VERSION` from
`ts/src/debug.ts`. Both MUST equal `ts/package.json` "version":
`go/version_test.go` and `ts/test/version.test.js` fail the build if
either drifts.

## CI

`.github/workflows/ci.yml` is a thin **caller** of the org-shared
reusable workflow `tabnas/.github/.github/workflows/polyglot-ci.yml@main`
(it replaced a local `build.yml`; a maintainer promotes changes to it via
`tabnas/admin`, because session credentials cannot write
`.github/workflows/*`). It passes only the sibling wiring:

```yaml
deps:        "parser json abnf railroad"
build-order: "parser debug json abnf railroad"
```

Everything else — the OS/Node matrix, `core.autocrlf false` (CRLF
corrupts fixtures and golden output), the sibling `git clone --depth 1`,
and the per-repo `npm i && npm run build --if-present` — lives in the
shared workflow, so read it there rather than assuming it here.

`.github/workflows/release.yml` handles publishing.

## Tests mirror each other

`ts/test/debug.test.js` ↔ `go/debug_test.go` + `go/model_test.go`; keep
them aligned. Both sides also run the shared fixtures through
`ts/test/parity.test.js` ↔ `go/parity_test.go`.

TS-only: `ts/test/abnf.test.js` (the `abnf()` ↔ `@tabnas/abnf`
round-trip — the emitter must never gain `@tabnas/abnf` as a runtime
dependency) and `ts/test/doc-examples.test.js`, which executes every
fenced `js` block containing a `// =>` assertion across `README.md`,
`ts/README.md`, `go/README.md`, `ts/doc/` and `docs/`. A doc example with
`// =>` is therefore a TEST: get it wrong and the suite goes red.

When you add a capability, extend `docs/reference.md` and add a how-to if
it introduces a new task. Prefer pinning cross-runtime behaviour in
`test/spec/*.tsv` over a one-off in-language assertion.

Regenerating `test/spec/model.tsv` (canonical TS is the source of the
expected values):

```bash
cd ts && node -e '
const { GRAMMARS } = require("./test/fixture.js")
const byName = (a) => [...a].sort((x, y) => (x.name < y.name ? -1 : x.name > y.name ? 1 : 0))
for (const n of ["bare", "add", "greet"]) {
  const m = GRAMMARS[n]().debug.model()
  console.log(n + "\t" + JSON.stringify(
    JSON.parse(JSON.stringify({ rules: byName(m.rules), graph: byName(m.graph) }))))
}'
```
