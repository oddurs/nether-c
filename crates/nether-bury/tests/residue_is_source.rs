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

/// A residue that did not come from that unit is a bug here, not a program to
/// report on. Zipping the two would drop a demand and break the staging law
/// with nothing said.
#[test]
#[should_panic(expected = "demands and the unit it is said to come from has")]
fn a_residue_from_a_different_unit_is_refused() {
    let one = unit("demand 1;\n");
    let two = unit("demand 1;\ndemand 2;\n");
    let r = bury(&one, Cairn::of_encoded(b"one"), 100).expect("this finishes");
    let _ = r.as_unit(&two);
}

/// §3.6's byte escape exists for this. A `Bytes` value is not text, and the
/// escape that names a scalar value spells two bytes as one character, which
/// comes back as four.
#[test]
fn a_bytes_value_survives_whatever_bytes_are_in_it() {
    residue_survives("demand b\"\u{e9}\";\n");
    residue_survives("demand b\"\u{96ea}:\u{0}\\xff\";\n");
    residue_survives("demand \"\u{e9}\u{96ea}\";\n");
    residue_survives("demand concat(b\"\\x00\\xff\", b\"\\x80\");\n");
}

#[test]
fn rites_descents_loops_aggregates_and_precedence_survive_round_trip() {
    for src in [
        "demand opaque (1 + 2 * 3);\n",
        "demand seal b\"stone\";\n",
        "Shade<Bytes> s = descend disk { shade must(read(\"a.txt\")) };\ndemand descend disk { look s };\n",
        "Bool b = must(descend env { env(\"FLAG\") }) == \"1\";\nI64 f(Bool c) @0 {\n  if (c && !false || false) {\n    return (1 + 2) * 3;\n  } else {\n    return 10 - (4 - 1);\n  }\n}\ndemand f(b);\n",
        "struct Pair { I64 a; I64 b; };\nI64 sum_pair(I64 x) @0 {\n  Pair p;\n  p.a = x;\n  p.b = x + 1;\n  return p.a + p.b;\n}\nI64 n = len(must(descend disk { read(\"in.bin\") }));\ndemand sum_pair(n);\n",
    ] {
        residue_survives(src);
    }
}

#[test]
fn staged_burial_through_printed_residue_matches_direct_burial() {
    // Program with two holes at different strata (stratum 1 `env` and stratum 3 `disk`),
    // pure arithmetic, and a speculative branch.
    let src = r#"Str@1 mode = must(descend env { env("MODE") });
Bytes@3 body = must(descend disk { read("payload.bin") });
I64 pick(Str m, Bytes b) @0 {
  if (m == "fast") {
    return len(b) + (2 * 3);
  } else {
    return len(b) + (10 - 1);
  }
}
demand pick(mode, body);
"#;

    // Stage 0: initial burial leaves both `env("MODE")` and `read("payload.bin")`
    let u0 = unit(src);
    let r0 = bury(&u0, Cairn::of_encoded(src.as_bytes()), u64::MAX).expect("buries stage 0");
    assert_eq!(r0.holes.len(), 2);

    // Stage A (`env` answered) followed by Stage B (`disk` answered) via source print/parse/lower
    let printed_r0 = print(&r0.as_unit(&u0));
    let env_hole = "must(descend env {\n  env(\"MODE\")\n})";
    let disk_hole = "must(descend disk {\n  read(\"payload.bin\")\n})";
    assert!(printed_r0.contains(env_hole), "unexpected printed r0:\n{printed_r0}");
    assert!(printed_r0.contains(disk_hole), "unexpected printed r0:\n{printed_r0}");

    let src_after_a = printed_r0.replace(env_hole, "\"fast\"");
    let u_a = unit(&src_after_a);
    let r_a =
        bury(&u_a, Cairn::of_encoded(src_after_a.as_bytes()), u64::MAX).expect("buries stage A");
    assert_eq!(r_a.holes.len(), 1);

    let src_after_ab_staged = print(&r_a.as_unit(&u_a)).replace(disk_hole, "b\"hello\"");
    let u_ab_staged = unit(&src_after_ab_staged);
    let r_ab_staged =
        bury(&u_ab_staged, Cairn::of_encoded(src_after_ab_staged.as_bytes()), u64::MAX)
            .expect("buries stage B");

    // Direct single-stage burial with `A ∪ B` (`env` and `disk` both answered at once)
    let src_direct = printed_r0.replace(env_hole, "\"fast\"").replace(disk_hole, "b\"hello\"");
    let u_direct = unit(&src_direct);
    let r_direct =
        bury(&u_direct, Cairn::of_encoded(src_direct.as_bytes()), u64::MAX).expect("buries direct");

    assert_eq!(r_ab_staged.holes.len(), 0);
    assert_eq!(r_direct.holes.len(), 0);
    assert_eq!(
        print(&r_ab_staged.as_unit(&u_ab_staged)),
        print(&r_direct.as_unit(&u_direct)),
        "staged burial residue diverged from direct burial residue"
    );
}
