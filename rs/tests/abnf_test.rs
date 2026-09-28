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
//!
//! The repeat-loop shapes (`loop_*` and `abnf_renders_a_*_loop_*` below)
//! are held by this set ALONE for now: tabnas/bnf#80 changed how every
//! repetition compiles, and the Rust emitter leads the canonical
//! TypeScript and the Go port there (`docs/reference.md`, "The repeat
//! loop: the Rust port leads"). When those follow, their suites take the
//! same shapes.

mod common;

use std::collections::HashMap;

use tabnas::{AltSpec, CompareOp, Condition, RuleSpec, Tabnas, Tin, Value};
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

/// The shape the ABNF compiler emitted for a repetition (`rep = *T`)
/// before tabnas/bnf#80: a `_gen…_star_…` production whose empty open
/// alternative marks it zero-or-more, and whose item alternative would
/// push the rule again for the next item (a push chain; this hand-built
/// one stops at a single item). It carries no self-replace entry, so it
/// is not a loop, and it does not reconstruct as `*( … )` reliably, so
/// the emitter must KEEP it as a production rather than fold it — exactly
/// as it did before the loop shape existed.
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

/// An old-shape `_gen…_star_…` synthetic is NOT folded. It stays a
/// bareword reference in its parent and is emitted as its own production.
/// Beyond the folding rule this pins two things RFC 5234 requires: the
/// empty alternative renders as `[ … ]` rather than a trailing `/`, and
/// the synthetic name is sanitised to a legal rulename, so `_gen1_star_T`
/// comes out as `r-gen1-star-T`. The pinned text is unchanged by the loop
/// rendering: a rule without the self-replace entry renders as before.
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

// ---------------------------------------------------------------------
// The repeat loop: the shape every repetition compiles to since
// tabnas/bnf#80. Hand-built here, alternate for alternate, from what the
// compiler emits (the emitter must never gain an ABNF dependency, so no
// test compiles ABNF source), for `*A` with helper `H`:
//
//   H             open   { c: {n.rep: 0}, n: {rep: 1}, r: H }   entry: allocate, count
//                        { s: FIRST(A), b: 1, r: H$alt0 } …     continue (a ref item)
//                        { s: A, r: H }                         continue (a terminal item)
//                        { s: FOLLOW(H), b: 1 }  { }            exits
//   H$alt0        open   { p: A, n: {rep: 0} }                  push the item
//                 close  { r: H$alt0$step1, n: {rep: 1} }       capture it
//   H$alt0$step1  open   { r: H }                               back to the loop
//
// and the whole grammar wrapped in the compiler's `__start__`.
// ---------------------------------------------------------------------

/// The `__start__` wrapper the bnf compiler puts round every grammar: its
/// open pushes the real start rule and its close matches `#ZZ`. The
/// emitter sees through it and leads with the real start.
fn wrap_start(parser: &mut Tabnas, start: &str) {
    let end = token(parser, "#ZZ");
    parser.options.rule.start = "__start__".into();
    let start = start.to_string();
    parser.define_rule("__start__", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            p: Some(start),
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![end]],
            ..Default::default()
        });
    });
}

/// A loop's entry: consumes nothing, replaces the rule with itself,
/// guarded by the iteration counter it bumps. Bookkeeping, not syntax.
fn loop_entry(name: &str) -> AltSpec {
    counted(
        AltSpec {
            c: vec![Condition {
                path: vec!["n".into(), "rep".into()],
                op: CompareOp::Eq,
                value: Value::Number(0.0),
            }],
            r: Some(name.into()),
            ..Default::default()
        },
        1,
    )
}

fn counted(alt: AltSpec, rep: i32) -> AltSpec {
    AltSpec {
        n: HashMap::from([("rep".to_string(), rep)]),
        ..alt
    }
}

/// A loop's exits: peek the FOLLOW token and give it back, then anything
/// else (and the end of the input).
fn loop_exits(spec: &mut RuleSpec, follow: Tin) {
    spec.add_open(AltSpec {
        s: vec![vec![follow]],
        b: 1,
        ..Default::default()
    });
    spec.add_open(AltSpec::new());
}

/// `*A` over a terminal item: the continue matches the token and
/// re-enters the loop directly.
fn terminal_loop(parser: &mut Tabnas, name: &str, item: Tin, follow: Tin) {
    let rule = name.to_string();
    parser.define_rule(name, move |spec| {
        spec.clear();
        spec.add_open(loop_entry(&rule));
        spec.add_open(AltSpec {
            s: vec![vec![item]],
            r: Some(rule.clone()),
            ..Default::default()
        });
        loop_exits(spec, follow);
    });
}

/// `*A` over a rule item: one continue per FIRST token of the item,
/// each peeking it and replacing with the iteration `H$alt0`, which
/// pushes the item and, on close, replaces with `H$alt0$step1`, which
/// replaces with `H`.
fn ref_loop(parser: &mut Tabnas, name: &str, firsts: &[Tin], item: &str, follow: Tin) {
    let rule = name.to_string();
    let iteration = format!("{name}$alt0");
    let step = format!("{iteration}$step1");
    let firsts = firsts.to_vec();
    let iteration_for_loop = iteration.clone();
    parser.define_rule(name, move |spec| {
        spec.clear();
        spec.add_open(loop_entry(&rule));
        for first in firsts {
            spec.add_open(AltSpec {
                s: vec![vec![first]],
                b: 1,
                r: Some(iteration_for_loop.clone()),
                ..Default::default()
            });
        }
        loop_exits(spec, follow);
    });
    let item = item.to_string();
    let step_for_iteration = step.clone();
    parser.define_rule(iteration, move |spec| {
        spec.clear();
        spec.add_open(counted(
            AltSpec {
                p: Some(item),
                ..Default::default()
            },
            0,
        ));
        spec.add_close(counted(
            AltSpec {
                r: Some(step_for_iteration),
                ..Default::default()
            },
            1,
        ));
    });
    let rule = name.to_string();
    parser.define_rule(step, move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            r: Some(rule),
            ..Default::default()
        });
    });
}

/// A rule of one open alternative and an empty close.
fn simple_rule(parser: &mut Tabnas, name: &str, open: AltSpec, close: Option<AltSpec>) {
    parser.define_rule(name, move |spec| {
        spec.clear();
        spec.add_open(open);
        if let Some(close) = close {
            spec.add_close(close);
        }
    });
}

/// The compiler's `[ X ]` over a rule: take it when its FIRST token is
/// next (peek, then the pushed rule consumes it), skip it on a FOLLOW
/// token or on anything else.
fn optional_of(parser: &mut Tabnas, name: &str, first: Tin, item: &str, follows: &[Tin]) {
    let item = item.to_string();
    let follows = follows.to_vec();
    parser.define_rule(name, move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![first]],
            b: 1,
            p: Some(item),
            ..Default::default()
        });
        for follow in follows {
            spec.add_open(AltSpec {
                s: vec![vec![follow]],
                b: 1,
                ..Default::default()
            });
        }
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec::new());
    });
}

/// `[ "x" ]` as the compiler builds it: a group holding the token, and
/// the optional over the group, named after it. Returns the optional's
/// name, `_gen<index+1>_opt__gen<index>_group`.
fn optional_group(parser: &mut Tabnas, index: u32, item: Tin, follows: &[Tin]) -> String {
    let group = format!("_gen{index}_group");
    let opt = format!("_gen{}_opt_{group}", index + 1);
    simple_rule(
        parser,
        &group,
        AltSpec {
            s: vec![vec![item]],
            ..Default::default()
        },
        None,
    );
    optional_of(parser, &opt, item, &group, follows);
    opt
}

/// The compiler's `1*X` over a rule item: the helper pushes the item
/// and, on close, replaces with a step that pushes the star of the item.
fn plus_chain(parser: &mut Tabnas, name: &str, item: &str, star: &str) {
    let step = format!("{name}$step1");
    simple_rule(
        parser,
        name,
        AltSpec {
            p: Some(item.into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some(step.clone()),
            ..Default::default()
        }),
    );
    simple_rule(
        parser,
        &step,
        AltSpec {
            p: Some(star.into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
}

/// The text a loop test pins, plus the two RFC 5234 shapes and the
/// absence of any leaked synthetic: neither `_gen` / `r-gen` nor the
/// `$alt` / `-alt` of an iteration helper may reach the output.
fn assert_loop_abnf(out: &str, want: &str) {
    assert_eq!(
        out, want,
        "loop rendering mismatch:\n--- got ---\n{out}\n--- want ---\n{want}"
    );
    for leak in ["_gen", "r-gen", "$", "-alt", "step1"] {
        assert!(!out.contains(leak), "a synthetic {leak:?} leaked:\n{out}");
    }
    assert_rfc5234_shape(out);
}

/// `rep = *"a"`: a terminal item. This is THE defect shape: with the
/// entry counted as an alternative the loop rendered as
/// `r-gen1-star-term = [ r-gen1-star-term / A ]`, a production that
/// names itself and, recompiled, accepts less than the original.
#[test]
fn abnf_renders_a_terminal_loop_as_a_star() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "rep",
        AltSpec {
            p: Some("_gen1_star_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    terminal_loop(&mut parser, "_gen1_star_term", a, end);
    wrap_start(&mut parser, "rep");

    assert_loop_abnf(&abnf(&parser), "rep = *A\n\nA = %s\"a\"");
}

/// `rep = 1*"a"`: the `_plus` helper is `A` followed by the star of `A`.
/// With the star a loop the helper folds, and is written back as the
/// `1*A` it was compiled from — not the `A *A` it is element by element.
/// The two recognise the same language, but the abnf crate does not
/// compile them to the same recogniser: where the item is nullable
/// (`1*( [ "+" "e" ] )`) or its FIRST meets its FOLLOW (`1*item` with
/// `item = "]" "e" / [ "d" ]`), the recompiled `A *A` rejected `+e` and
/// `]e`, which the original accepts. This pinned `rep = A *A` until that
/// round trip failed. (With an old-shape star the helper stays a
/// production, as it always did: see
/// `abnf_keeps_an_old_shape_plus_production`.)
#[test]
fn abnf_renders_a_plus_over_a_loop_as_one_or_more() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "rep",
        AltSpec {
            p: Some("_gen1_plus_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    terminal_loop(&mut parser, "_gen1_star_term", a, end);
    simple_rule(
        &mut parser,
        "_gen1_plus_term",
        AltSpec {
            s: vec![vec![a]],
            p: Some("_gen1_star_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    wrap_start(&mut parser, "rep");

    assert_loop_abnf(&abnf(&parser), "rep = 1*A\n\nA = %s\"a\"");
}

/// `doc = *item` with `item = "x" "y"`: a rule item, so the loop goes
/// through the iteration helpers. `item` keeps its production; `H$alt0`
/// and `H$alt0$step1` do not get one, and their back edges render
/// nothing. The helpers are installed BEFORE the loop, as the compiler
/// installs them, so a production order that merely followed insertion
/// would have leaked them first.
#[test]
fn abnf_renders_a_ref_loop_as_a_star_of_the_rule() {
    let mut parser = Tabnas::new();
    let x = parser.token_with_source("#X", "x");
    let y = parser.token_with_source("#Y", "y");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "doc",
        AltSpec {
            p: Some("_gen1_star_item".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    simple_rule(
        &mut parser,
        "item",
        AltSpec {
            s: vec![vec![x], vec![y]],
            ..Default::default()
        },
        None,
    );
    ref_loop(&mut parser, "_gen1_star_item", &[x], "item", end);
    wrap_start(&mut parser, "doc");

    assert_loop_abnf(
        &abnf(&parser),
        "doc = *item\nitem = X Y\n\nX = %s\"x\"\nY = %s\"y\"",
    );
}

/// `list = "[" *( "," item ) "]"` with `item = "x"`: a loop inside a
/// sequence, over a group. The group folds into the iteration, the
/// loop's FOLLOW is the closing bracket (peeked by the exit, never
/// rendered), and the `"]"` comes from `list`'s own chain step.
#[test]
fn abnf_renders_a_group_loop_inside_a_sequence() {
    let mut parser = Tabnas::new();
    let open = parser.token_with_source("#T", "[");
    let close = parser.token_with_source("#T1", "]");
    let comma = parser.token_with_source("#T2", ",");
    let x = parser.token_with_source("#X", "x");
    simple_rule(
        &mut parser,
        "list",
        AltSpec {
            s: vec![vec![open]],
            p: Some("_gen2_star__gen1_group".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("list$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "list$step1",
        AltSpec {
            s: vec![vec![close]],
            ..Default::default()
        },
        None,
    );
    simple_rule(
        &mut parser,
        "_gen1_group",
        AltSpec {
            s: vec![vec![comma]],
            p: Some("item".into()),
            ..Default::default()
        },
        None,
    );
    ref_loop(
        &mut parser,
        "_gen2_star__gen1_group",
        &[comma],
        "_gen1_group",
        close,
    );
    simple_rule(
        &mut parser,
        "item",
        AltSpec {
            s: vec![vec![x]],
            ..Default::default()
        },
        None,
    );
    wrap_start(&mut parser, "list");

    assert_loop_abnf(
        &abnf(&parser),
        "list = T *( T2 item ) T1\nitem = X\n\nT  = \"[\"\nT2 = \",\"\nT1 = \"]\"\nX  = %s\"x\"",
    );
}

/// `s = *( "a" / "b" ) ";"`: a loop over a two-way group has one
/// continue per FIRST token, both replacing with the same iteration. The
/// iteration renders once, as the parenthesised alternation, and the
/// repetition wraps it exactly once: `*( A / B )`, not `*( ( A / B ) )`.
#[test]
fn abnf_renders_a_loop_over_alternatives_once() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let b = parser.token_with_source("#B", "b");
    let semi = parser.token_with_source("#T", ";");
    simple_rule(
        &mut parser,
        "s",
        AltSpec {
            p: Some("_gen2_star__gen1_group".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("s$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "s$step1",
        AltSpec {
            s: vec![vec![semi]],
            ..Default::default()
        },
        None,
    );
    parser.define_rule("_gen1_group", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![a]],
            ..Default::default()
        });
        spec.add_open(AltSpec {
            s: vec![vec![b]],
            ..Default::default()
        });
    });
    ref_loop(
        &mut parser,
        "_gen2_star__gen1_group",
        &[a, b],
        "_gen1_group",
        semi,
    );
    wrap_start(&mut parser, "s");

    assert_loop_abnf(
        &abnf(&parser),
        "s = *( A / B ) T\n\nA = %s\"a\"\nB = %s\"b\"\nT = \";\"",
    );
}

/// `outer = *( "<" *"i" ">" )`: a loop nested in a loop's iteration. The
/// inner loop is a loop of its own, rendered as `*I` inside the outer
/// iteration; the outer's exit peeks `#ZZ`, the inner's peeks `">"`.
#[test]
fn abnf_renders_a_loop_nested_in_a_loop() {
    let mut parser = Tabnas::new();
    let lt = parser.token_with_source("#T", "<");
    let gt = parser.token_with_source("#T1", ">");
    let i = parser.token_with_source("#I", "i");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "outer",
        AltSpec {
            p: Some("_gen3_star__gen2_group".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    terminal_loop(&mut parser, "_gen1_star_term", i, gt);
    simple_rule(
        &mut parser,
        "_gen2_group",
        AltSpec {
            s: vec![vec![lt]],
            p: Some("_gen1_star_term".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("_gen2_group$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "_gen2_group$step1",
        AltSpec {
            s: vec![vec![gt]],
            ..Default::default()
        },
        None,
    );
    ref_loop(
        &mut parser,
        "_gen3_star__gen2_group",
        &[lt],
        "_gen2_group",
        end,
    );
    wrap_start(&mut parser, "outer");

    assert_loop_abnf(
        &abnf(&parser),
        "outer = *( T *I T1 )\n\nT  = \"<\"\nI  = %s\"i\"\nT1 = \">\"",
    );
}

/// `rep = 2*"a"`: a counted repetition compiles to a `_rep` helper, the
/// item `n` times then the star of the item, in one alternative for a
/// terminal item. It is written back as `2*A` for the reason `1*A` is.
#[test]
fn abnf_renders_a_counted_repetition_over_a_loop_with_its_count() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "rep",
        AltSpec {
            p: Some("_gen1_rep_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    terminal_loop(&mut parser, "_gen1_star_term", a, end);
    simple_rule(
        &mut parser,
        "_gen1_rep_term",
        AltSpec {
            s: vec![vec![a], vec![a]],
            p: Some("_gen1_star_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    wrap_start(&mut parser, "rep");

    assert_loop_abnf(&abnf(&parser), "rep = 2*A\n\nA = %s\"a\"");
}

/// `n = 2*4"z"`: a bounded repetition compiles to a `_rep` helper too,
/// but one that ends in nested optionals rather than a loop. Its body is
/// not an item then a repetition, so it renders as it is; the count
/// rewrite must not reach for a `_rep` name alone.
#[test]
fn abnf_leaves_a_bounded_repetition_as_its_optionals() {
    let mut parser = Tabnas::new();
    let z = parser.token_with_source("#Z", "z");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "n",
        AltSpec {
            p: Some("_gen1_rep_term".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    let inner = optional_group(&mut parser, 1, z, &[end]);
    simple_rule(
        &mut parser,
        "_gen3_group",
        AltSpec {
            s: vec![vec![z]],
            p: Some(inner),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    optional_of(
        &mut parser,
        "_gen4_opt__gen3_group",
        z,
        "_gen3_group",
        &[end],
    );
    simple_rule(
        &mut parser,
        "_gen1_rep_term",
        AltSpec {
            s: vec![vec![z], vec![z]],
            p: Some("_gen4_opt__gen3_group".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    wrap_start(&mut parser, "n");

    assert_loop_abnf(&abnf(&parser), "n = Z Z [ Z [ Z ] ]\n\nZ = %s\"z\"");
}

/// `top = *[ "," ]`: a loop over an optional. The star's helper is named
/// after its item, `_gen3_star__gen2_opt__gen1_group`, and so are its
/// iteration helpers, and deciding the `[ … ]` wrap by a substring test
/// for `_opt` reached all three: the option was wrapped again, and the
/// step, whose only content is the back edge, became the empty option in
/// `*[ [ T ] [  ] ]`. RFC 5234 has no room for it: an option holds an
/// alternation, and an alternation at least one concatenation. Read from
/// the rule's own segment, only the optional's helper is an optional.
/// (The optional's exits peek the item's own token as well as the end:
/// inside a loop, the item is its own FOLLOW.)
#[test]
fn abnf_renders_a_loop_over_an_optional_without_an_empty_option() {
    let mut parser = Tabnas::new();
    let comma = parser.token_with_source("#T", ",");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "top",
        AltSpec {
            p: Some("_gen3_star__gen2_opt__gen1_group".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    let opt = optional_group(&mut parser, 1, comma, &[comma, end]);
    ref_loop(
        &mut parser,
        "_gen3_star__gen2_opt__gen1_group",
        &[comma],
        &opt,
        end,
    );
    wrap_start(&mut parser, "top");

    let out = abnf(&parser);
    assert!(!has_empty_option(&out), "an empty `[ ]` option in:\n{out}");
    assert_loop_abnf(&out, "top = *[ T ]\n\nT = \",\"");
}

/// `top = 1*[ "a" ]`: the `_plus` helper over an optional is named after
/// it too (`_gen3_plus__gen2_opt__gen1_group`), as is its chain step. The
/// same substring test rendered `[ [ A ] [ *[ [ A ] [  ] ] ] ]`; the
/// helper is a plus, its step a step, and the whole is the `1*[ A ]` it
/// came from.
#[test]
fn abnf_renders_a_plus_over_an_optional_loop_as_one_or_more() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "top",
        AltSpec {
            p: Some("_gen3_plus__gen2_opt__gen1_group".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    let opt = optional_group(&mut parser, 1, a, &[a, end]);
    ref_loop(
        &mut parser,
        "_gen3_star__gen2_opt__gen1_group",
        &[a],
        &opt,
        end,
    );
    plus_chain(
        &mut parser,
        "_gen3_plus__gen2_opt__gen1_group",
        &opt,
        "_gen3_star__gen2_opt__gen1_group",
    );
    wrap_start(&mut parser, "top");

    let out = abnf(&parser);
    assert!(!has_empty_option(&out), "an empty `[ ]` option in:\n{out}");
    assert_loop_abnf(&out, "top = 1*[ A ]\n\nA = %s\"a\"");
}

/// `top = 1*( "a" "b" )` and `top = 1*( "a" / "b" )`: a plus over a
/// group. The loop writes the sequence as `*( A B )` and the alternation,
/// already one parenthesised element, as `*( A / B )`; the plus is the
/// item as the loop wrote it, once, then the loop, in either spelling,
/// and comes back as `1*( A B )` and `1*( A / B )`.
#[test]
fn abnf_renders_a_plus_over_a_group_loop_as_one_or_more() {
    for (alternation, want) in [
        (false, "top = 1*( A B )\n\nA = %s\"a\"\nB = %s\"b\""),
        (true, "top = 1*( A / B )\n\nA = %s\"a\"\nB = %s\"b\""),
    ] {
        let mut parser = Tabnas::new();
        let a = parser.token_with_source("#A", "a");
        let b = parser.token_with_source("#B", "b");
        let end = token(&parser, "#ZZ");
        simple_rule(
            &mut parser,
            "top",
            AltSpec {
                p: Some("_gen2_plus__gen1_group".into()),
                ..Default::default()
            },
            Some(AltSpec::new()),
        );
        parser.define_rule("_gen1_group", move |spec| {
            spec.clear();
            if alternation {
                spec.add_open(AltSpec {
                    s: vec![vec![a]],
                    ..Default::default()
                });
                spec.add_open(AltSpec {
                    s: vec![vec![b]],
                    ..Default::default()
                });
            } else {
                spec.add_open(AltSpec {
                    s: vec![vec![a], vec![b]],
                    ..Default::default()
                });
            }
        });
        let firsts: &[Tin] = if alternation { &[a, b] } else { &[a] };
        ref_loop(
            &mut parser,
            "_gen2_star__gen1_group",
            firsts,
            "_gen1_group",
            end,
        );
        plus_chain(
            &mut parser,
            "_gen2_plus__gen1_group",
            "_gen1_group",
            "_gen2_star__gen1_group",
        );
        wrap_start(&mut parser, "top");

        assert_loop_abnf(&abnf(&parser), want);
    }
}

/// Design point 4: a grammar in the OLD shape renders exactly as before.
/// `1*"a"` compiled to a `_plus` helper pushing a push-chain star; the
/// star is a kept production, so the helper stays one too, and the output
/// is what the emitter gave before the loop shape existed: the productions
/// in installation order (the compiler installs the star before the plus),
/// the star's empty alternative as `[ … ]`.
#[test]
fn abnf_keeps_an_old_shape_plus_production() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "rep".into();
    let a = parser.token_with_source("#A", "a");
    simple_rule(
        &mut parser,
        "rep",
        AltSpec {
            p: Some("_gen1_plus_A".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    parser.define_rule("_gen1_star_A", move |spec| {
        spec.clear();
        // The push chain: each item pushes the rule again.
        spec.add_open(AltSpec {
            s: vec![vec![a]],
            p: Some("_gen1_star_A".into()),
            ..Default::default()
        });
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec::new());
    });
    simple_rule(
        &mut parser,
        "_gen1_plus_A",
        AltSpec {
            s: vec![vec![a]],
            p: Some("_gen1_star_A".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );

    let out = abnf(&parser);
    assert_eq!(
        out,
        "rep = r-gen1-plus-A\nr-gen1-star-A = [ A r-gen1-star-A ]\nr-gen1-plus-A = A r-gen1-star-A\n\nA = %s\"a\"",
        "old-shape plus mismatch:\n{out}"
    );
    assert_rfc5234_shape(&out);
}

/// A user rule with a non-consuming self-replace open alternative that is
/// NOT a loop entry: a guarded or counted state transition, such as
/// `{ c: [n.mode == 0], n: {mode: 1}, r: st }`. Its `s`, `b`, `p` and
/// `r` are the entry's, and it is no repetition: the entry's own guard
/// `n.rep == 0` and its counter set to 1 are part of the shape, and each
/// is tried without the other here too. Read as a loop, the whole rule
/// was rewritten as `st = *( A / B )`, accepting the empty input and any
/// number of items where the original takes one. It renders as the
/// emitter always rendered it, a reference to the rule among its
/// alternatives (`st = st / A / B` is what origin/main emits), and never
/// as `*…`.
#[test]
fn abnf_does_not_read_an_unguarded_self_replace_as_a_loop() {
    let guard = |name: &str, value: f64| Condition {
        path: vec!["n".into(), name.into()],
        op: CompareOp::Eq,
        value: Value::Number(value),
    };
    let transitions = [
        // Another counter's transition.
        AltSpec {
            c: vec![guard("mode", 0.0)],
            n: HashMap::from([("mode".to_string(), 1)]),
            r: Some("st".into()),
            ..Default::default()
        },
        // The bare self-replace.
        AltSpec {
            r: Some("st".into()),
            ..Default::default()
        },
        // The guard without the counter.
        AltSpec {
            c: vec![guard("rep", 0.0)],
            r: Some("st".into()),
            ..Default::default()
        },
        // The counter without the guard.
        counted(
            AltSpec {
                r: Some("st".into()),
                ..Default::default()
            },
            1,
        ),
    ];
    for transition in transitions {
        let mut parser = Tabnas::new();
        parser.options.rule.start = "st".into();
        let a = parser.token_with_source("#A", "a");
        let b = parser.token_with_source("#B", "b");
        parser.define_rule("st", move |spec| {
            spec.clear();
            spec.add_open(transition);
            spec.add_open(AltSpec {
                s: vec![vec![a]],
                ..Default::default()
            });
            spec.add_open(AltSpec {
                s: vec![vec![b]],
                ..Default::default()
            });
            spec.add_close(AltSpec::new());
        });

        let out = abnf(&parser);
        assert_eq!(
            out, "st = st / A / B\n\nA = %s\"a\"\nB = %s\"b\"",
            "a self-replace that is not the loop entry:\n{out}"
        );
        assert!(!out.contains('*'), "read as a repetition:\n{out}");
        assert_rfc5234_shape(&out);
    }
}

/// `one = A [ one ]`, hand-built as a guarded close continuation: the
/// open consumes `A`, and the closes are `{ s: A, b: 1, r: one }`, which
/// peeks the next `A` and re-enters the rule, and `{ }`. The continuation
/// replaces with the rule being rendered and is not its loop entry (a
/// close, and unguarded), so it keeps its content. Calling it empty for
/// the self-replace alone skipped it and emitted `one = A`: exactly one
/// where the rule takes one or more. origin/main emits the pinned text.
#[test]
fn abnf_keeps_a_guarded_self_replacing_close_continuation() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "one".into();
    let a = parser.token_with_source("#A", "a");
    parser.define_rule("one", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![a]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![a]],
            b: 1,
            r: Some("one".into()),
            ..Default::default()
        });
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    assert_eq!(
        out, "one = A [ one ]\n\nA = %s\"a\"",
        "one-or-more continuation mismatch:\n{out}"
    );
    assert_rfc5234_shape(&out);
}

/// `top = *( "b" *"a" "c" )` with the outer star in the loop shape and
/// the inner star in the OLD push-chain shape, a mix a hand-built grammar
/// can carry. A loop's helpers are the rules named after it, `H$alt0` and
/// `H$alt0$step1`, and nothing else it reaches: the foldable group is
/// inlined on its own account, and the old star, a kept production, stays
/// a bareword reference inside the repetition, with its production and
/// its epsilon branch. Finding the helpers by reachability added the old
/// star to them, suppressed its production and inlined it without its
/// epsilon branch and back edge: `*( B A C )`, exactly one `A` where the
/// original takes any number.
#[test]
fn abnf_keeps_an_old_shape_star_inside_a_loop_group() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let b = parser.token_with_source("#B", "b");
    let c = parser.token_with_source("#C", "c");
    let end = token(&parser, "#ZZ");
    simple_rule(
        &mut parser,
        "top",
        AltSpec {
            p: Some("_gen3_star__gen2_group".into()),
            ..Default::default()
        },
        Some(AltSpec::new()),
    );
    parser.define_rule("_gen1_star_A", move |spec| {
        spec.clear();
        // The push chain: each item pushes the rule again.
        spec.add_open(AltSpec {
            s: vec![vec![a]],
            p: Some("_gen1_star_A".into()),
            ..Default::default()
        });
        spec.add_open(AltSpec::new());
        spec.add_close(AltSpec::new());
    });
    simple_rule(
        &mut parser,
        "_gen2_group",
        AltSpec {
            s: vec![vec![b]],
            p: Some("_gen1_star_A".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("_gen2_group$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "_gen2_group$step1",
        AltSpec {
            s: vec![vec![c]],
            ..Default::default()
        },
        None,
    );
    ref_loop(
        &mut parser,
        "_gen3_star__gen2_group",
        &[b],
        "_gen2_group",
        end,
    );
    wrap_start(&mut parser, "top");

    let out = abnf(&parser);
    assert_eq!(
        out,
        "top = *( B r-gen1-star-A C )\nr-gen1-star-A = [ A r-gen1-star-A ]\n\nB = %s\"b\"\nC = %s\"c\"\nA = %s\"a\"",
        "old-shape star inside a loop mismatch:\n{out}"
    );
    for leak in ["_gen", "$", "-alt", "step1"] {
        assert!(!out.contains(leak), "a synthetic {leak:?} leaked:\n{out}");
    }
    assert_rfc5234_shape(&out);
}

/// `( *"a" "b" / "c" )` as the compiler builds it under a repetition:
/// a group whose first alternative starts with a rule gets a `$alt` /
/// `$step` chain of its own — `_gen2_group$alt0` pushes the inner star
/// and closes into `_gen2_group$alt0$step1`, which takes the `"b"`;
/// `_gen2_group$alt1` takes the `"c"`; the group peeks each FIRST token
/// and pushes the chain that starts with it. The inner star is a terminal
/// loop whose FOLLOW is the `"b"`; the outer star, `_gen3_star__gen2_group`,
/// is a ref loop over the group with one continue per FIRST token and the
/// `"d"` after it as its FOLLOW. Installed in the compiler's order, from
/// the specs it emits for `top = *( *%s"a" %s"b" / %s"c" ) %s"d"`.
fn star_of_a_group_with_a_star_alternative(parser: &mut Tabnas, a: Tin, b: Tin, c: Tin, d: Tin) {
    terminal_loop(parser, "_gen1_star_term", a, b);
    simple_rule(
        parser,
        "_gen2_group$alt0",
        AltSpec {
            p: Some("_gen1_star_term".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("_gen2_group$alt0$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        parser,
        "_gen2_group$alt0$step1",
        AltSpec {
            s: vec![vec![b]],
            ..Default::default()
        },
        None,
    );
    simple_rule(
        parser,
        "_gen2_group$alt1",
        AltSpec {
            s: vec![vec![c]],
            ..Default::default()
        },
        None,
    );
    parser.define_rule("_gen2_group", move |spec| {
        spec.clear();
        for (first, chain) in [
            (a, "_gen2_group$alt0"),
            (b, "_gen2_group$alt0"),
            (c, "_gen2_group$alt1"),
        ] {
            spec.add_open(AltSpec {
                s: vec![vec![first]],
                b: 1,
                p: Some(chain.into()),
                ..Default::default()
            });
        }
        spec.add_close(AltSpec::new());
    });
    ref_loop(
        parser,
        "_gen3_star__gen2_group",
        &[a, b, c],
        "_gen2_group",
        d,
    );
}

/// `top = *( *"a" "b" / "c" ) "d"`: a loop over a group that has a
/// `$alt` / `$step` chain of its own, because one of its alternatives
/// starts with a rule. That chain is the loop's to inline, like the
/// group and the loop's own `H$alt0` and `H$alt0$step1`: the helpers of
/// a loop are everything its iteration reaches short of a kept
/// production. Finding them by name (`H$…`) missed the group's chain,
/// which `is_foldable` refuses for its `$alt`, and it surfaced as three
/// kept productions, `top = *( r-gen2-group-alt0 / r-gen2-group-alt1 ) D`
/// with `r-gen2-group-alt0 = *A r-gen2-group-alt0-step1`. The inner loop
/// renders as `*A` inside the alternative.
#[test]
fn abnf_renders_a_loop_over_a_group_with_a_star_alternative() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let b = parser.token_with_source("#B", "b");
    let c = parser.token_with_source("#C", "c");
    let d = parser.token_with_source("#D", "d");
    simple_rule(
        &mut parser,
        "top",
        AltSpec {
            p: Some("_gen3_star__gen2_group".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("top$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "top$step1",
        AltSpec {
            s: vec![vec![d]],
            ..Default::default()
        },
        None,
    );
    star_of_a_group_with_a_star_alternative(&mut parser, a, b, c, d);
    wrap_start(&mut parser, "top");

    assert_loop_abnf(
        &abnf(&parser),
        "top = *( *A B / C ) D\n\nA = %s\"a\"\nB = %s\"b\"\nC = %s\"c\"\nD = %s\"d\"",
    );
}

/// `top = 1*( *"a" "b" / "c" ) "d"`: the `_plus` helper over the same
/// group pushes it directly and, on close, steps into the loop. The
/// group's chain folds, so the helper folds too, and is written back as
/// the `1*( … )` it was compiled from. With the chain refused as kept
/// productions the helper was refused as well and came out as
/// `top = r-gen3-plus--gen2-group D` over a body of `X *X`, the spelling
/// that does not round-trip on a nullable item — and this item is
/// nullable: `*A B` can start with the `B`.
#[test]
fn abnf_renders_a_plus_over_a_group_with_a_star_alternative() {
    let mut parser = Tabnas::new();
    let a = parser.token_with_source("#A", "a");
    let b = parser.token_with_source("#B", "b");
    let c = parser.token_with_source("#C", "c");
    let d = parser.token_with_source("#D", "d");
    simple_rule(
        &mut parser,
        "top",
        AltSpec {
            p: Some("_gen3_plus__gen2_group".into()),
            ..Default::default()
        },
        Some(AltSpec {
            r: Some("top$step1".into()),
            ..Default::default()
        }),
    );
    simple_rule(
        &mut parser,
        "top$step1",
        AltSpec {
            s: vec![vec![d]],
            ..Default::default()
        },
        None,
    );
    star_of_a_group_with_a_star_alternative(&mut parser, a, b, c, d);
    plus_chain(
        &mut parser,
        "_gen3_plus__gen2_group",
        "_gen2_group",
        "_gen3_star__gen2_group",
    );
    wrap_start(&mut parser, "top");

    assert_loop_abnf(
        &abnf(&parser),
        "top = 1*( *A B / C ) D\n\nA = %s\"a\"\nB = %s\"b\"\nC = %s\"c\"\nD = %s\"d\"",
    );
}

/// `odd = A [ odd ]`, hand-built with the close continuation carrying the
/// loop entry's whole shape: `{ c: [n.rep == 0], n: {rep: 1}, s: A,
/// b: 1, r: odd }`, then `{ }`. A loop is decided by its OPEN
/// alternatives, and `odd` has no entry among them, so it is no loop, and
/// a rule that is not a loop has no entry to skip: every alternative is
/// content. Skipping the entry's shape wherever it appeared emitted
/// `odd = A`, exactly one where the rule takes one or more. origin/main
/// emits the pinned text.
#[test]
fn abnf_keeps_an_entry_shaped_close_continuation_of_a_rule_that_is_no_loop() {
    let mut parser = Tabnas::new();
    parser.options.rule.start = "odd".into();
    let a = parser.token_with_source("#A", "a");
    parser.define_rule("odd", move |spec| {
        spec.clear();
        spec.add_open(AltSpec {
            s: vec![vec![a]],
            ..Default::default()
        });
        spec.add_close(AltSpec {
            s: vec![vec![a]],
            b: 1,
            ..loop_entry("odd")
        });
        spec.add_close(AltSpec::new());
    });

    let out = abnf(&parser);
    assert_eq!(
        out, "odd = A [ odd ]\n\nA = %s\"a\"",
        "entry-shaped continuation of a non-loop mismatch:\n{out}"
    );
    assert!(!out.contains('*'), "read as a repetition:\n{out}");
    assert_rfc5234_shape(&out);
}
