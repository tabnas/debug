# Agent guide: ts/ (canonical)

This is the **canonical** TypeScript implementation of `@tabnas/debug`.
Behaviour defined here is the source of truth; the Go port in `../go`
mirrors it. See [../AGENTS.md](../AGENTS.md) for the parity rules.

- Source: `src/debug.ts`. Output format, option names (`DebugOptions`),
  `DEFAULTS`, and the `describe()` section order all originate here.
- Tests: `test/debug.test.js` (Node's built-in test runner).
- The `tabnas` parser dependency is pinned to GitHub `main`
  (`github:rjrodger/tabnas#main`) and must be installed to build or test.

```bash
npm i
npm run build
npm test
```

When you change behaviour here, mirror it in `../go/debug.go` and update
`../docs` in the same change.
