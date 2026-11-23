//! The proof for *depth inference*: every sample program in the specification
//! compiles with all its depth annotations deleted.
//!
//! §5.6 puts it as a requirement rather than a hope:
//!
//! > Depth annotations must remain optional in ordinary code. The roadmap item
//! > *Depth inference* is proven by deleting every `@n` from every sample
//! > program in this specification and requiring them all still to compile.
//!
//! This is the ergonomics risk addressed directly. If people have to write
//! `@3` everywhere they will leave, so the test is that they never have to.
//!
//! And the other direction, which is worth as much: every `@n` the
//! specification *does* write is checked against what inference produces. An
//! annotation that disagrees is an error by §5.6, so a specification that
//! writes a wrong one is a specification that does not compile.

use nether_core::check;
use nether_syntax::{lower, parse};

mod spec;
use spec::Shape;

/// Every `c` sample under `spec/` that §04 parses, with what it needs around
/// it. What each sample is, is said once, in `tests/spec/mod.rs`.
fn around_for(body: &str, is_unit: bool) -> String {
    let declared: Vec<String> = if is_unit {
        parse(body.as_bytes()).map_or_else(
            |_| Vec::new(),
            |u| {
                u.items
                    .into_iter()
                    .filter_map(|i| match i {
                        nether_syntax::ast::Item::Func(f) => Some(f.name.text),
                        nether_syntax::ast::Item::Let(l) => Some(l.name.text),
                        _ => None,
                    })
                    .collect()
            },
        )
    } else {
        Vec::new()
    };
    let mut out = String::new();
    for (decl, name) in [
        ("Bytes compile(Bytes s) { concat(b\"obj:\", s) }\n", "compile"),
        ("Bytes other = b\"\";\n", "other"),
        ("Bytes src = b\"\";\n", "src"),
        ("Str p = \"\";\n", "p"),
        ("Answer<Bytes> a = descend disk { read(\"main.nc\") };\n", "a"),
    ] {
        if !declared.iter().any(|d| d == name) {
            out.push_str(decl);
        }
    }
    out
}

fn samples() -> Vec<(String, String)> {
    let mut out = Vec::new();
    for (key, body) in spec::of_shape(Shape::Unit) {
        let prelude = around_for(&body, true);
        out.push((key, format!("{prelude}\n{body}\n")));
    }
    for (key, body) in spec::of_shape(Shape::Statements) {
        let prelude = around_for(&body, false);
        out.push((key, format!("{prelude}\nU0 sample()\n{{\n{body}\n}}\n")));
    }
    out
}

/// Delete every optional depth annotation, in both the forms §3.5 allows,
/// while preserving required `@d` origin parameters inside `Shade<T@d>` (§4.3).
fn without_annotations(src: &str) -> String {
    let mut out = String::with_capacity(src.len());
    let mut rest = src;
    while let Some(at) = rest.find('@') {
        out.push_str(&rest[..at]);
        let after = &rest[at + 1..];
        let digits = after.trim_start();
        let eaten = after.len() - digits.len();
        match digits.chars().next() {
            Some(d) if d.is_ascii_digit() => {
                let tail = &after[eaten + 1..];
                if tail.trim_start().starts_with('>') {
                    out.push_str(&rest[at..=(at + 1 + eaten)]);
                }
                rest = tail;
            }
            _ => {
                out.push('@');
                rest = after;
            }
        }
    }
    out.push_str(rest);
    out
}

fn checks(src: &str) -> Result<(), String> {
    let ast = parse(src.as_bytes()).map_err(|f| format!("does not parse: {f:?}"))?;
    let unit = lower(&ast).map_err(|f| format!("does not lower: {f:?}"))?;
    let faults = check(&unit);
    if faults.is_empty() { Ok(()) } else { Err(format!("does not check: {}", faults[0])) }
}

// ── the proof ───────────────────────────────────────────────────────────────

#[test]
fn every_sample_compiles_with_every_annotation_deleted() {
    let mut stripped = 0;
    for (reference, src) in samples() {
        let bare = without_annotations(&src);
        if bare != src {
            stripped += 1;
        }
        if let Err(why) = checks(&bare) {
            panic!("{reference} {why}\n{bare}");
        }
    }
    // A corpus with no annotations in it would pass this without proving
    // anything at all.
    assert!(stripped >= 4, "only {stripped} samples had an annotation to delete");
}

#[test]
fn every_sample_compiles_with_them_left_in() {
    for (reference, src) in samples() {
        if let Err(why) = checks(&src) {
            panic!("{reference} {why}\n{src}");
        }
    }
}

#[test]
fn every_annotation_the_specification_writes_is_the_one_inference_gives() {
    // The other direction, and worth as much. §5.6 makes an annotation a
    // checked assertion, so a sample whose `@n` disagrees with inference is a
    // sample that does not compile — and the test above would pass anyway,
    // because it deletes them.
    let mut annotated = 0;
    for (reference, src) in samples() {
        // `AROUND` has no annotations in it, so an `@` here came from the
        // specification.
        if !src.contains('@') {
            continue;
        }
        annotated += 1;
        assert!(checks(&src).is_ok(), "{reference} writes an `@n` inference disagrees with");
    }
    assert!(annotated >= 4, "only {annotated} samples write one at all");
}

// ── what inference actually has to do ───────────────────────────────────────

#[test]
fn a_depth_travels_from_where_it_was_reached_to_where_it_is_used() {
    let src = r#"
Bytes deep = descend disk { must(read("k")) };
I64   n    = len(deep);
Bool  b    = (n > 0);
demand b;
"#;
    let unit = lower(&parse(src.as_bytes()).unwrap()).unwrap();
    assert!(check(&unit).is_empty());
    // Nothing here is annotated, and all three are at stratum 3.
    for g in &unit.globals {
        assert_eq!(g.value.depth, nether_core::Depth::DISK, "{}", g.name);
        assert_eq!(g.asserted, None);
    }
}

#[test]
fn a_latent_depth_is_inferred_from_a_body_that_did_not_descend() {
    let src = "Answer<Bytes> raw(Str p) { read(p) }\n";
    let unit = lower(&parse(src.as_bytes()).unwrap()).unwrap();
    assert!(check(&unit).is_empty());
    assert_eq!(unit.funcs[0].latent, nether_core::Held::of(nether_core::Depth::DISK));
    assert_eq!(unit.funcs[0].asserted_latent, None, "and nobody wrote it");
}

#[test]
fn an_annotation_that_disagrees_is_an_error_and_not_a_coercion() {
    // Both directions: claiming shallower and claiming deeper.
    for wrong in ["Bytes@0 x = descend disk { must(read(\"k\")) };", "Bytes@5 y = b\"\";"] {
        let unit = lower(&parse(wrong.as_bytes()).unwrap()).unwrap();
        let faults = check(&unit);
        assert_eq!(faults.len(), 1, "{wrong}: {faults:?}");
        assert!(faults[0].to_string().contains("inference gives"), "{}", faults[0]);
    }
}

#[test]
fn deleting_an_annotation_never_changes_what_a_program_means() {
    // The point of the whole item: `@n` is documentation that is checked, and
    // removing it removes the check and nothing else.
    let annotated = "Bytes@3 src = descend disk { must(read(\"k\")) };\ndemand src;\n";
    let bare = without_annotations(annotated);
    assert_eq!(bare, "Bytes src = descend disk { must(read(\"k\")) };\ndemand src;\n");

    let one = lower(&parse(annotated.as_bytes()).unwrap()).unwrap();
    let two = lower(&parse(bare.as_bytes()).unwrap()).unwrap();
    assert_eq!(one.globals[0].value.depth, two.globals[0].value.depth);
    assert_eq!(one.globals[0].asserted, Some(nether_core::Depth::DISK));
    assert_eq!(two.globals[0].asserted, None);
}
