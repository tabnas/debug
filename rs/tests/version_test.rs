/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! The exported `VERSION` must equal both `rs/Cargo.toml`'s `version` and
//! `ts/package.json`'s `"version"`.
//!
//! This is the CI check for version drift. It exists because the constant
//! HAS drifted in practice: `@tabnas/json` shipped a TS `Version` export
//! reading 1.0.0 for several releases because nothing ever rewrote it,
//! and it was invisible until someone read the file. A release that bumps
//! `package.json` and forgets a constant now fails here instead of
//! shipping a lie.
//!
//! The read below is deliberately unguarded — a version check that
//! silently skips is the exact failure mode being designed out.

use std::path::Path;

use tabnas_debug::VERSION;

/// `ts/package.json`'s name and version.
fn package_json() -> (String, String) {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent directory")
        .join("ts")
        .join("package.json");
    let raw = std::fs::read_to_string(&path).unwrap_or_else(|error| {
        panic!(
            "cannot read {}, so VERSION cannot be checked: {error}",
            path.display()
        )
    });
    let parsed: serde_json::Value =
        serde_json::from_str(&raw).expect("ts/package.json is readable JSON");
    let name = parsed["name"]
        .as_str()
        .expect("ts/package.json has a name field")
        .to_string();
    let version = parsed["version"]
        .as_str()
        .expect("ts/package.json has a version field")
        .to_string();
    (name, version)
}

#[test]
fn version_matches_cargo_toml() {
    assert_eq!(
        VERSION,
        env!("CARGO_PKG_VERSION"),
        "VERSION drift: src/lib.rs exports {VERSION} but Cargo.toml is {}",
        env!("CARGO_PKG_VERSION")
    );
}

#[test]
fn version_matches_package_json() {
    let (name, version) = package_json();
    assert_eq!(
        VERSION, version,
        "VERSION drift: the Rust crate exports {VERSION} but {name} \
         package.json is {version}. All of them are rewritten by \
         admin/publish.sh at release; if you bumped one by hand, bump the \
         others."
    );
}

#[test]
fn version_looks_like_a_semver() {
    let mut parts = VERSION.split('.');
    let numeric = |part: Option<&str>| {
        part.is_some_and(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_digit()))
    };
    assert!(
        numeric(parts.next()) && numeric(parts.next()),
        "VERSION must be a semver, got {VERSION}"
    );
}
