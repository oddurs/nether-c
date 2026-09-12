//! Every `c` sample in the specification, and what each one is.
//!
//! Named the way the specification names things: by the heading the sample is
//! under, and its ordinal within that heading. A line number moves whenever a
//! paragraph is inserted above it, and a paragraph is what this project mostly
//! writes — so a table keyed by line number makes editing prose cost an edit
//! in another crate, and says `no entry found for key` when it does. 0137.
//!
//! The table below is the one place that says what a sample is. Four proofs
//! read it: the parser's, depth inference's, lowering's, and the three
//! programs lifted verbatim into `tests/programs/`.

// Four test binaries include this module and each uses part of it.
#![allow(dead_code)]

use std::collections::BTreeMap;

/// What a sample in the specification is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Shape {
    /// A whole compilation unit. §4.1.
    Unit,
    /// Statements, as they would be written inside a body.
    Statements,
    /// A whole unit that §04 parses and the depth checker rejects, on purpose:
    /// it is the sample an error message is printed from. The parser's proof
    /// takes it; the proofs about compiling do not.
    Illegal(&'static str),
    /// A sample §04 cannot parse, and the item that is about to fix it. When
    /// it lands, the parser's proof fails until the entry is changed — which
    /// is the point of it being here.
    Blocked(&'static str),
    /// A sample that parses and that the checker ought to reject and does not
    /// yet, and the item that will make it. One stage later than `Blocked`
    /// and the same idea: when the rule lands, the entry has to change.
    Unchecked(&'static str),
}

/// Every sample in `spec/`, and what it is.
///
/// The specification writes two different things in a `c` fence: whole units,
/// and fragments of a body. It used to write a third — §09's signature
/// listings, which §04 has no production for — until they stopped claiming to
/// be the language.
pub const SAMPLES: &[(&str, Shape)] = &[
    ("00-overview.md § 0.7 A first program #1", Shape::Unit),
    ("01-strata.md § 1.3 Descent #1", Shape::Unit),
    ("01-strata.md § 1.5 Seal #1", Shape::Unit),
    ("01-strata.md § 1.6 Shade, and the Orpheus rule #1", Shape::Illegal("the Orpheus error")),
    ("02-calculus.md § 2.3 What each rule is doing #1", Shape::Unit),
    ("02-calculus.md § 2.4 Metatheory #1", Shape::Statements),
    ("03-lexical.md § 3.2 Comments #1", Shape::Unit),
    ("04-grammar.md § 4.3 Types #1", Shape::Unit),
    ("04-grammar.md § 4.7 The bare-expression statement #1", Shape::Statements),
    ("05-types.md § 5.1.1 Answers and refusals #1", Shape::Statements),
    ("05-types.md § 5.2 Aggregates #1", Shape::Unit),
    ("05-types.md § 5.4 Mutation #1", Shape::Illegal("h was named by `seal h`")),
    ("06-evaluation.md § 6.2 Demand #1", Shape::Unit),
    ("09-prelude.md § 9.2 Depth 0 #1", Shape::Statements),
    ("90-rationale.md § One number on an arrow #1", Shape::Unit),
];

/// A fenced `c` block in the specification.
pub struct Sample {
    /// `01-strata.md § 1.5 Seal #1`.
    pub key: String,
    /// What is between the fences, with no trailing newline.
    pub body: String,
}

/// Where the specification is.
pub fn spec_dir() -> std::path::PathBuf {
    std::path::PathBuf::from(concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec"))
}

/// Every fenced `c` block under `spec/`, in the order they are written.
pub fn samples() -> Vec<Sample> {
    samples_in(&spec_dir())
}

/// The same, over any copy of the specification. The copy is what proves the
/// naming is stable: see `grammar.rs`.
pub fn samples_in(dir: &std::path::Path) -> Vec<Sample> {
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("a directory of specification files")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();

    let mut out = Vec::new();
    for path in files {
        let text = std::fs::read_to_string(&path).expect("readable");
        let file = path.file_name().unwrap().to_string_lossy().into_owned();
        let mut heading = String::from("(no heading)");
        let mut seen: BTreeMap<String, usize> = BTreeMap::new();
        let mut body: Vec<&str> = Vec::new();
        let mut open = false;
        for line in text.lines() {
            if line.starts_with("```") {
                if open {
                    let under = format!("{file} § {heading}");
                    let nth = seen.entry(under.clone()).or_default();
                    *nth += 1;
                    out.push(Sample { key: format!("{under} #{nth}"), body: body.join("\n") });
                    open = false;
                } else if line.trim() == "```c" {
                    open = true;
                    body.clear();
                }
                continue;
            }
            if open {
                body.push(line);
            } else if let Some(text) = line.strip_prefix('#') {
                heading = text.trim_start_matches('#').trim().to_string();
            }
        }
    }
    out
}

/// The sample by that name, or a panic that says what there is instead.
pub fn body(key: &str) -> String {
    let found = samples();
    if let Some(sample) = found.iter().find(|s| s.key == key) {
        return sample.body.clone();
    }
    let have: Vec<&str> = found.iter().map(|s| s.key.as_str()).collect();
    panic!("no sample called `{key}`.\nthe specification has:\n  {}", have.join("\n  "))
}

/// Every sample of that shape, body first, in specification order.
pub fn of_shape(want: Shape) -> Vec<(String, String)> {
    SAMPLES
        .iter()
        .filter(|(_, shape)| *shape == want)
        .map(|(key, _)| ((*key).to_string(), body(key)))
        .collect()
}
