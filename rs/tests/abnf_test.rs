/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Whole-emitter tests for `abnf`, mirroring the `TestAbnf*` set in
//! `../../go/debug_test.go` and the cases in `../../ts/test/abnf.test.js`.
//!
//! What the shared `../../test/spec/abnf.tsv` fixture already pins for
//! every runtime is NOT repeated here: alternation, `%s` literals, the
//! synthetic-optional fold, the rule/token name collision and the
//! prose-val fallback all have rows there. What is left are the grammar
//! SHAPES the fixture registry does not hold, each of which needs a
//! purpose-built instance.
//!
//! The canonical TypeScript proves most of these by round-tripping
//! through `@tabnas/abnf`. That suite cannot run here, or in Go: the
//! emitter must never gain an ABNF dependency, and neither port may take
//! one even in a test without putting that independence claim in doubt.
//! So both ports pin the same shapes by asserting the emitted text, and
//! this file is the Rust half.

mod common;

use tabnas::{AltSpec, Tabnas, Tin};
use tabnas_debug::abnf;

/// Resolve a built-in token identity, failing loudly if the engine has
/// renamed it.
fn token(parser: &Tabnas, name: &str) -> Tin {
    parser
        .options
        .token(name)
        .unwrap_or_else(|| panic!("the engine has no {name} token"))
}

/// The parts of RFC 5234 the emitter has violated before:
///
/// ```text
/// rulename    = ALPHA *(ALPHA / DIGIT / "-")
/// alternation = concatenation *(*c-wsp "/" *c-wsp concatenation)
/// ```
///
/// so `_gen1_star_x` is not a legal name, and `x = A x /` is not a legal
/// body. Mirrors `assertRfc5234Shape` in the TypeScript and Go suites.
fn assert_rfc5234_shape(out: &str) {
    for line in out.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with(';') {
            continue;
        }
        assert!(
            !trimmed.ends_with('/'),
            "dangling `/`, every `/` needs a concatenation after it:\n{line}\n--- in ---\n{out}"
        );
        if let Some(head) = head_name(line) {
            assert!(
                is_legal_rulename(head),
                "rulename is not ALPHA *(ALPHA / DIGIT / \"-\"):\n{line}\n--- in ---\n{out}"
            );
        }
    }
}

/// The rule name a production line defines, i.e. everything before the
/// first `=`, when the line has one. Mirrors the `^([^\s=]+)\s*=` the
/// other two suites use.
fn head_name(line: &str) -> Option<&str> {
    let (head, _) = line.split_once('=')?;
    let head = head.trim();
    if head.is_empty() || head.contains(char::is_whitespace) {
        return None;
    }
    Some(head)
}

fn is_legal_rulename(name: &str) -> bool {
    let mut chars = name.chars();
    chars
        .next()
        .is_some_and(|first| first.is_ascii_alphabetic())
        && chars.all(|ch| ch.is_ascii_alphanumeric() || '-' == ch)
}

/// The shape the ABNF compiler emits for a repetition (`rep = *T`): a
/// `_gen…_star_…` production whose empty open alternative marks it
/// zero-or-more. Unlike `[ … ]`, repetition uses a probe-optimised
/// subgraph that does not reconstruct as `*( … )` reliably, so the
/// emitter must KEEP it as a production rather than fold it.
fn star_grammar() -> Tabnas {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "rep".into();
    let plus = parser.token_with_source("#T", "+");

    parser.define_rule("rep", |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            p: Some("_gen1_star_T".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });
    parser.define_rule("_gen1_star_T", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![plus]],
            ..Default::default()
        });
        // The empty alternative is what makes it zero-or-more.
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec::new());
    });
    parser
}

/// A `_gen…_star_…` synthetic is NOT folded. It stays a bareword
/// reference in its parent and is emitted as its own production. Beyond
/// the folding rule this pins two things RFC 5234 requires: the empty
/// alternative renders as `[ … ]` rather than a trailing `/`, and the
/// synthetic name is sanitised to a legal rulename, so `_gen1_star_T`
/// comes out as `r-gen1-star-T`.
#[test]
fn abnf_keeps_a_repetition_production() {
    let out = abnf(&star_grammar());
    assert_eq!(
        out, "rep = r-gen1-star-T\nr-gen1-star-T = [ T ]\n\nT = \"+\"",
        "repetition mismatch:\n{out}"
    );
    assert!(
        !out.contains("_gen"),
        "a synthetic _gen name leaked:\n{out}"
    );
    assert_rfc5234_shape(&out);
}

/// The num-val fallback for a fixed token whose literal cannot go inside
/// quotes. RFC 5234 has `char-val = DQUOTE *(%x20-21 / %x23-7E) DQUOTE`,
/// so a control character, a DQUOTE, or anything above `%x7E` has to come
/// back as `%xNN`. Quoting a CR emitted `CR = "<CR>"`, an unterminated
/// char-val.
#[test]
fn abnf_renders_an_unquotable_literal_as_a_num_val() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let cr = parser.token_with_source("#CR", "\r");
    let dq = parser.token_with_source("#DQ", "\"");

    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![cr], vec![dq]],
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    for want in ["CR = %x0D", "DQ = %x22"] {
        assert!(out.contains(want), "missing {want:?} in:\n{out}");
    }
    assert_rfc5234_shape(&out);
}

/// The multi-character, padding and non-ASCII cases of the num-val
/// rendering, driven through the public surface rather than the private
/// helper.
#[test]
fn abnf_num_val_covers_multi_char_padding_and_non_ascii() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let crlf = parser.token_with_source("#CRLF", "\r\n");
    let nul = parser.token_with_source("#NUL", "\0");
    let acc = parser.token_with_source("#ACC", "\u{e9}");
    let tab = parser.token_with_source("#TAB", "\t");

    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![crlf], vec![nul], vec![acc], vec![tab]],
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    for want in [
        "CRLF = %x0D.0A", // multi-character, dot-concatenated
        "NUL  = %x00",    // zero-padded to two hex digits
        "ACC  = %xE9",    // non-ASCII
        "TAB  = %x09",    // control character
    ] {
        assert!(out.contains(want), "missing {want:?} in:\n{out}");
    }
    assert_rfc5234_shape(&out);
}

/// A rule whose only open alternative is empty, but which has a close
/// continuation, emits the continuation alone.
/// `option = "[" *c-wsp alternation *c-wsp "]"` and `alternation` needs
/// at least one concatenation, so `[ ]` is not a legal option. This used
/// to emit `inner = [  ] AA`.
#[test]
fn abnf_emits_a_continuation_alone_not_an_empty_option() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let xa = parser.token_with_source("#XA", "a");
    let zz = token(&parser, "#ZZ");

    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            p: Some("inner".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![zz]],
            ..Default::default()
        });
    });
    parser.define_rule("inner", move |spec| {
        spec.clear();
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec {
            s: vec![vec![xa]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![zz]],
            ..Default::default()
        });
    });

    let out = abnf(&parser);
    assert!(!has_empty_option(&out), "an empty `[ ]` option in:\n{out}");
    assert_rfc5234_shape(&out);
}

/// `[` then only whitespace then `]`, the shape the other two suites
/// match with the regex `\[\s*\]`.
fn has_empty_option(out: &str) -> bool {
    let bytes: Vec<char> = out.chars().collect();
    for (index, ch) in bytes.iter().enumerate() {
        if '[' != *ch {
            continue;
        }
        let mut cursor = index + 1;
        while cursor < bytes.len() && bytes[cursor].is_whitespace() {
            cursor += 1;
        }
        if cursor < bytes.len() && ']' == bytes[cursor] {
            return true;
        }
    }
    false
}

/// RFC 5234 §2.1: rule names are case-insensitive, so `Foo-Bar` and
/// `foo-bar` are ONE rule. Sanitising `foo_bar` beside a reserved
/// `Foo-Bar` used to emit two definitions of the same rule.
#[test]
fn abnf_rulename_collisions_are_case_insensitive() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let xa = parser.token_with_source("#XA", "a");
    let xb = parser.token_with_source("#XB", "b");
    let zz = token(&parser, "#ZZ");

    parser.define_rule("top", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![xa]],
            p: Some("Foo-Bar".into()),
            ..Default::default()
        });
        spec.add_open(AltSpec {
            s: vec![vec![xb]],
            p: Some("foo_bar".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![zz]],
            ..Default::default()
        });
    });
    parser.define_rule("Foo-Bar", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![xa]],
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });
    parser.define_rule("foo_bar", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![xb]],
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    let mut seen: Vec<String> = Vec::new();
    for line in out.lines() {
        let Some(head) = head_name(line) else {
            continue;
        };
        let key = head.to_lowercase();
        assert!(
            !seen.contains(&key),
            "two productions define the same rule (case-insensitively): {head:?} in:\n{out}"
        );
        seen.push(key);
    }
    assert_rfc5234_shape(&out);
}

/// The skip branch of an optional arrives as a FIRST-set-guarded epsilon:
/// an alt carrying the FOLLOW token in `s` with `b` set and NO
/// push/replace target. The token is lookahead, matched to choose the
/// branch and then backtracked, so the alt consumes nothing
/// (`s.len() - b == 0`).
///
/// The emitter used to skip `s` only when `b` was set AND the alt pushed
/// or replaced, so this shape rendered as a CONSUMING alternative:
/// `top = [ X "@" ] Y` came back as `top = [ X T / Y ] Y`, where the
/// optional could swallow the follow and leave nothing for the trailing
/// `Y`.
#[test]
fn abnf_does_not_render_a_follow_guarded_epsilon_as_an_alternative() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "top".into();
    let at = parser.token_with_source("#T", "@");
    let yy = parser.token_with_source("#Y", "b");
    let xx = parser.options.register_token("#X");

    parser.define_rule("top", |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            p: Some("_gen2_opt__gen1_group".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec {
            r: Some("top$step1".into()),
            ..Default::default()
        });
    });
    parser.define_rule("top$step1", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![yy]],
            ..Default::default()
        });
    });
    parser.define_rule("_gen1_group", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![xx], vec![at]],
            ..Default::default()
        });
    });
    parser.define_rule("_gen2_opt__gen1_group", move |spec| {
        spec.clear();
        // Take the optional: peek #X, the pushed group consumes it.
        spec.add_open(AltSpec {
            s: vec![vec![xx]],
            b: 1,
            p: Some("_gen1_group".into()),
            ..Default::default()
        });
        // Skip it: peek the FOLLOW #Y and consume nothing. The shape at
        // issue.
        spec.add_open(AltSpec {
            s: vec![vec![yy]],
            b: 1,
            ..Default::default()
        });
        // Bare epsilon.
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    for line in out.lines() {
        if !line.trim_start().starts_with("top") {
            continue;
        }
        // The follow token must not appear as an alternative INSIDE the
        // option. `top = [ X T ] Y` is right; `top = [ X T / Y ] Y` is
        // the defect.
        if let Some(close) = line.find(']') {
            assert!(
                !line[..close].contains('/'),
                "a follow-guarded epsilon rendered as a consuming alternative:\n{line}\n--- in ---\n{out}"
            );
        }
    }
    assert_rfc5234_shape(&out);
}

/// Every grammar in the shared registry emits ABNF that obeys the two
/// RFC 5234 shapes above. The `.tsv` fixture pins the exact bytes for
/// each; this pins the property, so a new fixture grammar cannot land an
/// illegal name or a dangling `/` merely by having its row regenerated
/// from the canonical runtime.
#[test]
fn every_fixture_grammar_emits_rfc5234_shaped_abnf() {
    for name in ["bare", "add", "greet", "collide"] {
        let parser = common::fixture::build(name).expect("a known grammar");
        assert_rfc5234_shape(&abnf(&parser));
    }
}
