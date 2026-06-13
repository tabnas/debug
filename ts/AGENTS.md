# Agent guide: ts/ (canonical)

This is the **canonical** TypeScript implementation of `@tabnas/debug`.
Behaviour defined here is the source of truth; the Go port in `../go`
tracks it. See [../AGENTS.md](../AGENTS.md) for the parity rules.

- Source: `src/debug.ts`. Output format, option names (`DebugOptions`),
  `DEFAULTS`, and the `describe()` section order all originate here.
- Tests: `test/debug.test.js` (Node's built-in test runner).
- The engine dependency `tabnas` is referenced as
  `file:../vendor/tabnas-parser/ts` and must be fetched first with
  `../scripts/fetch-parser.sh` (the root Makefile does this).

```bash
../scripts/fetch-parser.sh   # once, from a checkout
npm install
npm run build
npm test
```

When you change behaviour here, update `../go/debug.go` to match (within
the Go engine's API limits) and refresh `../docs` in the same change.
