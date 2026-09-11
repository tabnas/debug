/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Shared test scaffolding: the named grammar registry the fixtures
//! address, and the runner for the shared `test/spec/*.tsv` files.
//!
//! Cargo compiles this module into EVERY integration test binary, so a
//! helper only one of them uses reads as dead code in the others. The
//! allow is about that compilation model, not about unused code.

#![allow(dead_code)]

pub mod fixture;
pub mod spec;
