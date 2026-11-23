//! Burial: what reduces, what stays, and what stops.
//!
//! Every unit here is checked before it is buried. A burial of something the
//! calculus rejects proves nothing about burial.

use nether_bury::{Halt, HaltKind, Residue};
use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, GlobalDef, Held,
    Literal, LocalDef, LocalId, Prim, Rite, Span, Stmt, Type, Unit, check,
};
use nether_ledger::Cairn;

// ── building ────────────────────────────────────────────────────────────────

fn e(kind: ExprKind, ty: Type, depth: Depth) -> Expr {
    Expr { kind, ty, depth, span: Span::default() }
}

fn pure(kind: ExprKind, ty: Type) -> Expr {
    e(kind, ty, Depth::PURE)
}

fn int(n: i64) -> Expr {
    pure(ExprKind::Literal(Literal::Int(n)), Type::Int)
}

fn add(op: BinOp, lhs: Expr, rhs: Expr, ty: Type) -> Expr {
    let depth = lhs.depth.join(rhs.depth);
    e(ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, ty, depth)
}

fn local(id: u32, ty: Type, depth: Depth) -> Expr {
    e(ExprKind::Local(LocalId(id)), ty, depth)
}

fn answer_bytes() -> Type {
    Type::Answer(Box::new(Type::Bytes))
}

fn prim(p: Prim, params: Vec<Type>, result: Type) -> Expr {
    pure(
        ExprKind::Prim(p),
        Type::Fn {
            params,
            latent: Held::of(p.latent()),
            result: Box::new(result),
            result_depth: p.latent(),
        },
    )
}

fn call(callee: Expr, args: Vec<Expr>, result: Type, depth: Depth) -> Expr {
    e(ExprKind::Call { callee: Box::new(callee), args }, result, depth)
}

/// `descend disk { must(read(path)) }` — deep, and nothing can answer it.
fn a_file(path: &str) -> Expr {
    let read = call(
        prim(Prim::Read, vec![Type::Str], answer_bytes()),
        vec![pure(ExprKind::Literal(Literal::Str(path.into())), Type::Str)],
        answer_bytes(),
        Depth::DISK,
    );
    let must = call(
        prim(Prim::Must, vec![answer_bytes()], Type::Bytes),
        vec![read],
        Type::Bytes,
        Depth::DISK,
    );
    e(
        ExprKind::Descend { capability: Capability::Disk, body: Box::new(must) },
        Type::Bytes,
        Depth::DISK,
    )
}

fn demanding(value: Expr) -> Unit {
    Unit { demands: vec![Demand { value, span: Span::default() }], ..Unit::default() }
}

/// Check, then bury. A burial of something the calculus rejects proves nothing.
fn buried(unit: &Unit, fuel: u64) -> Residue {
    let faults = check(unit);
    assert!(faults.is_empty(), "the unit does not check: {faults:?}");
    bury(unit, fuel).expect("this was supposed to finish")
}

fn only(r: &Residue) -> &Expr {
    assert_eq!(r.demands.len(), 1);
    &r.demands[0]
}

// ── what reduces ────────────────────────────────────────────────────────────

#[test]
fn pure_arithmetic_disappears() {
    // `demand 2 + 3 * 4;`
    let unit =
        demanding(add(BinOp::Add, int(2), add(BinOp::Mul, int(3), int(4), Type::Int), Type::Int));
    let r = buried(&unit, 1000);
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(14)));
    assert_eq!(r.depth, Depth::PURE);
}

#[test]
fn fib_of_twenty_burns_to_a_literal() {
    let unit = fib_demanding(20);
    let r = buried(&unit, 5_000_000);
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(6765)));
    assert!(!contains_a_call(only(&r)), "the residue still has a call in it");
}

#[test]
fn a_loop_unrolls_when_everything_in_it_is_here() {
    // `I64 total = 0; for (I64 i = 0; ; i += 1) { if (i >= 10) { break; } total += i; } total`
    let unit = summing(10, false);
    let r = buried(&unit, 100_000);
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(45)));
}

#[test]
fn continue_still_runs_the_step() {
    // The same loop with a `continue` before the accumulation, so the sum is
    // zero and the counter still gets there. A `continue` that skipped the
    // step would not terminate at all.
    let unit = summing(10, true);
    let r = buried(&unit, 100_000);
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(0)));
}

#[test]
fn a_shade_is_carried_through_burial_and_look_gets_it_back() {
    let shaded = pure(
        ExprKind::Rite { rite: Rite::Shade, operand: Box::new(int(7)) },
        Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Int) },
    );
    let looked = pure(ExprKind::Rite { rite: Rite::Look, operand: Box::new(shaded) }, Type::Int);
    let r = buried(&demanding(looked), 1000);
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(7)));
}

// ── what stays ──────────────────────────────────────────────────────────────

#[test]
fn a_question_for_the_world_stays_the_question() {
    let r = buried(&demanding(a_file("kernel.nc")), 1000);
    assert!(matches!(only(&r).kind, ExprKind::Descend { .. }));
    assert_eq!(r.depth, Depth::DISK);
}

#[test]
fn the_pure_part_of_a_deep_expression_is_still_reduced() {
    // `len(must(read("k"))) + (2 * 3)` — the right-hand side is arithmetic and
    // goes; the left-hand side is a question and stays.
    let length = call(
        prim(Prim::Len, vec![Type::Bytes], Type::Int),
        vec![a_file("k")],
        Type::Int,
        Depth::DISK,
    );
    let sum = add(BinOp::Add, length, add(BinOp::Mul, int(2), int(3), Type::Int), Type::Int);
    let r = buried(&demanding(sum), 1000);

    let ExprKind::Binary { rhs, .. } = &only(&r).kind else { panic!("{:?}", only(&r)) };
    assert_eq!(rhs.kind, ExprKind::Literal(Literal::Int(6)));
    assert_eq!(only(&r).depth, Depth::DISK);
}

#[test]
fn opaque_is_never_burned_through() {
    // It could be reduced. That is the point: `opaque` is how a programmer
    // says do not, even though you could. `spec/06-evaluation.md` §6.4.
    let inner = add(BinOp::Add, int(2), int(3), Type::Int);
    let barrier = pure(ExprKind::Rite { rite: Rite::Opaque, operand: Box::new(inner) }, Type::Int);
    let r = buried(&demanding(barrier), 1000);
    let ExprKind::Rite { rite: Rite::Opaque, operand } = &only(&r).kind else {
        panic!("burned through: {:?}", only(&r))
    };
    assert!(matches!(operand.kind, ExprKind::Binary { .. }));
}

#[test]
fn a_branch_on_something_that_is_not_here_keeps_both_arms() {
    // Evaluating the arm that will not be taken is what §6.2 forbids, and
    // which arm that is has not been decided yet.
    let cond = add(BinOp::Gt, a_length(), int(0), Type::Bool);
    let select = e(
        ExprKind::Select {
            cond: Box::new(cond),
            then: Box::new(int(1)),
            otherwise: Box::new(int(2)),
        },
        Type::Int,
        Depth::DISK,
    );
    let r = buried(&demanding(select), 1000);
    let ExprKind::Select { then, otherwise, .. } = &only(&r).kind else { panic!("{:?}", only(&r)) };
    assert_eq!(then.kind, ExprKind::Literal(Literal::Int(1)));
    assert_eq!(otherwise.kind, ExprKind::Literal(Literal::Int(2)));
}

#[test]
fn a_loop_that_cannot_be_unrolled_stays_a_loop() {
    let guard = e(
        ExprKind::Select {
            cond: Box::new(add(BinOp::Gt, a_length(), int(0), Type::Bool)),
            then: Box::new(pure(ExprKind::Break, Type::Unit)),
            otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
        },
        Type::Unit,
        Depth::DISK,
    );
    let looping = e(ExprKind::Loop { body: Box::new(guard), step: None }, Type::Unit, Depth::DISK);
    let r = buried(&demanding(looping), 1000);
    assert!(matches!(only(&r).kind, ExprKind::Loop { .. }));
}

// ── what is never touched ───────────────────────────────────────────────────

#[test]
fn nothing_a_demand_does_not_reach_is_evaluated() {
    // The unreached binding divides by zero. Evaluating it would stop the
    // burial, so the burial finishing is the proof that it was not evaluated.
    // §6.2 makes this a guarantee rather than an optimisation.
    let unit = Unit {
        globals: vec![
            GlobalDef {
                name: "wanted".into(),
                ty: Type::Int,
                asserted: None,
                value: int(1),
                span: Span::default(),
            },
            GlobalDef {
                name: "unwanted".into(),
                ty: Type::Int,
                asserted: None,
                value: add(BinOp::Div, int(1), int(0), Type::Int),
                span: Span::default(),
            },
        ],
        demands: vec![Demand {
            value: pure(ExprKind::Global(nether_core::GlobalId(0)), Type::Int),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    let r = bury(&unit, 1000).expect("the unwanted binding was evaluated");
    assert_eq!(only(&r).kind, ExprKind::Literal(Literal::Int(1)));
}

// ── what stops ──────────────────────────────────────────────────────────────

#[test]
fn dividing_by_zero_collapses() {
    let unit = demanding(add(BinOp::Div, int(1), int(0), Type::Int));
    let halt = bury(&unit, 1000).expect_err("this cannot produce a value");
    assert_eq!(halt.kind, HaltKind::Collapsed("this divides by zero"));
}

#[test]
fn an_out_of_range_slice_collapses() {
    // §9.9 names this one. A collapse is not catchable and there is no rescue
    // form: every way to collapse is a mistake in the program.
    let s = |t: &str| pure(ExprKind::Literal(Literal::Str(t.into())), Type::Str);
    let sliced = call(
        prim(Prim::Slice, vec![Type::Bytes, Type::Int, Type::Int], Type::Bytes),
        vec![s("nc"), int(0), int(99)],
        Type::Bytes,
        Depth::PURE,
    );
    let halt = bury(&demanding(sliced), 1000).expect_err("there is no such slice");
    assert_eq!(halt.kind, HaltKind::Collapsed("this slice is outside what it is slicing"));
}

#[test]
fn running_out_of_fuel_is_a_diagnostic_and_not_a_crash() {
    let halt = bury(&fib_demanding(20), 100).expect_err("100 steps is not enough for fib(20)");
    assert!(matches!(halt.kind, HaltKind::OutOfFuel { spent: 100 }));
}

#[test]
fn fuel_accounting_is_the_same_on_every_run() {
    // §6.4: the same source and the same budget must exhaust at exactly the
    // same point, on every implementation and every machine.
    let unit = fib_demanding(12);
    let spent = bury(&unit, 5_000_000).unwrap().fuel_spent;
    for _ in 0..4 {
        assert_eq!(bury(&unit, 5_000_000).unwrap().fuel_spent, spent);
    }
    // And one step short of it is one step short.
    assert!(matches!(bury(&unit, spent - 1).unwrap_err().kind, HaltKind::OutOfFuel { .. }));
    assert_eq!(bury(&unit, spent).unwrap().fuel_spent, spent);
}

// ── the programs ────────────────────────────────────────────────────────────

fn a_length() -> Expr {
    call(prim(Prim::Len, vec![Type::Bytes], Type::Int), vec![a_file("k")], Type::Int, Depth::DISK)
}

fn contains_a_call(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Call { .. } => true,
        ExprKind::Binary { lhs, rhs, .. } => contains_a_call(lhs) || contains_a_call(rhs),
        ExprKind::Unary { operand, .. } | ExprKind::Rite { operand, .. } => {
            contains_a_call(operand)
        }
        _ => false,
    }
}

/// ```c
/// I64 fib(I64 n)
/// {
///   if (n < 2) { return n; }
///   return fib(n - 1) + fib(n - 2);
/// }
///
/// demand fib(k);
/// ```
fn fib_demanding(k: i64) -> Unit {
    let arrow = Type::Fn {
        params: vec![Type::Int],
        latent: Held::NONE,
        result: Box::new(Type::Int),
        result_depth: Depth::PURE,
    };
    let n = || local(0, Type::Int, Depth::PURE);
    let me = || pure(ExprKind::Func(FuncId(0)), arrow.clone());
    let recurse = |less: i64| {
        call(me(), vec![add(BinOp::Sub, n(), int(less), Type::Int)], Type::Int, Depth::PURE)
    };

    let base = pure(
        ExprKind::Block(Block {
            stmts: vec![Stmt::Expr(pure(ExprKind::Return(Some(Box::new(n()))), Type::Unit))],
            tail: None,
            span: Span::default(),
        }),
        Type::Unit,
    );
    let guard = pure(
        ExprKind::Select {
            cond: Box::new(add(BinOp::Lt, n(), int(2), Type::Bool)),
            then: Box::new(base),
            otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
        },
        Type::Unit,
    );
    let step = pure(
        ExprKind::Return(Some(Box::new(add(BinOp::Add, recurse(1), recurse(2), Type::Int)))),
        Type::Unit,
    );

    Unit {
        funcs: vec![FuncDef {
            name: "fib".into(),
            params: vec![LocalId(0)],
            ret: Type::Int,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Held::NONE,
            asserted_latent: None,
            locals: vec![LocalDef {
                name: "n".into(),
                ty: Type::Int,
                asserted: None,
                span: Span::default(),
            }],
            body: Block {
                stmts: vec![Stmt::Expr(guard), Stmt::Expr(step)],
                tail: None,
                span: Span::default(),
            },
            span: Span::default(),
        }],
        demands: vec![Demand {
            value: call(me(), vec![int(k)], Type::Int, Depth::PURE),
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

/// ```c
/// I64 sum()
/// {
///   I64 total = 0;
///   for (I64 i = 0; ; i += 1) {
///     if (i >= n) { break; }
///     [continue;]
///     total += i;
///   }
///   total
/// }
/// ```
fn summing(n: i64, skip: bool) -> Unit {
    let total = || local(0, Type::Int, Depth::PURE);
    let i = || local(1, Type::Int, Depth::PURE);

    let mut body = vec![Stmt::Expr(pure(
        ExprKind::Select {
            cond: Box::new(add(BinOp::Ge, i(), int(n), Type::Bool)),
            then: Box::new(pure(ExprKind::Break, Type::Unit)),
            otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
        },
        Type::Unit,
    ))];
    if skip {
        body.push(Stmt::Expr(pure(ExprKind::Continue, Type::Unit)));
    }
    body.push(Stmt::Expr(pure(
        ExprKind::Assign {
            place: nether_core::Place { local: LocalId(0), path: Vec::new() },
            value: Box::new(add(BinOp::Add, total(), i(), Type::Int)),
        },
        Type::Unit,
    )));

    let stepping = pure(
        ExprKind::Assign {
            place: nether_core::Place { local: LocalId(1), path: Vec::new() },
            value: Box::new(add(BinOp::Add, i(), int(1), Type::Int)),
        },
        Type::Unit,
    );
    let looping = pure(
        ExprKind::Loop {
            body: Box::new(pure(
                ExprKind::Block(Block { stmts: body, tail: None, span: Span::default() }),
                Type::Unit,
            )),
            step: Some(Box::new(stepping)),
        },
        Type::Unit,
    );

    let arrow = Type::Fn {
        params: Vec::new(),
        latent: Held::NONE,
        result: Box::new(Type::Int),
        result_depth: Depth::PURE,
    };
    Unit {
        funcs: vec![FuncDef {
            name: "sum".into(),
            params: Vec::new(),
            ret: Type::Int,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Held::NONE,
            asserted_latent: None,
            locals: vec![
                LocalDef {
                    name: "total".into(),
                    ty: Type::Int,
                    asserted: None,
                    span: Span::default(),
                },
                LocalDef { name: "i".into(), ty: Type::Int, asserted: None, span: Span::default() },
            ],
            body: Block {
                stmts: vec![
                    Stmt::Let { local: LocalId(0), value: int(0) },
                    Stmt::Let { local: LocalId(1), value: int(0) },
                    Stmt::Expr(looping),
                ],
                tail: Some(Box::new(total())),
                span: Span::default(),
            },
            span: Span::default(),
        }],
        demands: vec![Demand {
            value: call(pure(ExprKind::Func(FuncId(0)), arrow), Vec::new(), Type::Int, Depth::PURE),
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

/// The cairn these units pretend to have been lowered from.
fn source_cairn() -> Cairn {
    Cairn::of_encoded(b"a source")
}

/// `bury`, with a source for the spans to belong to.
fn bury(unit: &Unit, fuel: u64) -> Result<Residue, Halt> {
    nether_bury::bury(unit, source_cairn(), fuel)
}

fn parse_and_check(src: &str) -> Unit {
    let parsed = nether_syntax::parse(src.as_bytes()).expect("parse succeeds");
    let unit = nether_syntax::lower(&parsed).expect("lower succeeds");
    let faults = check(&unit);
    assert!(faults.is_empty(), "check faults: {faults:?}");
    unit
}

fn bury_source(src: &str, fuel: u64) -> Result<Residue, Halt> {
    let unit = parse_and_check(src);
    bury(&unit, fuel)
}

#[test]
fn unroll_rolls_back_deposits_when_loop_starves() {
    // Iteration 0 deposits `42;` then starves on `read("a")`, so `unroll` must
    // roll back the speculative deposit instead of leaking it into `res.deposits`.
    let src = r#"
demand {
    I64 i = 0;
    while (i < 1000) {
        42;
        descend disk {
            if (len(must(read("a"))) > 0) {
                i = i + 1;
            }
        }
    }
    i
};
"#;
    let unit = parse_and_check(src);
    let res = bury(&unit, 10_000).expect("buries");
    assert_eq!(res.deposits.len(), 0, "abandoned unroll must not leak deposits");
    assert!(
        matches!(res.demands[0].kind, ExprKind::Block(_) | ExprKind::Loop { .. }),
        "expected residual loop/block, got {:?}",
        res.demands[0].kind
    );
    let faults = check(&res.as_unit(&unit));
    assert!(
        faults.is_empty(),
        "residue faults: {faults:?}\n{}",
        nether_syntax::print(&res.as_unit(&unit))
    );
}

#[test]
fn early_return_short_circuits_enclosing_binary_call_and_assign() {
    let src = r"
I64 id(I64 x) { x }
I64 bin_case() {
    ({ return 7; 0 }) + (1 / 0)
}
I64 call_case() {
    id(({ return 11; 0 })) + (1 / 0)
}
I64 assign_case() {
    I64 x = 0;
    x = ({ return 13; 1 });
    1 / 0
}
demand bin_case() + call_case() + assign_case();
";
    let res = bury_source(src, 10_000).expect("early returns prevent division by zero");
    assert_eq!(res.demands[0].kind, ExprKind::Literal(Literal::Int(31)));
}

#[test]
fn opaque_and_seal_preserve_local_mutations_and_reduced_operands() {
    let src = r"
demand {
    I64 x = 1;
    x = 2;
    opaque x
};
";
    let unit = parse_and_check(src);
    let res = bury(&unit, 10_000).expect("buries");
    let residue_unit = res.as_unit(&unit);
    assert!(check(&residue_unit).is_empty());
    let printed = nether_syntax::print(&residue_unit);
    assert!(
        printed.contains('2'),
        "opaque must settle mutated local binding before leaving it residual: {printed}"
    );
    assert!(
        !printed.contains("I64 x = 1;\n    opaque x"),
        "opaque must not lose the update x = 2: {printed}"
    );

    // Seal on an expression with a pure subexpression must rebuild with the
    // reduced subexpression instead of cloning the unreduced operand.
    let seal_src = r#"
demand descend disk {
    seal concat(b"a", concat(b"b", must(read("p"))))
};
"#;
    let seal_unit = parse_and_check(seal_src);
    let seal_res = bury(&seal_unit, 10_000).expect("buries");
    assert!(check(&seal_res.as_unit(&seal_unit)).is_empty());
}

#[test]
fn shifts_at_or_above_64_collapse_cleanly() {
    let left = bury_source("demand 1 << 64;", 1_000).expect_err("<< 64 collapses");
    assert!(matches!(left.kind, HaltKind::Collapsed(_)));
    let right = bury_source("demand 1 >> 64;", 1_000).expect_err(">> 64 collapses");
    assert!(matches!(right.kind, HaltKind::Collapsed(_)));
    let neg = bury_source("demand 1 << -1;", 1_000).expect_err("<< -1 collapses");
    assert!(matches!(neg.kind, HaltKind::Collapsed(_)));
}

#[test]
fn cairn_of_hex_and_answer_equality_fold_at_burial() {
    let src = r#"
Answer<I64> r1 = refused(absent);
Answer<I64> r2 = refused(absent);
Bool same_given = answered(42) == answered(42);
Bool diff_given = answered(42) != answered(43);
Bool same_refusal = r1 == r2;
Str h = hex(cairn_of(b"hello"));
demand same_given && diff_given && same_refusal && len(raw(h)) == 64;
"#;
    let res = bury_source(src, 10_000).expect("buries");
    assert_eq!(res.demands[0].kind, ExprKind::Literal(Literal::Bool(true)));
}

#[test]
fn cyclic_globals_collapse_cleanly_and_global_starve_does_not_poison_caller() {
    // Two globals referencing each other must collapse instead of recursing forever.
    let cyclic = Unit {
        globals: vec![
            GlobalDef {
                name: "a".into(),
                ty: Type::Int,
                asserted: None,
                value: pure(ExprKind::Global(nether_core::GlobalId(1)), Type::Int),
                span: Span::default(),
            },
            GlobalDef {
                name: "b".into(),
                ty: Type::Int,
                asserted: None,
                value: pure(ExprKind::Global(nether_core::GlobalId(0)), Type::Int),
                span: Span::default(),
            },
        ],
        demands: vec![Demand {
            value: pure(ExprKind::Global(nether_core::GlobalId(0)), Type::Int),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    let err = bury(&cyclic, 1_000).expect_err("cyclic globals must collapse");
    assert!(matches!(err.kind, HaltKind::Collapsed(_)));
}

#[test]
fn answer_reassignment_before_opaque_and_shade_residuals_stay_valid() {
    // Reassigning an Answer local before `opaque` must preserve the update.
    let ans_src = r"
demand {
    Answer<I64> a = answered(1);
    a = answered(2);
    opaque a
};
";
    let ans_unit = parse_and_check(ans_src);
    let ans_res = bury(&ans_unit, 10_000).expect("buries");
    let ans_out = ans_res.as_unit(&ans_unit);
    assert!(check(&ans_out).is_empty(), "residue faults: {:?}", check(&ans_out));
    let printed = nether_syntax::print(&ans_out);
    assert!(printed.contains("answered(2)"), "missing updated Answer assignment: {printed}");

    // `shade(answered(1))` must not double-wrap as `shade(shade(...))`.
    let shade_ans = parse_and_check("demand shade(answered(1));\n");
    let shade_res = bury(&shade_ans, 10_000).expect("buries");
    let shade_out = shade_res.as_unit(&shade_ans);
    assert!(check(&shade_out).is_empty(), "residue faults: {:?}", check(&shade_out));
    let shade_printed = nether_syntax::print(&shade_out);
    assert!(
        !shade_printed.contains("shade(shade(") && !shade_printed.contains("shade shade"),
        "double-wrapped shade: {shade_printed}"
    );

    // `shade(must(read("f")))` answered by the world must stay valid under `check()`.
    let deep_shade_src = r#"
demand descend disk {
    shade(must(read("f")))
};
"#;
    let deep_unit = parse_and_check(deep_shade_src);
    let call = nether_ledger::Call {
        function: "read".into(),
        args: vec![nether_ledger::Value::Str("f".into()).cairn()],
    };
    let answers = nether_bury::Answers::none().and(
        call,
        nether_ledger::Value::Answer(Box::new(nether_ledger::AnswerOf::Given(
            nether_ledger::Value::Bytes(b"hi".to_vec()),
        ))),
    );
    let deep_res = nether_bury::bury_with(
        &deep_unit,
        Cairn::of_encoded(deep_shade_src.as_bytes()),
        10_000,
        &answers,
    )
    .expect("buries with answer");
    let deep_out = deep_res.as_unit(&deep_unit);
    assert!(check(&deep_out).is_empty(), "answered shade faults: {:?}", check(&deep_out));
}

#[test]
fn let_early_return_and_starved_act_global_and_concat_cap() {
    // `let x = ({ return ...; 0 });` inside a function must preserve the early return
    // even when evaluated under speculation.
    let ret_src = r#"
I64 f(Bool b) {
    if (b) {
        I64 x = ({ return 9; 0 });
        x
    } else {
        descend disk { len(must(read("r"))) }
    }
}
demand descend disk {
    f(len(must(read("c"))) == 0)
};
"#;
    let ret_unit = parse_and_check(ret_src);
    let ret_res = bury(&ret_unit, 10_000).expect("buries");
    let ret_out = ret_res.as_unit(&ret_unit);
    assert!(check(&ret_out).is_empty(), "speculated let-return faults: {:?}", check(&ret_out));

    // A starved global containing an act primitive must stay as a global reference
    // rather than duplicating the act at every site.
    let act_src = r#"
Answer<U0> g = descend disk! { write("out", b"x") };
demand descend disk! {
    Bool a = given(g);
    Bool b = given(g);
    a && b
};
"#;
    let act_unit = parse_and_check(act_src);
    let act_res = bury(&act_unit, 10_000).expect("buries");
    let act_out = act_res.as_unit(&act_unit);
    assert!(check(&act_out).is_empty());
    assert_eq!(act_res.holes.len(), 1, "starved act global must dig exactly one hole");
    let act_printed = nether_syntax::print(&act_out);
    assert_eq!(
        act_printed.matches("write(").count(),
        1,
        "starved act global must not be duplicated across references: {act_printed}"
    );

    // Repeated `concat` doubling must collapse cleanly once it exceeds 64 MiB.
    let oom_src = r#"
Bytes blow(I64 n, Bytes b) {
    if (n == 0) { b } else { blow(n - 1, concat(b, b)) }
}
demand blow(25, b"0123456789abcdef");
"#;
    let err = bury_source(oom_src, 100_000).expect_err("concat > 64 MiB must collapse");
    assert!(matches!(err.kind, HaltKind::Collapsed(_)), "expected Collapsed, got {err:?}");
}

#[test]
fn select_lowers_checks_control_flow_exit_depths() {
    let src = r#"
I64 g() {
    if (true) {
        descend disk { len(must(read("a"))) }
    } else {
        if (descend disk { len(must(read("b"))) == 0 }) {
            return 1;
        }
        0
    }
}
demand descend disk { g() };
"#;
    let unit = parse_and_check(src);
    let res = bury(&unit, 10_000).expect("buries");
    let out = res.as_unit(&unit);
    assert!(check(&out).is_empty(), "select exit-depth faults: {:?}", check(&out));
}
