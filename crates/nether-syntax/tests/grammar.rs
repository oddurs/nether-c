//! The proof for *the recursive descent parser*: every sample in the
//! specification parses, and malformed input produces one error rather than a
//! cascade.
//!
//! The samples are found rather than listed. Every fenced `c` block under
//! `spec/` is extracted and looked up in `tests/spec/mod.rs`, so a sample added
//! to the specification fails this file until somebody says what it is — and
//! what each one *is* has to be said, because the specification writes two
//! different things in a `c` fence: whole units and fragments of a body.

use std::collections::BTreeMap;

use nether_syntax::ast::{Item, Unit};
use nether_syntax::{Fault, FaultKind, parse};

mod spec;
use spec::{SAMPLES, Shape};

fn parses(src: &str) -> Result<Unit, Vec<Fault>> {
    parse(src.as_bytes())
}

// ── every sample ────────────────────────────────────────────────────────────

#[test]
fn every_sample_in_the_specification_is_accounted_for() {
    let found: std::collections::BTreeSet<String> =
        spec::samples().into_iter().map(|s| s.key).collect();
    let listed: std::collections::BTreeSet<String> =
        SAMPLES.iter().map(|(r, _)| (*r).to_string()).collect();
    assert_eq!(found, listed, "a sample in spec/ that spec/mod.rs does not know about");
}

#[test]
fn every_sample_in_the_specification_parses() {
    for (reference, shape) in SAMPLES {
        let body = &spec::body(reference);
        let result = match shape {
            Shape::Unit | Shape::Illegal(_) | Shape::Blocked(_) | Shape::Unchecked(_) => {
                parses(body)
            }
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
fn every_sample_in_the_specification_parses_now() {
    // There was one that did not: §5.4's `Header h;`, a declaration with no
    // initialiser, which §4.2 did not have until 0116 settled it and 0153
    // built it. Nothing is `Blocked` any more, and this says so rather than
    // the absence quietly meaning nothing.
    let blocked: Vec<&str> =
        SAMPLES.iter().filter(|(_, s)| matches!(s, Shape::Blocked(_))).map(|(r, _)| *r).collect();
    assert!(blocked.is_empty(), "these do not parse: {blocked:?}");
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

// ── the naming itself ───────────────────────────────────────────────────────

/// 0137: inserting a paragraph into the specification moves no sample.
///
/// The table used to be keyed `file.md:line`, so editing prose cost an edit in
/// three other files and said `no entry found for key` when somebody forgot.
/// This writes a copy of `spec/` with a paragraph pushed in above every
/// section heading and checks that every sample still has the name it had.
#[test]
fn a_paragraph_can_be_inserted_anywhere_and_no_sample_moves() {
    let before: Vec<String> = spec::samples().into_iter().map(|s| s.key).collect();

    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let copy = std::env::temp_dir().join(format!("nether-spec-{at}"));
    std::fs::create_dir_all(&copy).expect("a scratch directory");
    for entry in std::fs::read_dir(spec::spec_dir()).expect("spec/ is there") {
        let path = entry.expect("readable").path();
        if path.extension().is_none_or(|e| e != "md") {
            continue;
        }
        let text = std::fs::read_to_string(&path).expect("readable");
        let pushed: Vec<String> = text
            .lines()
            .map(|l| {
                if l.starts_with("## ") {
                    format!("A paragraph somebody added.\n\n{l}")
                } else {
                    l.to_string()
                }
            })
            .collect();
        std::fs::write(copy.join(path.file_name().unwrap()), pushed.join("\n")).expect("written");
    }

    let after: Vec<String> = spec::samples_in(&copy).into_iter().map(|s| s.key).collect();
    std::fs::remove_dir_all(&copy).ok();
    assert_eq!(before, after, "a sample changed name because prose moved");
    assert_eq!(before.len(), SAMPLES.len(), "the copy lost or gained a sample");
}

// ── the one reserved word with nothing behind it ────────────────────────────

/// §3.4 reserves `sizeof` and §4.5 has no production for it, so writing one is
/// an error — and the error says what to write instead.
///
/// It stays a keyword rather than becoming an ordinary identifier because a C
/// programmer will type it, and a program that had bound the word to something
/// of its own would be the worse outcome. 0123, and §90.2.
#[test]
fn sizeof_is_reserved_and_refused_and_names_len() {
    let faults = parses("I64 n = sizeof(I64);\n").expect_err("`sizeof` has no meaning");
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].kind, FaultKind::Reserved("sizeof"));

    let d = faults[0].diagnostic();
    assert_eq!(d.headline, "`sizeof` is reserved and has no meaning");
    assert!(d.note.is_some_and(|n| n.contains("`len`")), "it does not say what to write instead");
}

// ── the limit the grammar needs and the host does not have ──────────────────

/// §6.4: an implementation states its limits and reports reaching one rather
/// than crashing into it. §4.6's ladder is ten frames per level of nesting, so
/// before this a hundred parentheses aborted the process.
#[test]
fn nesting_past_the_limit_is_a_fault_and_not_a_crash() {
    let deep = |n: usize| format!("I64 a = {}1{};\n", "(".repeat(n), ")".repeat(n));

    let inside = usize::try_from(nether_syntax::MAX_NESTING).expect("a small number") - 2;
    assert!(parses(&deep(inside)).is_ok(), "{inside} deep should still parse");

    let faults = parses(&deep(100_000)).expect_err("this is deeper than the parser goes");
    assert_eq!(faults[0].kind, FaultKind::TooDeep, "{faults:?}");
    let d = faults[0].diagnostic();
    assert!(
        d.headline.contains(&nether_syntax::MAX_NESTING.to_string()),
        "the limit is not named: {}",
        d.headline
    );
    assert!(d.note.is_some_and(|n| n.contains("not of the language")));
}

/// The other two shapes that recurse: a prefix operator chain, which does not
/// pass through the precedence ladder, and a type argument.
#[test]
fn a_prefix_chain_and_a_nested_type_are_bounded_too() {
    let ops = format!("I64 a = {}1;\n", "!".repeat(100_000));
    assert_eq!(parses(&ops).expect_err("too deep")[0].kind, FaultKind::TooDeep);

    let n = 100_000;
    let ty = format!("{}I64{} a = 1;\n", "Answer<".repeat(n), ">".repeat(n));
    assert_eq!(parses(&ty).expect_err("too deep")[0].kind, FaultKind::TooDeep);
}
