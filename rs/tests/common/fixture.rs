/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Named grammar fixtures shared by the spec runner and the in-language
//! tests. The TypeScript counterparts are in `ts/test/fixture.js` and the
//! Go ones in `go/fixture_test.go` — all three registries must stay in
//! step, because `test/spec/*.tsv` addresses a grammar by NAME and every
//! runtime must build the same one.
//!
//! The grammars are hand-written against the engine on purpose: an ABNF
//! compiler must NOT become a dependency of the debug plugin (the emitter
//! reads only the live engine), so no fixture may be compiled from ABNF
//! source here.
//!
//! Like the Go registry, these do not install the debug plugin: in Rust
//! `describe` / `model` / `abnf` are free functions, so they need not.

use regex::Regex;
use tabnas::{AltSpec, MatchToken, MatchTokenMatcher, Tabnas};

/// bare: the engine with nothing installed. Pins what `describe` emits
/// for an instance with no grammar at all.
pub fn bare() -> Tabnas {
    Tabnas::new()
}

/// add: `val` pushes `add`; `add` matches `#NR` then optionally a
/// `#PL`-replace back into `add`, with an epsilon close and the `#ZZ` end
/// close. Exercises the emitter's optional folding (`[ PL add ]`) and
/// token definitions.
pub fn add() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "val".into();
    let plus = parser.token_with_source("#PL", "+");
    let number = token(&parser, "#NR");
    let end = token(&parser, "#ZZ");

    parser.define_rule("val", |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            p: Some("add".into()),
            ..Default::default()
        });
    });
    parser.define_rule("add", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![number]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![plus]],
            r: Some("add".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    parser
}

/// greet: a two-way alternation over case-sensitive literals. Exercises
/// the emitter's `/` alternation and `%s"…"` literal rendering.
pub fn greet() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "greet".into();
    let hi = parser.token_with_source("#HI", "hi");
    let hello = parser.token_with_source("#HE", "hello");
    let end = token(&parser, "#ZZ");

    parser.define_rule("greet", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![hi]],
            ..Default::default()
        });
        spec.add_open(AltSpec {
            s: vec![vec![hello]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    parser
}

/// collide: the two ABNF cases that used to emit invalid output, in one
/// grammar. A rule named `NR` sits beside the built-in `#NR` number token,
/// so the emitter has to keep rule and token names apart — sharing one
/// namespace gave `NR = NR` plus a duplicate `NR = <number>`. And `#WD` is
/// a match token whose regex is neither a char range nor a
/// case-insensitive literal, the case with no ABNF form: it must still
/// emit an ELEMENT (an RFC 5234 prose-val), because the bare `; /…/`
/// comment it used to emit left the legend entry with nothing in it.
pub fn collide() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "NR".into();
    let wd = parser.options.register_token("#WD");
    parser.options.match_tokens.insert(
        "#WD".into(),
        MatchToken {
            name: "#WD".into(),
            tin: wd,
            matcher: MatchTokenMatcher::Regex(Regex::new("^[a-z]+[0-9]*").unwrap()),
            eager: false,
        },
    );
    let nr = token(&parser, "#NR");
    let end = token(&parser, "#ZZ");

    parser.define_rule("NR", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![nr]],
            ..Default::default()
        });
        spec.add_open(AltSpec {
            s: vec![vec![wd]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
    parser
}

/// Resolve a built-in token identity, failing loudly if the engine has
/// renamed it out from under the fixture.
fn token(parser: &Tabnas, name: &str) -> tabnas::Tin {
    parser
        .options
        .token(name)
        .unwrap_or_else(|| panic!("the engine has no {name} token"))
}

/// The registry the spec fixtures address by name.
pub fn build(name: &str) -> Option<Tabnas> {
    match name {
        "bare" => Some(bare()),
        "add" => Some(add()),
        "greet" => Some(greet()),
        "collide" => Some(collide()),
        _ => None,
    }
}
