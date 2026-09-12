//! §5.4: a local may be assigned until its value is **named**.
//!
//! Named means used as a value — sealed, shaded, deposited, returned, or
//! passed as an argument. After that its cairn exists, and nothing can change
//! what a cairn names. Without this the ledger's central promise is a lie: a
//! program could seal a value and then alter it.
//!
//! Arithmetic on a local does not name it. `t + 1` is a different value with a
//! different cairn, and `t`'s own name never existed — which is exactly why a
//! `for` loop can advance its counter.

use nether_core::{FaultKind, check};
use nether_syntax::{lower, parse};

fn faults(src: &str) -> Vec<FaultKind> {
    let ast = parse(src.as_bytes()).unwrap_or_else(|f| panic!("does not parse: {f:?}\n{src}"));
    let unit = lower(&ast).unwrap_or_else(|f| panic!("does not lower: {f:?}\n{src}"));
    check(&unit).into_iter().map(|f| f.kind).collect()
}

fn frozen(src: &str) {
    let found = faults(src);
    assert!(
        found.iter().any(|f| matches!(f, FaultKind::Frozen { .. })),
        "this was accepted and should not be:\n{src}\n{found:?}"
    );
}

fn clean(src: &str) {
    let found = faults(src);
    assert!(found.is_empty(), "this was refused and should not be:\n{src}\n{found:?}");
}

// ── the five ways a value is named ──────────────────────────────────────────

#[test]
fn sealing_names_it() {
    frozen("U0 g() @0 { I64 n; n = 1; seal n; n = 2; }\n");
}

#[test]
fn shading_names_it() {
    frozen("U0 g() @0 { I64 n; n = 1; shade n; n = 2; }\n");
}

#[test]
fn depositing_names_it() {
    frozen("U0 g() @0 { I64 n; n = 1; n; n = 2; }\n");
}

/// Returning names it, and a return on one arm freezes it for the rest.
///
/// The conservative answer — frozen if any path names it — is the only sound
/// one: the checker does not know which arm runs, and the value is named on
/// the one that does.
#[test]
fn returning_names_it_even_on_one_arm() {
    frozen("I64 g(I64 c) @0 { I64 n; n = 1; if (c) { return n; } n = 2; return n; }\n");
}

#[test]
fn passing_it_as_an_argument_names_it() {
    frozen("U0 f(I64 x) @0 { }\nU0 g() @0 { I64 n; n = 1; f(n); n = 2; }\n");
}

/// §5.4's own sample, which is the case the rule exists for.
#[test]
fn the_sample_in_section_five_point_four_is_refused_at_its_last_line() {
    frozen(
        "struct Header {\n  I64   len;\n  Bytes tag;\n};\n\n\
         U0 build() @0\n{\n  Header h;\n  h.len = 3;\n  h.tag = b\"nc\";\n\
         \x20 Cairn c = seal h;\n  h.len = 4;\n}\n",
    );
}

/// A field is part of the value, so naming the whole freezes the parts.
#[test]
fn naming_an_aggregate_freezes_its_fields() {
    frozen("struct P { I64 x; };\nU0 g() @0 { P p; p.x = 1; seal p; p.x = 2; }\n");
}

// ── what does not name ──────────────────────────────────────────────────────

/// The whole reason this rule is about naming and not about reading.
#[test]
fn a_for_loop_can_still_advance_its_counter() {
    clean("I64 f(I64 n) @0 { I64 t = 0; for (I64 i = 0; i < n; i += 1) { t += i; } t }\n");
}

/// `t + 1` is a different value. `t` never had a name.
#[test]
fn arithmetic_does_not_name() {
    clean("U0 g() @0 { I64 n; n = 1; I64 m = n + 1; n = 2; }\n");
}

/// Sealing something computed *from* a local does not name the local.
///
/// The parentheses are load-bearing: §4.6 puts `seal` above `+`, so
/// `seal n + 1` is `(seal n) + 1` and that *does* name `n`.
#[test]
fn sealing_a_fresh_value_does_not_name_what_it_came_from() {
    clean("U0 g() @0 { I64 n; n = 1; seal (n + 1); n = 2; }\n");
}

/// And the precedence case itself, because it is the one that surprises.
#[test]
fn seal_binds_tighter_than_arithmetic_and_so_it_names() {
    frozen("U0 g() @0 { I64 n; n = 1; seal n + 1; n = 2; }\n");
}

#[test]
fn building_an_aggregate_before_naming_it_is_fine() {
    clean(
        "struct Header {\n  I64   len;\n  Bytes tag;\n};\n\
         U0 build() @0 { Header h; h.len = 3; h.tag = b\"nc\"; seal h; }\n",
    );
}

/// The first naming is the one reported: it is where the value stopped being
/// the program's to change.
#[test]
fn the_blame_is_the_first_naming() {
    let src = "U0 g() @0 { I64 n; n = 1; seal n; seal n; n = 2; }\n";
    let ast = parse(src.as_bytes()).expect("parses");
    let unit = lower(&ast).expect("lowers");
    let found = check(&unit);
    let FaultKind::Frozen { named } = found[0].kind else { panic!("{:?}", found[0].kind) };
    let at = usize::try_from(named.start).expect("fits");
    assert!(src[at..].starts_with('n'), "points at {:?}", &src[at..at + 6]);
    // The first `seal n`, not the second.
    assert!(at < src.rfind("seal").expect("two seals"), "blamed the later naming");
}
