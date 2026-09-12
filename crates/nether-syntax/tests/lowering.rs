//! The proof for *lowering: AST to IR*: a round trip from source to IR and
//! back to source is semantically identical.
//!
//! "Semantically identical" is not "textually identical", and it should not
//! be: the printer parenthesises everything and lowering folds four constructs
//! into two. What has to survive is the IR. Source that lowers to an IR that
//! prints to source that lowers to *the same IR* is source nothing was lost
//! from, and the second IR is compared against the first node for node.
//!
//! Every unit is also handed to `nether_core::check`, which derives the same
//! depths by its own route. Two implementations of §2.2 agreeing is worth more
//! than one agreeing with itself.

use nether_core::{Depth, ExprKind, Type, check};
use nether_syntax::{Fault, lower, parse, print};

fn ir(src: &str) -> nether_core::Unit {
    let ast = parse(src.as_bytes()).unwrap_or_else(|f| panic!("does not parse: {f:?}"));
    lower(&ast).unwrap_or_else(|f: Vec<Fault>| panic!("does not lower: {f:?}"))
}

/// Lower, print, lower again, and insist the two agree.
fn round_trip(src: &str) -> nether_core::Unit {
    let first = ir(src);
    let printed = print(&first);
    let ast = parse(printed.as_bytes())
        .unwrap_or_else(|f| panic!("what was printed does not parse: {f:?}\n{printed}"));
    let second =
        lower(&ast).unwrap_or_else(|f| panic!("what was printed does not lower: {f:?}\n{printed}"));

    // Spans move, because the printed source is not the source. Nothing else
    // is allowed to.
    let (a, b) = (forget_spans(&first), forget_spans(&second));
    if let Some(at) =
        a.char_indices().zip(b.chars()).find(|((_, x), y)| x != y).map(|((i, _), _)| i)
    {
        let from = at.saturating_sub(120);
        panic!(
            "the round trip lost something at {at}:\n  before: …{}…\n   after: …{}…\n\n{printed}",
            &a[from..(at + 120).min(a.len())],
            &b[from..(at + 120).min(b.len())],
        );
    }
    assert_eq!(a.len(), b.len(), "the round trip lost something:\n{printed}");
    first
}

/// A unit with every span set to nothing, so two of them can be compared
/// without comparing the files they came from.
fn forget_spans(unit: &nether_core::Unit) -> String {
    let text = format!("{unit:?}");
    let mut out = String::with_capacity(text.len());
    let mut rest = text.as_str();
    while let Some(at) = rest.find("span: Span { start: ") {
        out.push_str(&rest[..at]);
        let after = &rest[at..];
        let close = after.find(" }").map_or(after.len(), |i| i + 2);
        rest = &after[close..];
    }
    out.push_str(rest);
    out
}

fn clean(unit: &nether_core::Unit) {
    let faults = check(unit);
    assert!(faults.is_empty(), "the calculus and lowering disagree: {faults:?}");
}

// ── the round trip ──────────────────────────────────────────────────────────

#[test]
fn the_smallest_unit_there_is() {
    let unit = round_trip("demand 1;\n");
    clean(&unit);
    assert_eq!(unit.demands.len(), 1);
}

#[test]
fn every_construct_in_section_04_survives_the_round_trip() {
    let src = r#"
struct Header {
  I64   len;
  Bytes tag;
};

typedef I64[16] Row;

Str name = "kernel.nc";

I64 head_len(Header h)
{
  return h.len;
}

Bytes load(Str p)
{
  descend disk { must(read(p)) }
}

I64 counted(Row window, Bool flag)
{
  I64 total = 0;
  for (I64 i = 0; i < 16; i += 1) {
    if (window[i] < 0) { continue; }
    total += window[i];
  }
  while (total > 100) { total -= 1; }
  b"nc";
  sizeof(Header);
  Bool both = flag && (total == 0);
  Bool either = flag || (total != 0);
  if (both) { total = 1; } else if (either) { total = 2; } else { total = 3; }
  Shade<Bytes> hidden = descend net { shade must(get(name)) };
  Cairn id = seal hidden;
  Bytes body = opaque (descend net { look hidden });
  { total; }
  return ((-total) + (~total));
}

demand load(name);
"#;
    let unit = round_trip(src);
    clean(&unit);
    assert_eq!(unit.structs.len(), 1);
    assert_eq!(unit.funcs.len(), 3);
    assert_eq!(unit.globals.len(), 1);
    assert_eq!(unit.demands.len(), 1);
}

#[test]
fn every_sample_in_the_specification_that_is_a_unit_survives_it() {
    for src in unit_samples() {
        let unit = round_trip(&src);
        clean(&unit);
    }
}

/// The samples §04 parses as whole units. The parser's own proof classifies
/// every sample in the specification; these are the ones it calls units.
fn unit_samples() -> Vec<String> {
    const UNITS: [(&str, usize); 8] = [
        ("00-overview.md", 125),
        ("01-strata.md", 52),
        ("01-strata.md", 94),
        ("02-calculus.md", 159),
        ("03-lexical.md", 26),
        ("05-types.md", 71),
        ("06-evaluation.md", 33),
        ("90-rationale.md", 340),
    ];
    UNITS
        .iter()
        .map(|(file, line)| {
            let path = format!("{}/../../spec/{file}", env!("CARGO_MANIFEST_DIR"));
            let text = std::fs::read_to_string(&path).expect("readable");
            let lines: Vec<&str> = text.lines().collect();
            let body: Vec<&str> =
                lines[*line..].iter().take_while(|l| !l.starts_with("```")).copied().collect();
            // The samples name things the specification does not define in the
            // fence — `compile`, `other`, `src` — so they arrive with the
            // declarations a unit would need.
            format!("{}\n{}\n", PRELUDE_FOR_SAMPLES, body.join("\n"))
        })
        .collect()
}

/// What the samples assume exists around them.
const PRELUDE_FOR_SAMPLES: &str = r#"
Bytes compile(Bytes s) { concat(b"obj:", s) }
Bytes other = b"";
Bytes src = b"";
Str p = "";
"#;

// ── what lowering does ──────────────────────────────────────────────────────

#[test]
fn short_circuit_becomes_a_branch() {
    let unit = ir("Bool a = true; Bool b = (a && a); Bool c = (a || a);\n");
    for g in &unit.globals[1..] {
        assert!(matches!(g.value.kind, ExprKind::Select { .. }), "{:?}", g.value.kind);
    }
}

#[test]
fn a_compound_assignment_is_an_assignment_of_a_binary() {
    let unit = ir("U0 f() { I64 n = 1; n += 2; }\n");
    let f = &unit.funcs[0];
    let nether_core::Stmt::Expr(x) = &f.body.stmts[1] else { panic!() };
    let ExprKind::Assign { value, .. } = &x.kind else { panic!("{:?}", x.kind) };
    assert!(matches!(value.kind, ExprKind::Binary { op: nether_core::BinOp::Add, .. }));
}

#[test]
fn a_typedef_does_not_survive() {
    let unit = round_trip("typedef I64[16] Row;\nI64 first(Row r) { r[0] }\n");
    let row = Type::Array { elem: Box::new(Type::Int), len: Some(16) };
    assert_eq!(unit.funcs[0].locals[0].ty, row, "the alias was replaced by what it names");
    clean(&unit);
}

#[test]
fn a_written_depth_becomes_an_assertion_and_not_a_type() {
    let unit = ir("Bytes@3 src = descend disk { must(read(\"k\")) };\n");
    assert_eq!(unit.globals[0].ty, Type::Bytes, "the depth is not part of the type");
    assert_eq!(unit.globals[0].asserted, Some(Depth::DISK));
    assert_eq!(unit.globals[0].value.depth, Depth::DISK, "and it is the depth that was derived");
    clean(&unit);
}

#[test]
fn a_depth_that_disagrees_with_inference_is_caught_by_the_checker() {
    // Lowering derives; the checker compares. §5.6: an annotation is an
    // assertion and never a coercion.
    let unit = ir("Bytes@0 src = descend disk { must(read(\"k\")) };\n");
    let faults = check(&unit);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert!(faults[0].to_string().contains("annotated @0"));
}

#[test]
fn a_shade_takes_its_origin_from_what_it_holds() {
    let unit = ir("Shade<Bytes> s = descend net { shade must(get(\"u\")) };\n");
    assert_eq!(
        unit.globals[0].ty,
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) }
    );
    clean(&unit);
}

#[test]
fn a_function_that_descends_for_itself_asks_for_nothing() {
    let unit = ir("Bytes load(Str p) { descend disk { must(read(p)) } }\n");
    assert_eq!(unit.funcs[0].latent, Depth::PURE);
    assert_eq!(unit.funcs[0].ret_depth, Depth::DISK);
    clean(&unit);
}

#[test]
fn a_function_that_does_not_asks_its_caller() {
    let unit = ir("Answer<Bytes> raw(Str p) @3 { read(p) }\n");
    assert_eq!(unit.funcs[0].latent, Depth::DISK);
    clean(&unit);
}

#[test]
fn a_name_that_is_not_there_is_one_complaint() {
    let ast = parse(b"demand nowhere;\n").expect("parses");
    let faults = lower(&ast).expect_err("that name is not bound");
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].to_string(), "this does not name this name");
}

#[test]
fn a_capability_that_is_not_in_the_prelude_is_refused() {
    let ast = parse(b"U0 f() { descend sudo { 1 }; }\n").expect("parses");
    let faults = lower(&ast).expect_err("there is no such capability");
    assert_eq!(faults.len(), 1, "{faults:?}");
}
