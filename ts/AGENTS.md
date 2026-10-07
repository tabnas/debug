# Agent guide: ts/ (canonical)

This is the **canonical** TypeScript implementation of `@tabnas/debug`.
Behaviour defined here is the source of truth; the Go port in `../go`
tracks it. See [../AGENTS.md](../AGENTS.md) for the parity rules.

- Source: `src/debug.ts`. Output format, option names (`DebugOptions`),
  `DEFAULTS`, the `DebugModel` shape, and the `describe()` section order
  all originate here.
- Tests (Node's built-in runner): `test/debug.test.js` (describe, trace,
  print, model), `test/abnf.test.js` (the `abnf()` round-trip through
  `@tabnas/abnf`, loaded by sibling PATH — never a dependency),
  `test/parity.test.js` (the shared `../test/spec/*.tsv` fixtures) and
  `test/doc-examples.test.js` (executes doc snippets containing `// =>`).
- The engine dependency `@tabnas/parser` is a `"*"` devDependency, so
  `npm install` installs the published engine. To test against sibling
  `main`, build `../../parser/ts` and run `admin/scripts/link.sh`, which
  replaces `node_modules/@tabnas/parser` with a **symlink to the sibling**;
  a later `npm install` or `npm ci` puts the registry copy back.
  `../scripts/fetch-parser.sh` is legacy and is not a prerequisite.

```bash
npm run build
npm test
```

When you change behaviour here, update the Go port to match within the
Go engine's API limits (`../go/debug.go`, `../go/model.go`,
`../go/trace.go`) and refresh `../docs/reference.md` in the same change.
If the change is expressible as grammar → report, pin it in
`../test/spec/*.tsv` so all three runtimes enforce it.
