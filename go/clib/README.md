# libtabnasdebug — the debug parser as a C ABI

<!-- tabnas-clib-template: v5 — stamped by admin tasks/adopt-clib.sh;
     edit the template and re-stamp, not this file. -->

The debug format parser as a C shared library, so languages with no
tabnas port can parse and validate debug input. This is one of the
per-format tabnas clibs sharing the **uniform ABI** decided by ADR-12:
every such library exports the same five symbols, and which library you
load decides which format you parse — so one generic binding per
language covers the whole fleet. Two format clibs consequently cannot
be statically linked into one binary; load them dynamically.

```sh
./build.sh            # host, into ./dist
ZIG=/path/to/zig ./build.sh all
```

## The contract

| Function | Returns |
|---|---|
| `tabnas_version()` | `{"ok":true,"lib":"libtabnasdebug","format":"debug","template":"v5"}` |
| `tabnas_grammar(opts, len)` | `{"ok":true,"handle":N}` — opts reserved, pass `(NULL, 0)`, unless the format notes below define them |
| `tabnas_parse(handle, src, len)` | `{"ok":true,"accept":true[,"value":…]}` or `{"ok":true,"accept":false,"error":{…}}` |
| `tabnas_grammar_free(handle)` | — |
| `tabnas_free(str)` | — |

The rules every tabnas clib shares, each load-bearing:

1. **Every call returns JSON.** A C ABI has one return value and no
   exceptions; each entry point returns a document, so a binding in any
   language is *call, decode* and the error contract is identical
   everywhere.
2. **Three outcomes, not two.** A broken call is `ok:false` with a
   code; input outside the language is `ok:true, accept:false`; an
   accepted input is `ok:true, accept:true` — plus `value` where the
   parse result is JSON-representable.
3. **A rejection is an answer, not a failure.**
4. **Lengths are explicit.** Buffers are not read as NUL-terminated C
   strings; input may legitimately contain a zero byte.
5. **The caller owns what it is given.** Every `char*` must be released
   with `tabnas_free` (malloc'd — `free(3)`); every handle with
   `tabnas_grammar_free`.

Handles are safe to use from several threads: each carries a mutex,
because the underlying engine is not safe for concurrent Parse and an
FFI caller is under no obligation to serialise.

The grammar is installed **natively** — compiled in-process, not
serialized and reloaded. Lexing configuration is part of the accepted
language, and format plugins keep format-specific behaviour as
closures, which cannot cross a data boundary; see
`admin/notes/2026-08-16-clib-ffi-strategy.md` for the full account.

## Consuming without a binding

C, C++, Zig, Swift, Nim and D consume `include/tabnas.h` directly — no
binding layer exists or is needed. Zig example:

```zig
const c = @cImport(@cInclude("tabnas.h"));
// link against libtabnasdebug; every call returns a JSON []u8 to free with
// c.tabnas_free.
```

## Format notes

The library runs on the engine, not on another grammar: `tabnas_grammar`'s argument is DEFINED, as in libtabnasparser, and is a serialized GrammarSpec (the JSON `Tabnas.grammarSpec()` / `GrammarSpecFromJSON` exchange), which is installed first; the Debug plugin is then installed on it with all six trace kinds on and the trace captured per handle rather than written to the host process's stdout. debug is not a format but the parse-tracing plugin, so it traces whatever grammar the spec defines. An accepted input's `value` is `{"value": <parsed value>, "trace": "<trace text>"}`, not the bare parsed value; a rejection carries the ordinary diagnostic and no trace, because the ABI has no slot for one. The trace is informative text, not a contract, and it is expensive. Node and rule-state lines serialise the whole growing node, so for flat input the trace runs to over a thousand times the input, and it grows quadratically with nesting depth (measured: about 0.56 MB at depth 50 and 120 MB at depth 800). A deeply nested input of a few tens of kilobytes can therefore exhaust the host's memory; feed this library small inputs and use libtabnasparser to validate. `describe`, `model` and `abnf` have no slot in the five-symbol ABI. Handle creation runs a canary parse that must produce a trace, so if the plugin stops wiring its trace, `tabnas_grammar` fails loudly instead of every parse returning an empty trace.

## Layout

- `core.go` — the behaviour, in plain Go (testable).
- `tabnas_c.go` — the cgo shim: `(pointer, length)` in, malloc'd string
  out, nothing else. (Go forbids cgo in `_test.go`, which is why the
  behaviour lives in `core.go`.)
- `core_test.go` — the contract: accept/reject samples, unknown-handle,
  reserved options, double-free, concurrency under `-race`.
- `include/tabnas.h`, `tabnas.pc.in` — the header and pkg-config file
  for C-header-native consumers.
