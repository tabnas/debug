# Agents Guide — shared spec fixtures

`spec/*.tsv` holds the cross-runtime conformance fixtures. Both runtimes
auto-discover and run **every** file in this directory, so a change here
affects TypeScript and Go together — edit with that in mind.

These replaced `test/headers.golden`, which pinned the section headers for
one instance in a bespoke format; `sections.tsv` pins them per grammar in
the shared one.

## Format

Tab-separated, one case per line, with a header row naming the columns.
Blank lines are skipped, and so are comment lines — a line starting with
`#` that contains no tab. (A data row always has at least one tab.)

| Column | Meaning |
|---|---|
| `grammar` | The NAME of a grammar in the shared fixture registry (see below). Debug reports on a live engine, so a fixture names a grammar rather than carrying source text. |
| `abnf` *or* `sections` | See below. |

The **second column's header name selects what the runner reports**:

- `abnf` — `tn.debug.abnf()` / `Abnf(j)`, the emitted ABNF, as a JSON string.
- `sections` — the `describe()` section banners, in order, as a JSON array
  of strings.

## The grammar registry

`ts/test/fixture.js` (`GRAMMARS`) and `go/fixture_test.go` (`grammars`) hold
the same named grammars — `bare`, `add`, `greet`. A fixture row addresses one
by name, so **both registries must stay in step**; adding a grammar means
adding it to both.

The grammars are hand-written against the engine on purpose: `@tabnas/abnf`
must NOT become a dependency of `@tabnas/debug` (the emitter reads only the
live engine), so no fixture may be compiled from ABNF source.

## Who runs what

- TypeScript: `ts/test/parity.test.js` — reads `../../test/spec`.
- Go: `go/parity_test.go` — `TestSpec` globs `../test/spec/*.tsv`.

Both discover files by directory listing: adding a `.tsv` here runs it in
both runtimes without touching either runner.

## Rules

- Prefer adding a fixture here over a one-off in-language assertion when a
  case is expressible as grammar → report. That is what keeps the two
  runtimes honest against each other.
- TypeScript is canonical. If the two runtimes disagree, the TS behaviour is
  the expected value — unless Go has exposed a genuine TS defect, in which
  case fix TS first and pin the corrected behaviour here.
- A new fixture must pass in BOTH runtimes: run `go test ./...` (from `go/`)
  and `npm test` (from `ts/`) before considering it done.
