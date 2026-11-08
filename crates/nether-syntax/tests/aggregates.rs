//! §4.2 and §5.4: a local may be declared without a value, and assigned.
//!
//! Three constructs depended on this and none of them could be reached before
//! it: aggregate construction, the `assign` expression, and the `for` step.

use nether_core::{Stmt, check};
use nether_syntax::{lower, parse, print};

fn ir(src: &str) -> nether_core::Unit {
    let ast = parse(src.as_bytes()).unwrap_or_else(|f| panic!("does not parse: {f:?}\n{src}"));
    lower(&ast).unwrap_or_else(|f| panic!("does not lower: {f:?}\n{src}"))
}

fn checks(src: &str) -> nether_core::Unit {
    let unit = ir(src);
    let faults = check(&unit);
    assert!(faults.is_empty(), "does not check: {faults:?}\n{src}");
    unit
}

/// §5.4's own sample, which did not parse under §04 until 0116 settled it.
#[test]
fn the_sample_in_section_five_point_four_compiles() {
    checks(
        "struct Header {\n  I64   len;\n  Bytes tag;\n};\n\n\
         U0 build() @0\n{\n  Header h;\n  h.len = 3;\n  h.tag = b\"nc\";\n  seal h;\n}\n",
    );
}

/// A declaration with no value is its own statement, not a `Let` of nothing.
#[test]
fn a_declaration_without_a_value_lowers_to_declare() {
    let unit = ir("U0 g() @0 { I64 n; n = 1; }\n");
    let body = &unit.funcs[0].body;
    assert!(matches!(body.stmts.first(), Some(Stmt::Declare { .. })), "{:?}", body.stmts.first());
}

/// The `for` step assigns its counter, which is what makes a loop advance.
#[test]
fn a_for_loop_can_advance_its_own_counter() {
    checks("I64 f(I64 n) @0 { I64 t = 0; for (I64 i = 0; i < n; i += 1) { t += i; } t }\n");
}

/// A global must have one: there is no statement above it to assign one.
#[test]
fn a_global_without_a_value_is_refused() {
    let ast = parse(b"I64 n;\ndemand 1;\n");
    assert!(ast.is_err(), "a global with no value was accepted");
}

/// A residue is source (§6.5), so a declaration has to print back as one.
#[test]
fn a_declaration_survives_being_printed_and_lowered_again() {
    let src = "U0 g() @0 { I64 n; n = 7; }\n";
    let first = ir(src);
    let printed = print(&first);
    assert!(printed.contains("I64 n;"), "the declaration did not print back:\n{printed}");

    // Compared as printed source: two lowerings of two different texts carry
    // different spans, and a span is a fact about where something was written.
    let again = ir(&printed);
    assert_eq!(printed, print(&again), "printing a residue twice gave two programs");
}

/// Arrays are built the same way, with index assignment rather than fields.
#[test]
fn an_array_is_built_by_index() {
    checks("U0 g() @0 { I64[3] xs; xs[0] = 1; xs[1] = 2; seal xs; }\n");
}

/// Writing a deep value into a field or across a branch or loop deepens the
/// local, and lowering and checking agree on the resulting depth. 0226.
#[test]
fn assigning_a_deep_value_deepens_the_local_across_blocks_branches_and_loops() {
    checks(
        "struct Stamp { I64 when; };\n\
         I64@2 stamp() @0 {\n\
           Stamp s;\n\
           descend env { s.when = clock(); };\n\
           return s.when;\n\
         }\n",
    );
    checks(
        "struct Stamp { I64 when; };\n\
         I64@2 branched(Bool c) @0 {\n\
           Stamp s;\n\
           if (c) {\n\
             descend env { s.when = clock(); };\n\
           } else {\n\
             s.when = 0;\n\
             I64@0 pure_arm = s.when;\n\
           }\n\
           return s.when;\n\
         }\n",
    );
    checks(
        "I64@2 looped(I64 n) @0 {\n\
           I64 total = 0;\n\
           for (I64 i = 0; i < n; i += 1) {\n\
             total += descend env { clock() };\n\
           }\n\
           return total;\n\
         }\n",
    );
}

/// A loop hands each turn the depths the last one left, so `x`'s depth reaches
/// `y` on the second turn and `z` on the third. Lowering looked ahead one turn
/// and called `z` pure, and the checker agreed with it. 0284.
#[test]
fn a_depth_a_loop_carries_reaches_every_link_of_a_chain() {
    let chain = |name: &str| {
        format!(
            "U0 f(Bool c) @0 {{\n\
               Bytes x = b\"\";\n\
               Bytes y = b\"\";\n\
               Bytes z = b\"\";\n\
               while (c) {{ z = y; y = x; x = must(descend disk {{ read(\"f\") }}); }}\n\
               Bytes@0 w = {name};\n\
             }}\n"
        )
    };
    for name in ["x", "y", "z"] {
        let src = chain(name);
        let faults = check(&ir(&src));
        assert!(!faults.is_empty(), "{name} holds a disk read and was called pure:\n{src}");
    }
    checks(&chain("z").replace("Bytes@0 w", "Bytes@3 w"));
}

/// A loop that deepens nothing is lowered once. Lowering each body twice cost
/// twice as much per level of nesting, and forty levels never finished. 0284.
#[test]
fn forty_nested_loops_lower_once_each() {
    let depth = 40;
    let src = format!(
        "U0 f(Bool c) @0 {{\n  I64 x = 0;\n  {}x += 1;{}\n}}\n",
        "while (c) { ".repeat(depth),
        " }".repeat(depth)
    );
    checks(&src);
}
