/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Cross-runtime conformance, driven by the shared `test/spec/*.tsv`
//! fixtures at the repo root (see `test/AGENTS.md`).
//!
//! A row names a GRAMMAR from the shared registry, and the second
//! column's header names what is reported about it: the emitted ABNF, the
//! `describe()` section banners, or the grammar-structure portion of the
//! model. A row green in one runtime and red in another is a failure, not
//! a discrepancy.

mod common;

#[test]
fn spec() {
    common::spec::run_spec_dir();
}
