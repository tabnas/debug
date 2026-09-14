/* Copyright (c) 2026 Richard Rodger and other contributors, MIT License */

//! Runner for the shared `test/spec/*.tsv` conformance fixtures.
//!
//! A row names a GRAMMAR from the shared registry, and the second
//! column's HEADER names what is reported about it — so the reporter is
//! per file, and every file in the spec directory is discovered and run.
//!
//! TypeScript and Go read these files through `@tabnas/support` and its
//! Go half. There is no Rust half, so this loader is written out and must
//! keep to the same codec — see `test/AGENTS.md`.

use std::path::PathBuf;

use tabnas::Tabnas;
use tabnas_debug::{abnf, describe, model};

use super::fixture;

/// A `describe()` section banner, e.g. `========= TOKENS ========`.
/// The canonical runners use the regex `^=+ .* =+$`; this is the same
/// shape without pulling in a regex dependency for one pattern.
fn is_section_header(line: &str) -> bool {
    let bytes = line.as_bytes();
    let lead = bytes.iter().take_while(|byte| **byte == b'=').count();
    let trail = bytes.iter().rev().take_while(|byte| **byte == b'=').count();
    if 0 == lead || 0 == trail || lead + trail + 2 > bytes.len() {
        return false;
    }
    b' ' == bytes[lead] && b' ' == bytes[bytes.len() - trail - 1]
}

/// What the second column holds, keyed by its header name.
fn report(kind: &str, parser: &Tabnas) -> Option<serde_json::Value> {
    match kind {
        "abnf" => Some(serde_json::Value::String(abnf(parser))),

        "sections" => Some(serde_json::Value::Array(
            describe(parser)
                .lines()
                .filter(|line| is_section_header(line))
                .map(|line| serde_json::Value::String(line.to_string()))
                .collect(),
        )),

        // The grammar-structure portion of model(), as it serialises.
        // Pins the cross-runtime claim that every runtime's field names
        // agree. Instance-level sections are excluded: `lexer` is
        // summarised outside TypeScript, the non-TS registries need not
        // load the debug plugin, and `tag` is held out pending a Go
        // engine-version boundary — see docs/reference.md.
        //
        // The runtimes order rules/graph differently by design (TS
        // insertion order, Go by name), so the shared fixture compares
        // them sorted by name.
        "model" => {
            let built = model(parser);
            let mut rules = built.rules;
            rules.sort_by(|left, right| left.name.cmp(&right.name));
            let mut graph = built.graph;
            graph.sort_by(|left, right| left.name.cmp(&right.name));
            Some(serde_json::json!({ "rules": rules, "graph": graph }))
        }

        _ => None,
    }
}

/// The repo-root `test/spec` directory, resolved from this crate's
/// manifest so the runner does not depend on the working directory.
fn spec_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("rs/ has a parent directory")
        .join("test")
        .join("spec")
}

/// One fixture file: its name, header row, and data rows.
struct Spec {
    name: String,
    header: Vec<String>,
    rows: Vec<(usize, String, String)>,
}

/// Load every `.tsv` in the spec directory, in name order.
fn load_spec_dir() -> Vec<Spec> {
    let dir = spec_dir();
    let mut paths: Vec<PathBuf> = std::fs::read_dir(&dir)
        .unwrap_or_else(|error| panic!("failed to read {}: {error}", dir.display()))
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| "tsv" == ext))
        .collect();
    paths.sort();

    paths
        .into_iter()
        .map(|path| {
            let name = path
                .file_name()
                .expect("a spec file has a name")
                .to_string_lossy()
                .into_owned();
            let text = std::fs::read_to_string(&path)
                .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));

            let mut header: Vec<String> = Vec::new();
            let mut rows = Vec::new();
            for (index, raw) in text.lines().enumerate() {
                let line = raw.strip_suffix('\r').unwrap_or(raw);
                // Blank lines are skipped, and so are comment lines — a
                // line starting with `#` that contains no tab. (A data row
                // always has at least one tab.)
                if line.trim().is_empty() || (line.starts_with('#') && !line.contains('\t')) {
                    continue;
                }
                let columns: Vec<&str> = line.split('\t').collect();
                if header.is_empty() {
                    header = columns.into_iter().map(str::to_string).collect();
                    continue;
                }
                if 2 > columns.len() {
                    panic!("{name}:{}: expected at least two columns", index + 1);
                }
                rows.push((index + 1, columns[0].to_string(), columns[1].to_string()));
            }
            Spec { name, header, rows }
        })
        .collect()
}

/// Run every fixture in the spec directory.
pub fn run_spec_dir() {
    let specs = load_spec_dir();
    // An empty fixture, and a spec directory with no fixtures in it, both
    // fail — a runner that silently reports nothing proves nothing.
    assert!(!specs.is_empty(), "no .tsv fixtures in the spec directory");

    let mut failures: Vec<String> = Vec::new();

    for spec in specs {
        assert!(
            2 <= spec.header.len(),
            "{}: expected at least two columns in the header",
            spec.name
        );
        let kind = spec.header[1].clone();
        assert!(!spec.rows.is_empty(), "{}: fixture has no rows", spec.name);

        for (line, grammar, expected_raw) in spec.rows {
            let at = format!("{}:{line}", spec.name);
            let Some(parser) = fixture::build(&grammar) else {
                panic!("{at}: unknown grammar fixture {grammar:?}");
            };
            let Some(got) = report(&kind, &parser) else {
                panic!("{at}: unknown second column {kind:?}");
            };
            let expected: serde_json::Value = serde_json::from_str(&expected_raw)
                .unwrap_or_else(|error| panic!("{at}: expected is not valid JSON: {error}"));

            // Compare as decoded JSON so both sides are the same shape of
            // generic value and field ORDER is irrelevant.
            let got = serde_json::to_value(&got).expect("the report serialises");
            if got != expected {
                failures.push(format!(
                    "{at} grammar {grammar:?}\n  got:      {got}\n  expected: {expected}"
                ));
            }
        }
    }

    assert!(
        failures.is_empty(),
        "{} spec row(s) failed:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[cfg(test)]
mod tests {
    use super::is_section_header;

    #[test]
    fn section_header_shape() {
        assert!(is_section_header("========= ABNF ========="));
        assert!(is_section_header("= a ="));
        assert!(!is_section_header("====="));
        assert!(!is_section_header("  tag: -"));
        assert!(!is_section_header(""));
    }
}
