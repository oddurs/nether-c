//! §6.5 now requires that a residue can be written down as source, and that
//! lowering what was written gives back the same program.
//!
//! Without that the staging law is unprovable: `bury(bury(p, A), B)` would not
//! be burying the first burial's residue, it would be burying something that
//! resembled it.

use nether_bury::bury;
use nether_core::Unit;
use nether_ledger::Cairn;
use nether_syntax::{lower, parse, print};

fn unit(src: &str) -> Unit {
    let ast = parse(src.as_bytes()).unwrap_or_else(|f| panic!("does not parse: {f:?}"));
    lower(&ast).unwrap_or_else(|f| panic!("does not lower: {f:?}"))
}

/// A residue, printed and lowered again, is the same program.
///
/// Compared as printed source rather than as IR. Two lowerings of two
/// different texts carry different spans, and a span is a fact about where
/// something was written, not about what it is. What has to hold is that
/// burying a residue leaves that residue alone — which is the staging law
/// with `B` empty, and the smallest case of it that can be checked.
fn residue_survives(src: &str) {
    let first = unit(src);
    let once = bury(&first, Cairn::of_encoded(src.as_bytes()), u64::MAX).expect("buries");
    let printed = print(&once.as_unit(&first));

    let ast = parse(printed.as_bytes())
        .unwrap_or_else(|f| panic!("a residue does not parse:\n{printed}\n{f:?}"));
    let again =
        lower(&ast).unwrap_or_else(|f| panic!("a residue does not lower:\n{printed}\n{f:?}"));

    let twice =
        bury(&again, Cairn::of_encoded(printed.as_bytes()), u64::MAX).expect("buries again");
    let reprinted = print(&twice.as_unit(&again));

    assert_eq!(printed, reprinted, "burying a residue changed it\n--- from ---\n{src}");
}

#[test]
fn a_residue_that_is_a_literal_survives() {
    residue_survives("demand 1 + 2;\n");
}

#[test]
fn a_residue_with_a_branch_in_it_survives() {
    residue_survives("I64 f(I64 n) @0 { if (n) { return 1; } else { return 2; } }\ndemand f(3);\n");
}

#[test]
fn every_construct_a_burial_can_leave_survives() {
    for src in [
        "demand 1;\n",
        "demand 1 + 2 * 3;\n",
        "I64 f(I64 n) @0 { return n; }\ndemand f(7);\n",
        "U0 g() @0 { \"a\"; }\ndemand g();\n",
    ] {
        residue_survives(src);
    }
}
