//! The stage one proof: `build.nc` buries to depth 3 with exactly one hole.
//!
//! `spec/06-evaluation.md` §6.6 prints it:
//!
//! ```text
//! buried   build.nc → 4c02ab7f   depth 3   holes 1   903 nodes
//!   hole ①  read("main.nc")              stratum 3  disk
//! ```
//!
//! The shape is pinned and the counts are not. Depth 3 and one hole are facts
//! about the program; nine hundred nodes is a fact about whatever `compile`
//! happens to be, and an ordinary refactor should not have to argue with it.
//!
//! There is no parser yet, so the unit is built by hand — but against the real
//! source text, so the span the hole carries is the span `read("main.nc")`
//! actually occupies in the file it came from.

use nether_bury::{Residue, bury};
use nether_core::{
    Block, Capability, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, GlobalDef, GlobalId,
    Literal, LocalDef, LocalId, Prim, Span, Type, Unit, check,
};
use nether_ledger::{Cairn, Node, Value};

/// `build.nc`, as §6.2 writes it, with the `compile` it says a program has to
/// supply for itself.
const BUILD_NC: &str = r#"Bytes compile(Bytes s) { concat(b"obj:", s) }

Bytes@3 src    = must(descend disk { read("main.nc") });
Bytes   obj    = compile(src);
Bytes   other  = b"nothing";
Bytes   unused = compile(other);

demand obj;
"#;

fn span_of(what: &str) -> Span {
    let start = BUILD_NC.find(what).expect("build.nc no longer contains that");
    let start = u32::try_from(start).unwrap();
    Span { start, end: start + u32::try_from(what.len()).unwrap() }
}

// ── building ────────────────────────────────────────────────────────────────

fn e(kind: ExprKind, ty: Type, depth: Depth) -> Expr {
    Expr { kind, ty, depth, span: Span::default() }
}

fn pure(kind: ExprKind, ty: Type) -> Expr {
    e(kind, ty, Depth::PURE)
}

fn at(kind: ExprKind, ty: Type, depth: Depth, span: Span) -> Expr {
    Expr { kind, ty, depth, span }
}

fn answer_bytes() -> Type {
    Type::Answer(Box::new(Type::Bytes))
}

fn prim(p: Prim, params: Vec<Type>, result: Type) -> Expr {
    pure(
        ExprKind::Prim(p),
        Type::Fn { params, latent: p.latent(), result: Box::new(result), result_depth: p.latent() },
    )
}

fn call(callee: Expr, args: Vec<Expr>, result: Type, depth: Depth) -> Expr {
    e(ExprKind::Call { callee: Box::new(callee), args }, result, depth)
}

fn bytes(b: &[u8]) -> Expr {
    pure(ExprKind::Literal(Literal::Bytes(b.to_vec())), Type::Bytes)
}

fn global(id: u32, ty: Type, depth: Depth) -> Expr {
    e(ExprKind::Global(GlobalId(id)), ty, depth)
}

fn binding(name: &str, ty: Type, value: Expr) -> GlobalDef {
    GlobalDef { name: name.into(), ty, asserted: None, value, span: Span::default() }
}

fn compile_arrow() -> Type {
    Type::Fn {
        params: vec![Type::Bytes],
        latent: Depth::PURE,
        result: Box::new(Type::Bytes),
        result_depth: Depth::PURE,
    }
}

/// `Bytes compile(Bytes s) { concat(b"obj:", s) }`
fn compile() -> FuncDef {
    let body = call(
        prim(Prim::Concat, vec![Type::Bytes, Type::Bytes], Type::Bytes),
        vec![bytes(b"obj:"), pure(ExprKind::Local(LocalId(0)), Type::Bytes)],
        Type::Bytes,
        Depth::PURE,
    );
    FuncDef {
        name: "compile".into(),
        params: vec![LocalId(0)],
        ret: Type::Bytes,
        ret_depth: Depth::PURE,
        asserted_ret: None,
        latent: Depth::PURE,
        asserted_latent: None,
        locals: vec![LocalDef {
            name: "s".into(),
            ty: Type::Bytes,
            asserted: None,
            span: Span::default(),
        }],
        body: Block { stmts: Vec::new(), tail: Some(Box::new(body)), span: Span::default() },
        span: Span::default(),
    }
}

/// `must(descend disk { read("main.nc") })`
fn the_read() -> Expr {
    let path = at(
        ExprKind::Literal(Literal::Str("main.nc".into())),
        Type::Str,
        Depth::PURE,
        span_of(r#""main.nc""#),
    );
    let read = at(
        ExprKind::Call {
            callee: Box::new(prim(Prim::Read, vec![Type::Str], answer_bytes())),
            args: vec![path],
        },
        answer_bytes(),
        Depth::DISK,
        span_of(r#"read("main.nc")"#),
    );
    let descent = e(
        ExprKind::Descend { capability: Capability::Disk, body: Box::new(read) },
        answer_bytes(),
        Depth::DISK,
    );
    call(
        prim(Prim::Must, vec![answer_bytes()], Type::Bytes),
        vec![descent],
        Type::Bytes,
        Depth::DISK,
    )
}

fn build_nc(unused: Expr) -> Unit {
    let compiling = |arg: Expr, depth: Depth| {
        call(pure(ExprKind::Func(FuncId(0)), compile_arrow()), vec![arg], Type::Bytes, depth)
    };
    Unit {
        funcs: vec![compile()],
        globals: vec![
            binding("src", Type::Bytes, the_read()),
            binding(
                "obj",
                Type::Bytes,
                compiling(global(0, Type::Bytes, Depth::DISK), Depth::DISK),
            ),
            binding("other", Type::Bytes, bytes(b"nothing")),
            binding("unused", Type::Bytes, unused),
        ],
        demands: vec![Demand { value: global(1, Type::Bytes, Depth::DISK), span: Span::default() }],
        ..Unit::default()
    }
}

fn buried(unit: &Unit) -> Residue {
    let faults = check(unit);
    assert!(faults.is_empty(), "build.nc does not check: {faults:?}");
    bury(unit, Cairn::of_encoded(BUILD_NC.as_bytes()), 1_000_000).expect("build.nc buries")
}

// ── the proof ───────────────────────────────────────────────────────────────

/// `compile(other)` — the binding nothing demands.
fn compiling_other() -> Expr {
    let other = global(2, Type::Bytes, Depth::PURE);
    call(pure(ExprKind::Func(FuncId(0)), compile_arrow()), vec![other], Type::Bytes, Depth::PURE)
}

#[test]
fn build_nc_buries_to_depth_three_with_one_hole() {
    let r = buried(&build_nc(compiling_other()));

    assert_eq!(r.depth, Depth::DISK, "depth 3");
    assert_eq!(r.holes.len(), 1, "holes 1: {:?}", r.questions());

    let [Node::Hole { call, stratum, span, .. }] = r.questions()[..] else { panic!() };
    assert_eq!(call.function, "read");
    assert_eq!(*stratum, 3);
    assert_eq!(r.value(call.args[0]), Some(&Value::Str("main.nc".into())));

    // The span is the one `read("main.nc")` occupies in build.nc, named
    // against the cairn of build.nc rather than against a path.
    assert_eq!(span.source, Cairn::of_encoded(BUILD_NC.as_bytes()));
    let want = span_of(r#"read("main.nc")"#);
    assert_eq!((span.start, span.end), (u64::from(want.start), u64::from(want.end)));
}

#[test]
fn the_demand_starves_on_the_hole_and_stays_a_program() {
    // §6.2's comment on the line: `compile(src)` is pure, and starves, because
    // `src` is a hole. What comes back is the call it was written as.
    let r = buried(&build_nc(compiling_other()));

    assert_eq!(r.demands.len(), 1);
    let ExprKind::Call { callee, .. } = &r.demands[0].kind else {
        panic!("the demand reduced, and it should not have: {:?}", r.demands[0])
    };
    assert_eq!(callee.kind, ExprKind::Func(FuncId(0)));
    assert_eq!(r.demands[0].depth, Depth::DISK);
}

#[test]
fn nothing_demands_unused_and_nothing_evaluates_it() {
    // The only way to prove a thing was not evaluated is to make evaluating it
    // impossible to miss. `unused` divides by zero here; §6.2 makes the burial
    // finishing anyway a semantic guarantee rather than an optimisation.
    let dividing = pure(
        ExprKind::Binary {
            op: nether_core::BinOp::Div,
            lhs: Box::new(pure(ExprKind::Literal(Literal::Int(1)), Type::Int)),
            rhs: Box::new(pure(ExprKind::Literal(Literal::Int(0)), Type::Int)),
        },
        Type::Bytes,
    );
    let r = buried(&build_nc(dividing));
    assert_eq!(r.holes.len(), 1);
    assert_eq!(r.depth, Depth::DISK);
}

#[test]
fn reading_the_same_file_twice_is_still_one_hole() {
    // Two demands, both reaching the same read through `obj`. §6.3: two holes
    // with identical calls in one trace are the same hole.
    let mut unit = build_nc(compiling_other());
    unit.demands.push(Demand { value: global(0, Type::Bytes, Depth::DISK), span: Span::default() });
    let r = buried(&unit);
    assert_eq!(r.holes.len(), 1);
}

#[test]
fn the_summary_line_is_the_one_the_codex_prints() {
    // Not the cairn and not the node count: those are facts about a `compile`
    // this file does not have. The two that are facts about the program are
    // the two that are checked.
    let r = buried(&build_nc(compiling_other()));
    let summary = format!("depth {}   holes {}", r.depth, r.holes.len());
    assert_eq!(summary, "depth 3   holes 1");
}
