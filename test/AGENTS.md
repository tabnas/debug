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
| `abnf`, `sections` *or* `model` | See below. |

The **second column's header name selects what the runner reports**:

- `abnf` — `tn.debug.abnf()` / `Abnf(j)`, the emitted ABNF, as a JSON string.
- `sections` — the `describe()` section banners, in order, as a JSON array
  of strings.
- `model` — the *grammar-structure* portion of `tn.debug.model()` /
  `Model(j)` as a JSON object: `{"rules": …, "graph": …}`. This pins the
  cross-runtime serialisation claim in `../docs/reference.md` — that the Go
  `DebugModel`'s JSON tags match the TS field names, so the two runtimes'
  models are comparable once decoded.

  Two deliberate normalisations make `model` shareable. Both runners sort
  `rules` and `graph` by name, because the orderings differ by design (TS
  insertion order, Go by name). And the *instance-level* sections —
  `lexer`, `plugins`, `tag` — are excluded: the Go engine exposes only
  custom lexer matchers, and the Go registry's grammars do not load the
  debug plugin (in Go `Describe`/`Model` are package functions, so they
  need not). Comparison is on decoded JSON, so field order is irrelevant.

  `tag` is a different case, and a temporary one. Both engines now default
  an unset tag to `-` (the engine exports `tabnas.DefaultTag`), so the two
  runtimes agree — but only against the sibling engine checkout. Under
  `GOWORK=off`, as `make test` runs it, the Go suite resolves
  `github.com/tabnas/parser/go` from `go/go.mod` (v0.6.1, pre-alignment)
  and that engine still leaves the tag empty. CI resolves the other way
  (workspace on, sibling `main`) and gets `-`. A shared fixture has to
  pass under both, so `tag` stays out. Add it once `go/go.mod` moves past
  the alignment; see `../docs/reference.md` §"Engine-version note: the
  unset instance `tag`".

  A row's expected value is written by the CANONICAL TypeScript side; see
  the note in `../AGENTS.md` on regenerating it.

## The grammar registry

`ts/test/fixture.js` (`GRAMMARS`) and `go/fixture_test.go` (`grammars`) hold
the same named grammars — `bare`, `add`, `greet`. A fixture row addresses one
by name, so **both registries must stay in step**; adding a grammar means
adding it to both.

The grammars are hand-written against the engine on purpose: `@tabnas/abnf`
must NOT become a dependency of `@tabnas/debug` (the emitter reads only the
live engine), so no fixture may be compiled from ABNF source.

## Who runs what

- TypeScript: `ts/test/parity.test.js` — a `makeRunner(...)` per fixture.
- Go: `go/parity_test.go` — a `support.Runner{...}` per fixture.

One runner per FILE, not one over the directory, because the second
column's header names the reporter. Both hold only what is specific to
debug: that reporter table, and the normalisations that make its output
comparable across runtimes. Everything else — finding `test/spec`,
reading the file, the comparison, the `<file>:<line>` in a failure
message — comes from
[`@tabnas/support`](https://github.com/tabnas/support) and its Go half,
so the two loaders cannot drift from each other either.

Both discover files by directory listing: adding a `.tsv` here runs it in
both runtimes without touching either runner. An empty fixture, and a spec
directory with no fixtures in it, both **fail**.

## Rules

- Prefer adding a fixture here over a one-off in-language assertion when a
  case is expressible as grammar → report. That is what keeps the two
  runtimes honest against each other.
- TypeScript is canonical. If the two runtimes disagree, the TS behaviour is
  the expected value — unless Go has exposed a genuine TS defect, in which
  case fix TS first and pin the corrected behaviour here.
- A new fixture must pass in BOTH runtimes: run `go test ./...` (from `go/`)
  and `npm test` (from `ts/`) before considering it done.
