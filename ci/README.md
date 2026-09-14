# ci/

Staging area for GitHub Actions workflow changes.

This directory exists because session credentials cannot write
`.github/workflows/*` — see admin `DECISIONS.md` ADR-8. To change CI:

1. Put the intended workflow file in `workflows/`.
2. A maintainer promotes it with the admin `rollout/apply-ci-folders.sh`
   script.

## Pending

- **`workflows/docs.yml`** — the prose gate: Vale over the reader-facing
  pages at the levels set in `.vale.ini`, on the file list
  `ts/scripts/gated-docs.cjs` produces. See `docs/STYLE-GUIDE.md`.

  It needs no sibling checkouts and no secrets, and pins its own Vale
  version. Errors fail the job; warnings go to the run summary as a
  report. `make prose` runs the identical check locally, and the test
  suite already runs the other half of the gate
  (`ts/test/docs.test.js`), so promoting this adds the spelling and
  Google-convention arm rather than the whole gate.

- **`workflows/rust.yml`** — the Rust gate: `rs/` built, tested,
  `rustfmt`-checked and clippy-clean at `-D warnings`. The commands live
  in `ci/rust/run.sh`, which the workflow calls and you can run too;
  `make test-rs` stays the fast inner loop.

  **Nothing tests `rs/` remotely today.** `ci.yml` calls the org-shared
  polyglot workflow, which takes no Rust input, so the Rust port has
  been proved only by hand since it landed. This workflow is standalone
  and needs no change in `tabnas/.github` to run.

  It clones the sibling `tabnas/parser` itself, because `rs/Cargo.toml`
  takes the engine as a path dependency and the crate is unpublished.
  That is also why it does **not** pass `--locked`: the lock records the
  engine by version, so once parser bumps, `--locked` would fail every
  pull request here, including ones touching no Rust. See the comment in
  `ci/rust/run.sh`.

  One thing to settle at promotion: `dtolnay/rust-toolchain` is
  referenced by tag, not SHA, matching `tabnas/parser`'s own staged
  `ci/workflows/rust.yml`. Every other action here is SHA-pinned.
