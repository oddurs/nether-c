//! The proof for *the recursive descent parser*: every sample in the
//! specification parses, and malformed input produces one error rather than a
//! cascade.
//!
//! The samples are found rather than listed. Every fenced `c` block under
//! `spec/` is extracted and looked up in the table below, so a sample added to
//! the specification fails this file until somebody says what it is — and what
//! each one *is* has to be said, because the specification writes three
//! different things in a `c` fence: whole units and fragments of a body. The
//! third — §09's signature listings, which §04 has no production for — stopped
//! being labelled as the language, which is what made that list two long
//! instead of three.

use std::collections::BTreeMap;

use nether_syntax::ast::{Item, Unit};
use nether_syntax::{Fault, FaultKind, parse};

/// What a sample in the specification is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Shape {
    /// A whole compilation unit. §4.1.
    Unit,
    /// Statements, as they would be written inside a body.
    Statements,
    /// A sample §04 cannot parse, and the item that is about to fix it. When
    /// it lands, this test fails until the entry is changed — which is the
    /// point of it being here.
    Blocked(&'static str),
}

/// Every sample in `spec/`, and what it is.
const SAMPLES: &[(&str, Shape)] = &[
    ("spec/00-overview.md:125", Shape::Unit),
    ("spec/01-strata.md:52", Shape::Unit),
    ("spec/01-strata.md:94", Shape::Unit),
    ("spec/01-strata.md:111", Shape::Unit),
    ("spec/02-calculus.md:159", Shape::Unit),
    ("spec/02-calculus.md:202", Shape::Statements),
    ("spec/03-lexical.md:26", Shape::Unit),
    ("spec/04-grammar.md:155", Shape::Statements),
    ("spec/05-types.md:52", Shape::Statements),
    ("spec/05-types.md:71", Shape::Unit),
    ("spec/05-types.md:113", Shape::Blocked("0116: a struct cannot be constructed")),
    ("spec/06-evaluation.md:33", Shape::Unit),
    ("spec/09-prelude.md:78", Shape::Statements),
    ("spec/90-rationale.md:368", Shape::Unit),
];

/// Every fenced `c` block under `spec/`, keyed `file:line` the way the
/// transcript manifest keys them.
fn samples() -> BTreeMap<String, String> {
    let root = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");
    let mut out = BTreeMap::new();
    let mut files: Vec<_> = std::fs::read_dir(format!("{root}/spec"))
        .expect("spec/ is there")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "md"))
        .collect();
    files.sort();

    for path in files {
        let text = std::fs::read_to_string(&path).expect("readable");
        let name = format!("spec/{}", path.file_name().unwrap().to_string_lossy());
        let mut body: Vec<&str> = Vec::new();
        let mut opened = 0usize;
        for (i, line) in text.lines().enumerate() {
            if line.starts_with("```") {
                if opened > 0 {
                    out.insert(format!("{name}:{opened}"), body.join("\n"));
                    opened = 0;
                } else if line.trim() == "```c" {
                    opened = i + 1;
                    body.clear();
                }
                continue;
            }
            if opened > 0 {
                body.push(line);
            }
        }
    }
    out
}

fn parses(src: &str) -> Result<Unit, Vec<Fault>> {
    parse(src.as_bytes())
}

// ── every sample ────────────────────────────────────────────────────────────

#[test]
fn every_sample_in_the_specification_is_accounted_for() {
    let found: std::collections::BTreeSet<String> = samples().into_keys().collect();
    let listed: std::collections::BTreeSet<String> =
        SAMPLES.iter().map(|(r, _)| (*r).to_string()).collect();
    assert_eq!(found, listed, "a sample in spec/ that this file does not know about");
}

#[test]
fn every_sample_in_the_specification_parses() {
    for (reference, shape) in SAMPLES {
        let body = &samples()[*reference];
        let result = match shape {
            Shape::Unit | Shape::Blocked(_) => parses(body),
            // A fragment of a body is a body with something round it.
            Shape::Statements => parses(&format!("U0 sample()\n{{\n{body}\n}}\n")),
        };
        match shape {
            Shape::Blocked(item) => {
                assert!(result.is_err(), "{reference} parses now — reclassify it ({item})");
            }
            _ => {
                assert!(result.is_ok(), "{reference} does not parse: {:?}", result.unwrap_err());
            }
        }
    }
}

#[test]
fn the_one_sample_that_does_not_parse_says_so_once() {
    // §5.4's `Header h;` is a declaration with no initialiser, which §4.2 does
    // not have. One complaint, at the `;` where the `=` should be.
    let (reference, _) = SAMPLES
        .iter()
        .find(|(_, s)| matches!(s, Shape::Blocked(_)))
        .expect("the blocked sample went away");
    let faults = parses(&samples()[*reference]).expect_err("this does not parse");
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].kind, FaultKind::Expected("`=` after a binding"));
}

// ── one mistake, one message ────────────────────────────────────────────────

fn faults(src: &str) -> Vec<FaultKind> {
    parses(src).expect_err("this is malformed").into_iter().map(|f| f.kind).collect()
}

#[test]
fn one_missing_semicolon_is_one_complaint() {
    let src = "I64 a = 1\nI64 b = 2;\nI64 c = 3;\n";
    assert_eq!(faults(src), vec![FaultKind::Expected("`;` after a binding")]);
}

#[test]
fn the_items_after_a_mistake_still_parse() {
    let src = "I64 a = ;\nI64 b = 2;\ndemand b;\n";
    assert_eq!(faults(src).len(), 1);

    // And the parse that produced that one complaint still found the rest.
    let src_ok = "I64 b = 2;\ndemand b;\n";
    let unit = parses(src_ok).expect("this is fine");
    assert!(matches!(unit.items[..], [Item::Let(_), Item::Demand { .. }]));
}

#[test]
fn three_mistakes_are_three_complaints_and_not_thirty() {
    let src = "I64 a = 1\nI64 b = 2\nI64 c = 3\n";
    assert_eq!(faults(src).len(), 3);
}

#[test]
fn a_mistake_inside_a_body_does_not_escape_it() {
    let src = "U0 f()\n{\n  I64 a = ;\n  I64 b = 2;\n}\n\ndemand 1;\n";
    assert_eq!(faults(src).len(), 1);
}

#[test]
fn a_mistake_in_one_body_does_not_reach_the_next() {
    let src = "U0 f() { I64 a = ; }\nU0 g() { I64 b = ; }\n";
    assert_eq!(faults(src).len(), 2);
}

#[test]
fn a_file_that_is_all_wrong_still_stops() {
    // Recovery that does not move is a loop, and a parser that hangs on a
    // malformed file is worse than one that says the wrong thing.
    for src in ["}}}}", "((((", ";;;;", "@", "struct", "demand", "]["] {
        assert!(parses(src).is_err(), "{src} parsed");
    }
}

// ── §4.6, read back ─────────────────────────────────────────────────────────

/// The rows of §4.6's table: `| level | operators |`.
fn precedence_in_spec() -> BTreeMap<String, u8> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/04-grammar.md");
    let text = std::fs::read_to_string(path).expect("cannot read spec/04-grammar.md");
    let mut out = BTreeMap::new();
    for line in text.lines() {
        // A `\|` inside a cell is a pipe, not a cell boundary, and §4.6 has
        // three of them.
        let line = line.replace("\\|", "\u{0}");
        let mut cells = line.split('|').map(str::trim);
        let (Some(""), Some(level), Some(ops)) = (cells.next(), cells.next(), cells.next()) else {
            continue;
        };
        let Ok(level) = level.parse::<u8>() else { continue };
        for op in ops.split_whitespace() {
            if let Some(op) = op.strip_prefix('`').and_then(|o| o.strip_suffix('`')) {
                out.insert(op.replace('\u{0}', "|"), level);
            }
        }
    }
    out
}

#[test]
fn the_binary_operators_sit_where_section_four_point_six_puts_them() {
    let in_spec = precedence_in_spec();
    assert!(!in_spec.is_empty(), "§4.6's table moved");
    for (punct, level, _) in nether_syntax::levels() {
        assert_eq!(
            in_spec.get(punct.spelling()),
            Some(&level),
            "`{punct}` is at level {level} here and somewhere else in §4.6"
        );
    }
}

/// An expression as a tree, without the spans — which are what makes two
/// spellings of one shape look different.
fn render(e: &nether_syntax::ast::Expr) -> String {
    use nether_syntax::ast::ExprKind as K;
    let each =
        |xs: &[nether_syntax::ast::Expr]| xs.iter().map(render).collect::<Vec<_>>().join(" ");
    match &e.kind {
        K::Int(n) => n.to_string(),
        K::Name(n) => n.text.clone(),
        K::Binary { op, lhs, rhs } => format!("({op:?} {} {})", render(lhs), render(rhs)),
        K::Assign { op, place, value } => {
            format!("(assign {op:?} {} {})", render(place), render(value))
        }
        K::Unary { op, operand } => format!("({op:?} {})", render(operand)),
        K::Rite { rite, operand } => format!("({rite:?} {})", render(operand)),
        K::Call { callee, args } => format!("(call {} {})", render(callee), each(args)),
        other => format!("{other:?}"),
    }
}

fn shape(src: &str) -> String {
    let unit = parses(&format!("demand {src};")).expect("this parses");
    let Item::Demand { value, .. } = &unit.items[0] else { panic!("not a demand") };
    render(value)
}

#[test]
fn precedence_is_what_the_table_says() {
    // `1 + 2 * 3` groups as `1 + (2 * 3)`, and `1 * 2 + 3` as `(1 * 2) + 3`.
    assert_ne!(shape("1 + 2 * 3"), shape("(1 + 2) * 3"));
    assert_eq!(shape("1 + 2 * 3"), shape("1 + (2 * 3)"));
    assert_eq!(shape("1 * 2 + 3"), shape("(1 * 2) + 3"));
    assert_eq!(shape("1 || 2 && 3"), shape("1 || (2 && 3)"));
    assert_eq!(shape("1 == 2 < 3"), shape("1 == (2 < 3)"));

    // §4.6: `seal read(p)` seals the result of the call, not the function.
    assert_eq!(shape("seal read(p)"), shape("seal (read(p))"));
}

#[test]
fn assignment_is_right_associative_and_the_binaries_are_not() {
    assert_eq!(shape("a = b = c"), shape("a = (b = c)"));
    assert_eq!(shape("1 - 2 - 3"), shape("(1 - 2) - 3"));
    assert_ne!(shape("1 - 2 - 3"), shape("1 - (2 - 3)"));
}
