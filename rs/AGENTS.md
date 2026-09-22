# Agent guide: rs/ (parity)

This is the Rust port of `@tabnas/debug`. It is **not** canonical: it
tracks the TypeScript implementation in `../ts`, which is the source of
truth. See [../AGENTS.md](../AGENTS.md) for the parity rules and
[../docs/reference.md](../docs/reference.md) — the authoritative
divergence register — for the intentional TS/Go/Rust differences.

## Layout

| File | What it is |
|---|---|
| `src/lib.rs` | The plugin (`plugin`, `apply`), `DebugOptions`, the `use_plugin` wrapper, `VERSION`, and the re-exports. |
| `src/describe.rs` | `describe` — the eight-section text dump. `SECTIONS` holds the banners. |
| `src/model.rs` | `model` and the `Debug*` types, with serde names matching the TS field names and the Go JSON tags. |
| `src/abnf.rs` | `abnf` — the ABNF emitter, and the name sanitiser it needs. |
| `src/trace.rs` | `TraceKinds` and the subscriber wiring. |
| `tests/parity_test.rs` | The shared `../test/spec/*.tsv` fixtures. |
| `tests/common/fixture.rs` | The named grammar registry (`bare`, `add`, `greet`, `collide`). |
| `tests/common/spec.rs` | The TSV loader and the reporter table. |
| `tests/debug_test.rs` | What the fixtures cannot express. |
| `tests/version_test.rs` | The version constants. |

```bash
cargo build --all-targets
cargo test --all-targets
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt
```

`../ci/rust/run.sh` is the full gate and runs more than those: it adds
`cargo fmt --check`, `cargo test --doc` (which `--all-targets` does not
include) and `RUSTDOCFLAGS=-D warnings cargo doc --no-deps`. That last
one is the only command that resolves an intra-doc link. It matters here
because `describe`, `model` and `abnf` are each a public module AND a
re-exported function, so a bare ``[`describe`]`` is ambiguous and
rustdoc drops it; write ``[`describe()`]`` for the function.

The engine crate `tabnas` is a **path dependency on the sibling
checkout** (`../../parser/rs`) — it is not published, so there is no
version to fall back on and no second resolution to keep green. Clone
`https://github.com/tabnas/parser` next to this repo.

## The five things worth knowing before editing

1. **Free functions, and infallible.** `describe` / `model` / `abnf` take
   `&Tabnas` rather than being methods: Rust cannot add methods to a type
   it does not own, so this matches the Go port. Unlike Go they return
   plain values, not `(value, error)` — the Rust engine's accessors
   cannot fail, so there is nothing to surface.

2. **`SECTIONS` is the parity contract.** The eight banners in
   `describe.rs` are pinned byte-for-byte by `../test/spec/sections.tsv`
   and must stay in order. The text BETWEEN them is not pinned and
   legitimately differs between runtimes.

3. **The ABNF emitter must never gain an ABNF dependency.** It reads only
   the live engine. That independence is what makes the round-trip claim
   meaningful, and it is why the fixture grammars are hand-written
   against the engine rather than compiled from ABNF source.

4. **`step` never fires.** The Rust engine has no `ctx.log` and emits no
   per-step event, so `TraceKinds::step` is accepted for option-name
   parity and logs nothing — and a selection of `step` alone installs no
   tracing at all, not even the per-parse banner. `TraceKinds::any_live`
   is what encodes that.

5. **Installing the trace twice must not stack it.** The engine
   accumulates subscribers and parse-prepare hooks, so `trace::install`
   registers its callbacks exactly ONCE per instance and keeps the live
   selection in a `debug.trace` decoration that later installs replace —
   including with `None`, which turns tracing off. `derive` re-runs a
   parent's plugins on the child, so this is not hypothetical. The
   regression tests are `reapplying_*` and
   `deriving_a_child_does_not_stack_trace_subscribers` in
   `tests/debug_test.rs`.

## Where the trace kinds come from

TypeScript drives all six kinds through the engine's `ctx.log`. The Rust
engine has typed subscribers instead, so each kind is wired to the one
that carries its information:

| Kind | Rust source |
|---|---|
| `lex` | `subscribe_lex` |
| `rule` | `subscribe_rules` |
| `stack` | `subscribe_rules` (the context's rule stack) |
| `parse` | `subscribe_rule_done` (the matched alternate) |
| `node` | `subscribe_rule_done` (the completed node) |
| `step` | no engine hook — see above |

`RuleDone` carries the matched alternate's push/replace/back/groups, so
Rust `parse` lines say more than Go's can — though no port reports the
TypeScript alt *index*.

## Ordering

The Rust engine holds rules in an `IndexMap`, so rule order IS the
TypeScript insertion order — Rust matches TS here where Go cannot. Token
sets, tokens and alt counter maps come out of unordered maps, so those
are sorted (by name, by tin, by name) to keep the dump deterministic.

## Spec fixtures

`../test/spec/*.tsv` is the parity contract. TypeScript and Go read the
files through `@tabnas/support`; there is no Rust half, so the loader
lives in `tests/common/spec.rs` and must keep to the same format — see
[`../test/AGENTS.md`](../test/AGENTS.md). Adding a `.tsv` there runs it
here automatically (the runner discovers the directory), but a new
GRAMMAR has to be added to all three registries by hand, because a row
addresses one by name.
