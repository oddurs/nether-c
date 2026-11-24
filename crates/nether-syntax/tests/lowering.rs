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

use nether_core::{Depth, ExprKind, Held, Type, check};
use nether_syntax::{Fault, lower, parse, print};

mod spec;
use spec::Shape;

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

/// The samples §04 parses as whole units. What each sample is, is said once,
/// in `tests/spec/mod.rs`.
fn unit_samples() -> Vec<String> {
    spec::of_shape(Shape::Unit)
        .into_iter()
        .map(|(_, body)| {
            let declared: Vec<String> = parse(body.as_bytes()).map_or_else(
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
            );
            let mut prelude = String::new();
            for (decl, name) in [
                ("Bytes compile(Bytes s) { concat(b\"obj:\", s) }\n", "compile"),
                ("Bytes other = b\"\";\n", "other"),
                ("Bytes src = b\"\";\n", "src"),
                ("Str p = \"\";\n", "p"),
            ] {
                if !declared.iter().any(|d| d == name) {
                    prelude.push_str(decl);
                }
            }
            format!("{prelude}\n{body}\n")
        })
        .collect()
}

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
    assert_eq!(unit.funcs[0].latent, Held::NONE);
    assert_eq!(unit.funcs[0].ret_depth, Depth::DISK);
    clean(&unit);
}

#[test]
fn a_function_that_does_not_asks_its_caller() {
    let unit = ir("Answer<Bytes> raw(Str p) @3 { read(p) }\n");
    assert_eq!(unit.funcs[0].latent, Held::of(Depth::DISK));
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

#[test]
fn a_cairn_literal_survives_the_round_trip() {
    // §6.5 is a MUST: a residue is a program. A burial that folds `seal` has a
    // cairn to write down, and until §3.6 had a literal the printer emitted a
    // placeholder that did not parse — with no capability granted and none
    // needed, because `seal` is depth 0. 0172, 0173.
    let name = "f1353fd9d1aea164452daae9d822f156fb89e8f4d7734cf382d453f0afcfc60f";
    let unit = round_trip(&format!("Cairn c = #{name};\ndemand c;\n"));
    assert!(print(&unit).contains(&format!("#{name}")), "{}", print(&unit));
}

/// A call holds its callee's result depth, and a callee's is not known until
/// its body is lowered. A chain settles one link per pass, so a chain longer
/// than the bound used to settle on the wrong depth and check cleanly anyway.
///
/// Burial's residue is where this bites: §6.5 mints a function per starved
/// call, and the chain is as deep as the specialisation went.
#[test]
fn a_call_chain_longer_than_a_handful_still_settles() {
    use std::fmt::Write as _;
    let mut src = String::from("Bytes deep() { descend disk { must(read(\"k\")) } }\n");
    for i in 0..24 {
        let below = if i == 0 { "deep".to_string() } else { format!("up{}", i - 1) };
        let _ = writeln!(src, "Bytes up{i}() {{ {below}() }}");
    }
    let unit = ir(&src);
    for f in &unit.funcs {
        assert_eq!(f.ret_depth, Depth::DISK, "{}", f.name);
    }
    clean(&unit);
}

#[test]
fn struct_type_faults_are_not_swallowed_by_fixpoint_passes() {
    let ast = parse(b"struct Bad { NoSuch x; };\nU0 f() { 1; }\n").unwrap();
    let faults = lower(&ast).expect_err("unknown struct field type must fail");
    assert!(!faults.is_empty());
}

#[test]
fn cyclic_typedef_through_generic_type_argument_fails_without_overflow() {
    let ast = parse(b"typedef Answer<A> A;\nA f() { }\n").unwrap();
    let faults = lower(&ast).expect_err("cyclic typedef must fail");
    assert!(!faults.is_empty());
}

#[test]
fn received_shade_origin_survives_lowering_and_round_trip() {
    let src = "Cairn keep(Shade<Bytes@5> s) @0 { seal s }\n";
    let unit = round_trip(src);
    assert_eq!(
        unit.funcs[0].locals[0].ty,
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) }
    );
    clean(&unit);
}

#[test]
fn refused_inherits_landing_type_across_blocks_descend_opaque_assign_and_calls() {
    let src = r"
Answer<Bytes> tail_fn() { refused(absent) }
Answer<Bytes> block_fn() { ({ refused(absent) }) }
Answer<Bytes> block_let() { Answer<Bytes> b = { refused(absent) }; b }
Answer<Bytes> descend_fn() { descend disk { refused(absent) } }
Answer<Bytes> opaque_fn() { opaque refused(absent) }
U0 take(Answer<Bytes> a) { a; }
U0 assign_and_call() {
  Answer<Bytes> x;
  x = refused(absent);
  take(refused(absent));
}
";
    let unit = round_trip(src);
    clean(&unit);
}

#[test]
fn round_trip_handles_multidim_arrays_nested_generics_negative_int_and_leading_block() {
    let src = r"
I64[2][3] arr_fn(I64[2][3] m) { m }
Answer<Answer<I64>> nested(Answer<Answer<I64>> a) { a }
I64 neg_min() { 0x8000000000000000 }
U0 lead_block() { ({ 1 }) + 2; }
";
    let unit = round_trip(src);
    clean(&unit);
}

#[test]
fn duplicate_declarations_and_misplaced_control_flow_are_rejected() {
    for bad in [
        "struct S { I64 a; I64 a; };\n",
        "struct S { I64 a; }; struct S { I64 b; };\n",
        "typedef I64 T; typedef Str T;\n",
        "U0 f() {} U0 f() {}\n",
        "I64 g = 1; I64 g = 2;\n",
        "U0 f(I64 x, I64 x) {}\n",
        "U0 f() { break; }\n",
        "U0 f() { continue; }\n",
        "I64 g = { return 1; };\n",
        "U0 f() { I64 x = 1; x[0] = 2; }\n",
        "U0 f() { I64<Str> x = 1; }\n",
        "U0 f() { must(1); }\n",
        "U0 f() { answered(1, 2); }\n",
    ] {
        let ast = parse(bad.as_bytes()).unwrap_or_else(|e| panic!("should parse: {bad}\n{e:?}"));
        assert!(lower(&ast).is_err(), "expected lowering error for: {bad}");
    }
}

#[test]
fn parser_rejects_trailing_commas_and_bounds_blocks_ifs_and_assignments() {
    assert!(parse(b"U0 f(I64 a,) {}\n").is_err());
    assert!(parse(b"U0 f() { g(1,); }\n").is_err());
    // `a < b > c + 1;` is an expression, not a binding declaration.
    assert!(parse(b"U0 f(I64 a, I64 b, I64 c) { a < b > c + 1; }\n").is_ok());

    let deep_blocks = format!("U0 f() {}{}", "{ ".repeat(80), "}".repeat(80));
    assert!(parse(deep_blocks.as_bytes()).is_err());

    let mut deep_if = String::from("U0 f(Bool c) { if (c) {}");
    for _ in 0..80 {
        deep_if.push_str(" else if (c) {}");
    }
    deep_if.push_str(" }");
    assert!(parse(deep_if.as_bytes()).is_err());

    let deep_assign = format!("U0 f() {{ {}1; }}", "a = ".repeat(80));
    assert!(parse(deep_assign.as_bytes()).is_err());
}

#[test]
fn implicit_flow_in_if_while_and_indexed_assign_deepens_assigned_locals() {
    let src = r#"
I64 from_if() {
  I64 x = 0;
  Bytes b = descend disk { must(read("f")) };
  if (len(b) > 0) {
    x = 1;
  }
  return x;
}
I64 from_while() {
  I64 x = 0;
  Bytes b = descend disk { must(read("f")) };
  while (len(b) > x) {
    x = x + 1;
  }
  return x;
}
I64 from_early_return() {
  Bytes b = descend disk { must(read("f")) };
  if (len(b) > 0) {
    return 1;
  }
  return 0;
}
"#;
    let unit = ir(src);
    for f in &unit.funcs {
        assert_eq!(f.ret_depth, Depth::DISK, "{}", f.name);
    }
    clean(&unit);
}

#[test]
fn shade_origin_cannot_be_laundered_across_bindings_calls_returns_or_fields() {
    for exploit in [
        r#"Bytes@0 steal() { Shade<Bytes@0> s = descend disk { shade must(read("f")) }; look s }"#,
        r#"Bytes@0 open(Shade<Bytes@0> s) { look s } Bytes@0 steal() { open(descend unrecorded { shade must(call_foreign("x", b"")) }) }"#,
        r#"Shade<Bytes@0> forge() { descend net { shade must(get("u")) } } Bytes@0 steal() { look forge() }"#,
        r#"struct B { Shade<Bytes@0> s; }; Bytes@0 steal() { B b; b.s = descend net { shade must(get("u")) }; look b.s }"#,
    ] {
        let ast = parse(exploit.as_bytes()).expect("parses");
        let lowered = lower(&ast);
        let rejected = match lowered {
            Err(_) => true,
            Ok(unit) => !check(&unit).is_empty(),
        };
        assert!(rejected, "Orpheus bypass was accepted: {exploit}");
    }
}

#[test]
fn unmatched_split_shr_terminates_without_hanging_or_panicking() {
    for bad in [
        "Answer<I64>>",
        "Answer<I64>> x = 1;",
        "typedef Answer<I64>> T;",
        "struct S { Answer<I64>> f; };",
        "U0 f(Answer<I64>> x) {}",
    ] {
        assert!(parse(bad.as_bytes()).is_err(), "expected parse fault for: {bad}");
    }
}

#[test]
fn recursive_function_cannot_self_fulfill_forged_latent_depth() {
    let src = "I64 f(I64 n) @3 { if (n <= 0) { return 0; } f(n - 1) }\n";
    let unit = ir(src);
    assert_eq!(unit.funcs[0].latent, Held::NONE);
    assert!(!check(&unit).is_empty(), "forged @3 latent annotation must fail check");
}

#[test]
fn lower_and_check_agree_on_assign_call_order_compound_assign_and_subexpr_pc() {
    // Evaluation order in Assign and Call must match between lower and check.
    for valid in [
        r#"U0 f() { I64[2] arr; I64 i = 0; arr[i] = { i = descend disk { len(must(read("a"))) }; 1 }; }"#,
        r#"U0 f() { I64[2] arr; I64 i = 0; arr[{ i = descend disk { len(must(read("a"))) }; 0 }] = i; }"#,
        r#"I64 id(I64 x) { x } U0 f() { I64 i = 0; ({ i = descend disk { len(must(read("a"))) }; id })(i); }"#,
        "U0 f() { I64[2] arr; I64 i = 0; arr[{ i = i + 1; 0 }] += 1; }",
    ] {
        let unit = ir(valid);
        clean(&unit);
    }

    // Early returns inside subexpressions must taint subsequent writes via pc.
    for tainted in [
        r#"U0 f() { I64@0 x = 0; Bool d = descend disk { exists("f") }; if ({ if (d) { return; } true }) { x = 1; } }"#,
        r#"U0 f() { I64@0 x = 0; Bool d = descend disk { exists("f") }; x = { if (d) { return; } 1 }; }"#,
        r#"U0 f() { I64@0 x = 0; Bool d = descend disk { exists("f") }; I64 t = ({ if (d) { return; } 1 }) + ({ x = 1; 2 }); }"#,
    ] {
        let unit = ir(tainted);
        assert!(!check(&unit).is_empty(), "implicit flow bypassed @0 assertion: {tainted}");
    }

    // Naming through short-circuit Select must freeze the local.
    let frozen_src = "U0 f() { Bool b = true; seal (true && b); b = false; }\n";
    let frozen_unit = ir(frozen_src);
    let faults = check(&frozen_unit);
    assert!(
        faults.iter().any(|f| matches!(f.kind, nether_core::FaultKind::Frozen { .. })),
        "expected Frozen fault through Select: {faults:?}"
    );

    // Arity on polymorphic prelude primitives, unused cyclic typedefs, and
    // global/function name collisions must fail lowering.
    for bad in [
        "U0 f() { Answer<I64> a = refused(); }\n",
        "U0 f() { Answer<I64> a = refused(absent, 1); }\n",
        "U0 f() { Answer<I64> a = answered(1); I64 x = must(a, 2); }\n",
        "typedef A B; typedef B A;\n",
        "I64 foo = 1; I64 foo() { 2 }\n",
    ] {
        let ast = parse(bad.as_bytes()).unwrap_or_else(|e| panic!("should parse: {bad}\n{e:?}"));
        assert!(lower(&ast).is_err(), "expected lowering error for: {bad}");
    }
}

#[test]
fn pass3_orpheus_container_and_struct_bypasses_and_syntax_hardening() {
    // 1. Nominal struct, container, and empty-body Shade bypasses must fail check().
    for bypass in [
        r#"
struct S8 { Shade<Bytes@8> s; };
struct S0 { Shade<Bytes@0> s; };
Bytes@0 steal() {
    S8 a;
    a.s = descend unrecorded { shade must(call_foreign("x", b"")) };
    S0 b = a;
    look b.s
}
"#,
        r#"
Bytes@0 steal() {
    Answer<Shade<Bytes@8>> a = answered(descend unrecorded { shade must(call_foreign("x", b"")) });
    Shade<Bytes@0>[] arr = a;
    look arr[0]
}
"#,
        "Shade<Bytes@0> f() {}\nBytes@0 steal() { look f() }\n",
        "typedef Shade<Bytes@5> NetShade;\nBytes@0 f() { NetShade s = shade b\"pure\"; look s }\n",
        "typedef I64@3 DeepInt;\nU0 f() { DeepInt x = 1; }\n",
    ] {
        let unit = ir(bypass);
        assert!(!check(&unit).is_empty(), "bypass unexpectedly checked clean:\n{bypass}");
    }

    // 2. Indexed compound assignment prints valid re-parseable source (§6.5).
    let comp = ir("U0 f() { I64[2] arr; I64 i = 0; arr[{ i = i + 1; 0 }] += 1; }\n");
    clean(&comp);
    let printed = nether_syntax::print(&comp);
    let reparsed = parse(printed.as_bytes()).expect("printed compound assignment must re-parse");
    let relowered = lower(&reparsed).expect("printed compound assignment must re-lower");
    clean(&relowered);

    // 3. Wrapped local Shade with omitted origin infers origin cleanly, and prelude type shadowing works (§3.4).
    for ok in [
        "U0 f() { Answer<Shade<Bytes>> a = answered(descend disk { shade must(read(\"f\")) }); }\n",
        "struct I64 { Bytes b; };\nBytes f(I64 x) { x.b }\n",
    ] {
        clean(&ir(ok));
    }

    // 4. Call freezes earlier local argument before later argument mutates it (§5.4).
    let arg_freeze = ir("U0 f(I64 a, U0 b) {}\nU0 g() { I64 n = 1; f(n, { n = 2; }); }\n");
    assert!(
        check(&arg_freeze).iter().any(|f| matches!(f.kind, nether_core::FaultKind::Frozen { .. })),
        "later call argument must not mutate earlier passed local"
    );

    // 5. Received shades without `@d` (§4.3), bare polymorphic prelude names, and wrong arity fail lowering.
    for bad_lower in [
        "struct H { Shade<Bytes> s; };\n",
        "Cairn keep(Shade<Bytes> s) { seal s }\n",
        "Shade<Bytes> make() { shade b\"a\" }\n",
        "U0 f() { ({ refused })(1, 2, 3); }\n",
        "U0 f() { ({ must })(123); }\n",
        "U0 f() { descend disk { read(); } }\n",
        "I64 f() { min(1) }\n",
    ] {
        let ast =
            parse(bad_lower.as_bytes()).unwrap_or_else(|e| panic!("parse: {bad_lower}\n{e:?}"));
        assert!(lower(&ast).is_err(), "expected lowering error for: {bad_lower}");
    }
}

#[test]
fn pass4_core_prim_control_dag_field_assert_and_syntax_hardening() {
    use nether_core::{Depth, Expr, ExprKind, Held, Prim, Span, Type, Unit};

    let sp = Span::default();
    let shade0 = Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Bytes) };
    let shade5 = Type::Shade { origin: Depth::new(5).unwrap(), inner: Box::new(Type::Bytes) };

    // 1. Monomorphic Prim::Len cannot forge Shade<Bytes@0> from Shade<Bytes@5>.
    let bad_prim = Unit {
        structs: vec![],
        globals: vec![],
        locals: vec![],
        funcs: vec![],
        demands: vec![nether_core::Demand {
            value: Expr {
                kind: ExprKind::Prim(Prim::Len),
                ty: Type::Fn {
                    params: vec![shade5],
                    latent: Held::NONE,
                    result: Box::new(shade0.clone()),
                    result_depth: Depth::PURE,
                },
                depth: Depth::PURE,
                span: sp,
            },
            span: sp,
        }],
    };
    assert!(!check(&bad_prim).is_empty(), "monomorphic Prim must not forge Shade");

    // 2. Loop / Break / Continue / Return cannot forge Shade<Bytes@0> or run outside valid scopes.
    let bad_loop = Unit {
        structs: vec![],
        globals: vec![],
        locals: vec![],
        funcs: vec![],
        demands: vec![nether_core::Demand {
            value: Expr {
                kind: ExprKind::Loop {
                    body: Box::new(Expr {
                        kind: ExprKind::Break,
                        ty: Type::Unit,
                        depth: Depth::PURE,
                        span: sp,
                    }),
                    step: None,
                },
                ty: shade0.clone(),
                depth: Depth::PURE,
                span: sp,
            },
            span: sp,
        }],
    };
    assert!(!check(&bad_loop).is_empty(), "Loop must not fabricate Shade");

    let bad_top_return = Unit {
        structs: vec![],
        globals: vec![],
        locals: vec![],
        funcs: vec![],
        demands: vec![nether_core::Demand {
            value: Expr {
                kind: ExprKind::Return(None),
                ty: Type::Unit,
                depth: Depth::PURE,
                span: sp,
            },
            span: sp,
        }],
    };
    assert!(!check(&bad_top_return).is_empty(), "Return outside a function must be rejected");
}

#[test]
fn pass4_break_before_return_and_dag_and_field_assert() {
    use nether_core::{Block, Depth, Expr, ExprKind, FuncDef, Held, Span, Stmt, Type, Unit};
    use std::fmt::Write as _;

    let sp = Span::default();
    let shade0 = Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Bytes) };

    let bad_break_before_ret = Unit {
        structs: vec![],
        globals: vec![],
        locals: vec![],
        funcs: vec![FuncDef {
            name: "f".into(),
            params: vec![],
            ret: shade0.clone(),
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Held::NONE,
            asserted_latent: None,
            locals: vec![],
            body: Block {
                stmts: vec![
                    Stmt::Expr(Expr {
                        kind: ExprKind::Break,
                        ty: Type::Unit,
                        depth: Depth::PURE,
                        span: sp,
                    }),
                    Stmt::Expr(Expr {
                        kind: ExprKind::Return(Some(Box::new(Expr {
                            kind: ExprKind::Rite {
                                rite: nether_core::Rite::Shade,
                                operand: Box::new(Expr {
                                    kind: ExprKind::Literal(nether_core::Literal::Bytes(vec![])),
                                    ty: Type::Bytes,
                                    depth: Depth::PURE,
                                    span: sp,
                                }),
                            },
                            ty: shade0,
                            depth: Depth::PURE,
                            span: sp,
                        }))),
                        ty: Type::Unit,
                        depth: Depth::PURE,
                        span: sp,
                    }),
                ],
                tail: None,
                span: sp,
            },
            span: sp,
        }],
        demands: vec![],
    };
    assert!(
        !check(&bad_break_before_ret).is_empty(),
        "Break outside loop / before Return must be rejected"
    );

    // 4. Acyclic diamond struct DAG (30 levels) must check in linear time without hanging.
    let mut dag = String::from("struct S0 { I64 a; };\n");
    for i in 1..=30 {
        let _ = writeln!(dag, "struct S{i} {{ S{} a; S{} b; }};", i - 1, i - 1);
    }
    dag.push_str("U0 f() { S30 x; x; }\n");
    clean(&ir(&dag));

    // 5. Struct Field::asserted depth annotations are checked on field assignment.
    let field_assert = ir(
        "struct Header { Bytes@0 tag; };\nU0 f() { Header h; h.tag = descend disk { must(read(\"s\")) }; }\n",
    );
    assert!(
        check(&field_assert)
            .iter()
            .any(|f| matches!(f.kind, nether_core::FaultKind::Asserted { .. })),
        "struct field @0 assertion must reject depth-3 assignment"
    );

    // 6. Long flat binary, postfix, and array-dimension chains hit TooDeep instead of stack overflow.
    let mut long_add = String::from("I64 x = 1");
    for _ in 0..300 {
        long_add.push_str(" + 1");
    }
    long_add.push_str(";\n");
    assert!(parse(long_add.as_bytes()).is_err(), "long binary chain must hit TooDeep");

    let mut long_arr = String::from("I64");
    for _ in 0..300 {
        long_arr.push_str("[]");
    }
    long_arr.push_str(" x;\n");
    assert!(parse(long_arr.as_bytes()).is_err(), "long array dimension chain must hit TooDeep");
}

#[test]
fn pass4_typedef_short_circuit_print_and_refused_shade() {
    use nether_core::{Block, Depth, Expr, ExprKind, FuncDef, Held, Span, Stmt, Type, Unit};

    let sp = Span::default();

    // 7. Typedef asserted depths apply on returns and Shade<Typedef>, and conflicts fault.
    let td_ret =
        ir("typedef Bytes@0 PureBytes;\nPureBytes leak() { descend disk { must(read(\"s\")) } }\n");
    assert!(
        check(&td_ret).iter().any(|f| matches!(f.kind, nether_core::FaultKind::Asserted { .. })),
        "typedef @0 must be enforced on function return"
    );
    clean(&ir(
        "typedef Bytes@3 DiskBytes;\nU0 f(Shade<DiskBytes> s) { descend disk { look s; } }\n",
    ));
    let td_shade_bad =
        ir("typedef Bytes@3 DiskBytes;\nU0 g() { Shade<DiskBytes> s = shade b\"p\"; }\n");
    assert!(!check(&td_shade_bad).is_empty(), "Shade<DiskBytes> must keep @3 origin");
    let td_conflict = parse(
        b"typedef Bytes@0 PureBytes;\nU0 f() { PureBytes@3 x = descend disk { must(read(\"s\")) }; }\n",
    )
    .expect("parses");
    assert!(lower(&td_conflict).is_err(), "conflicting typedef @0 and use @3 must fail lowering");

    // 8. `print::short_circuit` must not emit `«a branch outside a statement»` on non-bool `if`/`else if`.
    let sel_unit = Unit {
        structs: vec![],
        globals: vec![],
        locals: vec![],
        funcs: vec![FuncDef {
            name: "f".into(),
            params: vec![],
            ret: Type::Unit,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Held::NONE,
            asserted_latent: None,
            locals: vec![],
            body: Block {
                stmts: vec![Stmt::Expr(Expr {
                    kind: ExprKind::Select {
                        cond: Box::new(Expr {
                            kind: ExprKind::Literal(nether_core::Literal::Bool(true)),
                            ty: Type::Bool,
                            depth: Depth::PURE,
                            span: sp,
                        }),
                        then: Box::new(Expr {
                            kind: ExprKind::Literal(nether_core::Literal::Bool(true)),
                            ty: Type::Bool,
                            depth: Depth::PURE,
                            span: sp,
                        }),
                        otherwise: Box::new(Expr {
                            kind: ExprKind::Select {
                                cond: Box::new(Expr {
                                    kind: ExprKind::Literal(nether_core::Literal::Bool(false)),
                                    ty: Type::Bool,
                                    depth: Depth::PURE,
                                    span: sp,
                                }),
                                then: Box::new(Expr {
                                    kind: ExprKind::Literal(nether_core::Literal::Int(1)),
                                    ty: Type::Int,
                                    depth: Depth::PURE,
                                    span: sp,
                                }),
                                otherwise: Box::new(Expr {
                                    kind: ExprKind::Literal(nether_core::Literal::Int(2)),
                                    ty: Type::Int,
                                    depth: Depth::PURE,
                                    span: sp,
                                }),
                            },
                            ty: Type::Int,
                            depth: Depth::PURE,
                            span: sp,
                        }),
                    },
                    ty: Type::Unit,
                    depth: Depth::PURE,
                    span: sp,
                })],
                tail: None,
                span: sp,
            },
            span: sp,
        }],
        demands: vec![],
    };
    let printed_sel = nether_syntax::print(&sel_unit);
    assert!(
        !printed_sel.contains("«a branch outside a statement»"),
        "must not misprint non-bool Select as ||: {printed_sel}"
    );
    assert!(parse(printed_sel.as_bytes()).is_ok(), "printed Select must parse: {printed_sel}");

    // 9. `Answer<Shade<Bytes>> x = refused(absent);` must not fabricate @8 origin.
    let bad_refused =
        parse(b"U0 f() { Answer<Shade<Bytes>> x = refused(absent); }\n").expect("parses");
    assert!(
        lower(&bad_refused).is_err(),
        "refused(absent) must reject omitted Shade<T> origin instead of fabricating @8"
    );
}
