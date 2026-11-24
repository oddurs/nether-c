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
    let err = bury_source(oom_src, 5_000_000).expect_err("concat > 64 MiB must collapse");
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

#[test]
fn pass3_descend_return_and_nested_block() {
    // 1. `descend` must be preserved on stuck `return` inside a function body.
    let desc_src = r#"
Bytes f() {
    descend disk {
        return must(read("a.txt"));
    }
}
demand f();
"#;
    let desc_unit = parse_and_check(desc_src);
    let desc_res = bury(&desc_unit, 10_000).expect("buries");
    let desc_out = desc_res.as_unit(&desc_unit);
    assert!(check(&desc_out).is_empty(), "descend return faults: {:?}", check(&desc_out));
    let desc_printed = nether_syntax::print(&desc_out);
    assert!(check(&parse_and_check(&desc_printed)).is_empty());

    // 2. Nested block returning a stuck expression must retain its local bindings.
    let nested_src = r#"
Bytes f() {
    {
        Bytes w = b"hi";
        return opaque w;
    }
    b"nope"
}
demand f();
"#;
    let nested_unit = parse_and_check(nested_src);
    let nested_res = bury(&nested_unit, 10_000).expect("buries");
    let nested_out = nested_res.as_unit(&nested_unit);
    assert!(check(&nested_out).is_empty(), "nested block return faults: {:?}", check(&nested_out));
    let nested_printed = nether_syntax::print(&nested_out);
    assert!(check(&parse_and_check(&nested_printed)).is_empty());
}

#[test]
fn pass3_specialise_caller_locals_latent_asserted_and_apart() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{AnswerOf, Call, Value};

    let given_bytes = |b: &[u8]| Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(b.to_vec()))));

    // 3. `specialise` must never splice caller-frame LocalIds for deep Answered/Shade args.
    let caller_local = r#"
Bytes f(Answer<Bytes> a) {
    Bytes u = must(descend net { get("u") });
    concat(u, must(a))
}
Bytes caller() {
    Bytes p1 = b"1";
    Bytes p2 = b"2";
    Answer<Bytes> local_ans = descend disk { read("f.txt") };
    f(local_ans)
}
demand caller();
"#;
    let caller_unit = parse_and_check(caller_local);
    let caller_ans = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("f.txt".into()).cairn()] },
        given_bytes(b"hi"),
    );
    let caller_res =
        bury_with(&caller_unit, Cairn::of_encoded(caller_local.as_bytes()), 10_000, &caller_ans)
            .expect("buries");
    let caller_out = caller_res.as_unit(&caller_unit);
    assert!(
        check(&caller_out).is_empty(),
        "caller-local specialise faults: {:?}",
        check(&caller_out)
    );
    let caller_printed = nether_syntax::print(&caller_out);
    assert!(check(&parse_and_check(&caller_printed)).is_empty());

    // 4. `specialise` must recompute latent capabilities and clear local asserted depths.
    let stale_latent = r#"
Bytes f(Bool flag) {
    if (flag) { must(read("a.txt")) } else { must(get("https://x")) }
}
demand descend disk { descend net { f(false) } };
"#;
    let latent_unit = parse_and_check(stale_latent);
    let latent_res = bury(&latent_unit, 10_000).expect("buries");
    let latent_out = latent_res.as_unit(&latent_unit);
    assert!(check(&latent_out).is_empty(), "stale latent faults: {:?}", check(&latent_out));

    let stale_assert = r#"
Bytes@5 f(I64 k) {
    Bytes@3 a = must(descend disk { read("a.txt") });
    Bytes@5 b = must(descend net { get("u") });
    opaque concat(a, b)
}
demand f(0);
"#;
    let assert_unit = parse_and_check(stale_assert);
    let assert_ans = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("a.txt".into()).cairn()] },
        given_bytes(b"hi"),
    );
    let assert_res =
        bury_with(&assert_unit, Cairn::of_encoded(stale_assert.as_bytes()), 10_000, &assert_ans)
            .expect("buries");
    let assert_out = assert_res.as_unit(&assert_unit);
    assert!(check(&assert_out).is_empty(), "stale asserted local faults: {:?}", check(&assert_out));
    let assert_printed = nether_syntax::print(&assert_out);
    assert!(check(&parse_and_check(&assert_printed)).is_empty());

    // 5. `apart` must not collide with global or local names.
    let apart_src = r#"
I64 f__1 = 0;
Bytes f(I64 x) {
    I64 y = x + 1;
    must(descend disk { read("a") })
}
demand f(1);
"#;
    let apart_unit = parse_and_check(apart_src);
    let apart_res = bury(&apart_unit, 10_000).expect("buries");
    let apart_out = apart_res.as_unit(&apart_unit);
    assert!(check(&apart_out).is_empty());
    let apart_printed = nether_syntax::print(&apart_out);
    assert!(check(&parse_and_check(&apart_printed)).is_empty(), "apart collision: {apart_printed}");
}

#[test]
fn pass3_shade_origin_select_lowers_globals_residuals_and_declare() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{AnswerOf, Call, Value};

    let given_bytes = |b: &[u8]| Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(b.to_vec()))));

    // 5a. Shade origin preserved on pure op over answered deep value.
    let shade_eq = r#"
Bytes b = must(descend disk { read("f") });
demand (shade b) == (shade concat(b, b""));
"#;
    let eq_unit = parse_and_check(shade_eq);
    let read_f = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("f".into()).cairn()] },
        given_bytes(b"hi"),
    );
    let eq_res = bury_with(&eq_unit, Cairn::of_encoded(shade_eq.as_bytes()), 10_000, &read_f)
        .expect("buries");
    assert_eq!(eq_res.demands[0].kind, ExprKind::Literal(Literal::Bool(true)));

    // 5b. Stuck shade whose subexpression got answered must not lower origin under descend.
    let shade_stuck = r#"
demand descend net {
    look (shade concat(must(descend disk { read("f") }), must(descend net { get("u") })))
};
"#;
    let stuck_unit = parse_and_check(shade_stuck);
    let get_u = Answers::none().and(
        Call { function: "get".into(), args: vec![Value::Str("u".into()).cairn()] },
        given_bytes(b"hi"),
    );
    let stuck_res =
        bury_with(&stuck_unit, Cairn::of_encoded(shade_stuck.as_bytes()), 10_000, &get_u)
            .expect("buries");
    let stuck_out = stuck_res.as_unit(&stuck_unit);
    assert!(check(&stuck_out).is_empty(), "stuck shade origin faults: {:?}", check(&stuck_out));

    // 5c. Answered deep value shaded then consumed before starvation must prune cleanly.
    let shade_prune = r#"
demand {
    Bytes b = must(descend disk { read("f") });
    Shade<Bytes@3> s = shade b;
    Bytes u = must(descend net { get("u") });
    concat(u, descend disk { look s })
};
"#;
    let prune_unit = parse_and_check(shade_prune);
    let prune_res =
        bury_with(&prune_unit, Cairn::of_encoded(shade_prune.as_bytes()), 10_000, &read_f)
            .expect("buries");
    let prune_out = prune_res.as_unit(&prune_unit);
    assert!(check(&prune_out).is_empty(), "pruned shade faults: {:?}", check(&prune_out));

    // 5d. Reassigning a deep Shade before opaque must not synthesize `s = s;`.
    let shade_assign = r#"
Bytes f() {
    Shade<Bytes@3> s = shade must(descend disk { read("a") });
    s = shade must(descend disk { read("b") });
    descend disk { opaque (look s) }
}
demand f();
"#;
    let assign_unit = parse_and_check(shade_assign);
    let read_ab = Answers::none()
        .and(
            Call { function: "read".into(), args: vec![Value::Str("a".into()).cairn()] },
            given_bytes(b"A"),
        )
        .and(
            Call { function: "read".into(), args: vec![Value::Str("b".into()).cairn()] },
            given_bytes(b"B"),
        );
    let assign_res =
        bury_with(&assign_unit, Cairn::of_encoded(shade_assign.as_bytes()), 10_000, &read_ab)
            .expect("buries");
    let assign_out = assign_res.as_unit(&assign_unit);
    assert!(check(&assign_out).is_empty(), "shade assign faults: {:?}", check(&assign_out));
    let assign_printed = nether_syntax::print(&assign_out);
    assert!(!assign_printed.contains("s = s;"), "must not emit no-op s = s: {assign_printed}");
}

#[test]
fn pass3_select_lowers_guards() {
    // 6a & 6b. Decided select preserves local write depths under speculation and before opaque.
    let sel_spec = r#"
Bytes@5 g(Bool c) {
    Bytes x = must(descend disk { read("f") });
    if (must(descend net { get("u") }) == b"a") { return b"early"; }
    if (c) { x = b"pure"; }
    descend net { look shade x }
}
demand g(false);
"#;
    let spec_unit = parse_and_check(sel_spec);
    let spec_res = bury(&spec_unit, 10_000).expect("buries");
    let spec_out = spec_res.as_unit(&spec_unit);
    assert!(check(&spec_out).is_empty(), "speculated select lowers faults: {:?}", check(&spec_out));

    let sel_opaque = r#"
Bytes@5 g(Bool c) {
    Bytes x;
    if (c) { x = must(descend net { get("u") }); }
    else   { x = b"ok"; }
    Shade<Bytes@5> s = opaque (shade x);
    descend net { look s }
}
demand g(false);
"#;
    let opq_unit = parse_and_check(sel_opaque);
    let opq_res = bury(&opq_unit, 10_000).expect("buries");
    let opq_out = opq_res.as_unit(&opq_unit);
    assert!(check(&opq_out).is_empty(), "opaque select lowers faults: {:?}", check(&opq_out));
    let opq_printed = nether_syntax::print(&opq_out);
    assert!(
        check(&parse_and_check(&opq_printed)).is_empty(),
        "reparsed select guard: {opq_printed}"
    );
}

#[test]
fn pass3_globals_residuals_declare_and_rollback() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{Call, Value};

    // 7a. Starved global inside rolled-back loop must not dig duplicate act holes.
    let dup_global = r"
Bytes g = descend entropy { draw(4) };
demand { for (I64 i = 0; i < 1; i = i + 1) { g; } };
demand g;
";
    let dup_unit = parse_and_check(dup_global);
    let dup_res = bury(&dup_unit, 10_000).expect("buries");
    assert_eq!(dup_res.holes.len(), 1, "one global must dig at most one entropy hole");

    // 7b. Rolled-back act answer must not be stolen by a later demand.
    let steal_act = r#"
Bytes g = {
    Bytes a = descend entropy { draw(4) };
    Bytes b = must(descend net { get("u") });
    concat(a, b)
};
demand g;
demand descend entropy { draw(4) };
"#;
    let steal_unit = parse_and_check(steal_act);
    let draw_four = Answers::none().and(
        Call { function: "draw".into(), args: vec![Value::Int(4).cairn()] },
        Value::Bytes(b"AAAA".to_vec()),
    );
    let steal_res =
        bury_with(&steal_unit, Cairn::of_encoded(steal_act.as_bytes()), 10_000, &draw_four)
            .expect("buries");
    assert!(
        !matches!(steal_res.demands[1].kind, ExprKind::Literal(_)),
        "second demand must not steal rolled-back draw answer"
    );

    // 7c. Starved global calling an act helper indirectly must not be inlined twice.
    let indirect_act = r"
Bytes do_draw() { descend entropy { draw(4) } }
Bytes g = do_draw();
demand g;
demand g;
";
    let ind_unit = parse_and_check(indirect_act);
    let ind_res = bury(&ind_unit, 10_000).expect("buries");
    let ind_printed = nether_syntax::print(&ind_res.as_unit(&ind_unit));
    assert_eq!(
        ind_printed.matches("demand g;").count(),
        2,
        "indirect act global must stay as `demand g;` at both demands: {ind_printed}"
    );
    assert_eq!(
        ind_printed.matches("do_draw(").count(),
        2,
        "only the function definition and global initializer may name do_draw(: {ind_printed}"
    );
}

#[test]
fn pass3_residuals_declare_and_rollback() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{AnswerOf, Call, Value};

    let given_bytes = |b: &[u8]| Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(b.to_vec()))));

    // 8a. Pure `answered(x)` captures mutated `x` cleanly as `answered(b"new")`.
    let ans_mut = r#"
Bytes f() {
    Bytes x = b"old";
    x = b"new";
    Answer<Bytes> a = answered(x);
    Bytes u = must(descend disk { read("f") });
    opaque concat(must(a), u)
}
demand f();
"#;
    let mut_unit = parse_and_check(ans_mut);
    let mut_res = bury(&mut_unit, 10_000).expect("buries");
    let mut_out = mut_res.as_unit(&mut_unit);
    assert!(check(&mut_out).is_empty(), "answered(x) faults: {:?}", check(&mut_out));
    let mut_printed = nether_syntax::print(&mut_out);
    assert!(
        mut_printed.contains("answered(b\"new\")"),
        "expected answered(b\"new\"), got: {mut_printed}"
    );

    // 8b. Answered `post` whose binding is consumed before starvation is pruned from residue.
    let post_prune = r#"
demand {
    Answer<Bytes> a = descend net! { post("https://x", b"hi") };
    Bytes x = must(a);
    Bytes u = must(descend net { get("u") });
    concat(x, u)
};
"#;
    let post_unit = parse_and_check(post_prune);
    let post_ans = Answers::none().and(
        Call {
            function: "post".into(),
            args: vec![
                Value::Str("https://x".into()).cairn(),
                Value::Bytes(b"hi".to_vec()).cairn(),
            ],
        },
        given_bytes(b"ok"),
    );
    let post_res =
        bury_with(&post_unit, Cairn::of_encoded(post_prune.as_bytes()), 10_000, &post_ans)
            .expect("buries");
    let post_out = post_res.as_unit(&post_unit);
    assert!(check(&post_out).is_empty());
    let post_printed = nether_syntax::print(&post_out);
    assert!(
        !post_printed.contains("post("),
        "consumed answered post must not stay in residue: {post_printed}"
    );

    // 10. `Stmt::Declare` inside a loop resets `env` between iterations.
    let decl_loop = r"
I64 f() {
    I64 out = 0;
    for (I64 i = 0; i < 2; i = i + 1) {
        I64 x;
        if (i == 0) { x = 10; } else { out = x; }
    }
    out
}
demand f();
";
    let decl_unit = parse_and_check(decl_loop);
    let decl_res = bury(&decl_unit, 10_000).expect("buries");
    assert_ne!(
        decl_res.demands[0].kind,
        ExprKind::Literal(Literal::Int(10)),
        "loop-local Declare must not leak turn 0 value into turn 1"
    );

    // 11. Rolled-back deposit in an abandoned loop must not leak its Value into `named`.
    let rollback_val = r#"
demand {
    for (I64 i = 0; i < 2; i = i + 1) {
        if (i == 0) { 12345; }
        else { must(descend disk { read("f") }); }
    }
};
"#;
    let rb_unit = parse_and_check(rollback_val);
    let rb_res = bury(&rb_unit, 10_000).expect("buries");
    let leaked = Value::Int(12345).cairn();
    assert!(
        !rb_res.named.iter().any(|(c, _)| *c == leaked),
        "rolled-back deposit value must not leak into residue.named"
    );
}

#[test]
fn pass4_pc_assign_rollback_seal_and_fuel() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{AnswerOf, Call, Value};

    let given_bytes = |b: &[u8]| Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(b.to_vec()))));

    // 1. Outer-local `Answer` assignment inside a finished nested block survives into residue.
    let nested_ans = r#"
demand {
    Answer<Bytes> a = refused(absent);
    {
        a = descend disk { read("k") };
    }
    Bytes u = must(descend disk { read("u") });
    concat(must(a), u)
};
"#;
    let na_unit = parse_and_check(nested_ans);
    let na_ans = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("k".into()).cairn()] },
        given_bytes(b"hello"),
    );
    let na_res =
        bury_with(&na_unit, Cairn::of_encoded(nested_ans.as_bytes()), 10_000, &na_ans).unwrap();
    let na_out = na_res.as_unit(&na_unit);
    assert!(check(&na_out).is_empty(), "nested Answer assign faults: {:?}", check(&na_out));
    let na_printed = nether_syntax::print(&na_out);
    assert!(
        na_printed.contains("read(\"k\")"),
        "outer-local Answer assign in inner block must survive: {na_printed}"
    );

    // 2. Global and Stmt::Let returning Answer after a deposit roll back that deposit.
    let global_dep = r#"
Answer<Bytes> g = {
    777;
    descend disk { read("g") }
};
demand g;
demand g;
"#;
    let gd_unit = parse_and_check(global_dep);
    let gd_ans = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("g".into()).cairn()] },
        given_bytes(b"ok"),
    );
    let gd_res =
        bury_with(&gd_unit, Cairn::of_encoded(global_dep.as_bytes()), 10_000, &gd_ans).unwrap();
    assert!(
        gd_res.deposits.is_empty(),
        "global that cannot close-residualise must roll back deposits: {:?}",
        gd_res.deposits
    );

    let let_dep = r#"
demand {
    Answer<Bytes> a = {
        888;
        descend disk { read("l") }
    };
    Bytes u = must(descend disk { read("u") });
    concat(must(a), u)
};
"#;
    let ld_unit = parse_and_check(let_dep);
    let ld_ans = Answers::none().and(
        Call { function: "read".into(), args: vec![Value::Str("l".into()).cairn()] },
        given_bytes(b"ok"),
    );
    let ld_res =
        bury_with(&ld_unit, Cairn::of_encoded(let_dep.as_bytes()), 10_000, &ld_ans).unwrap();
    assert!(
        ld_res.deposits.is_empty(),
        "Stmt::Let that cannot close-residualise must roll back deposits: {:?}",
        ld_res.deposits
    );
}

#[test]
fn pass4_seal_list_fuel_and_branch_pc() {
    use nether_bury::{Answers, bury_with};
    use nether_ledger::{AnswerOf, Call, Value};

    // 3. Speculative `seal` does not leak its Value into `residue.named`.
    let spec_seal = r#"
demand {
    Bytes u = must(descend disk { read("u") });
    Cairn c = seal 99999;
    u
};
"#;
    let ss_unit = parse_and_check(spec_seal);
    let ss_res = bury(&ss_unit, 10_000).unwrap();
    let sealed_val = Value::Int(99999).cairn();
    assert!(
        !ss_res.named.iter().any(|(c, _)| *c == sealed_val),
        "speculative seal must not leak Value into residue.named"
    );

    // 4. Answered `list(expr)` preserves reduced argument in residue.
    let list_arg = r#"
demand {
    Str dir = "sub";
    Answer<Str[]> ls = descend disk { list(dir) };
    Bytes u = must(descend disk { read("u") });
    if (given(ls)) { u } else { b"" }
};
"#;
    let la_unit = parse_and_check(list_arg);
    let la_ans = Answers::none().and(
        Call { function: "list".into(), args: vec![Value::Str("sub".into()).cairn()] },
        Value::Answer(Box::new(AnswerOf::Given(Value::Array(vec![Value::Str("a".into())])))),
    );
    let la_res =
        bury_with(&la_unit, Cairn::of_encoded(list_arg.as_bytes()), 10_000, &la_ans).unwrap();
    let la_out = la_res.as_unit(&la_unit);
    assert!(check(&la_out).is_empty(), "list aggregate answer faults: {:?}", check(&la_out));

    // 5. Byte-proportional fuel halts exponential concat amplification within fuel budget.
    let amp = r#"
Bytes grow(I64 n, Bytes b) {
    if (n == 0) { b } else { grow(n - 1, concat(b, b)) }
}
demand grow(18, b"0123456789abcdef");
"#;
    let amp_unit = parse_and_check(amp);
    let amp_err =
        bury(&amp_unit, 1_000).expect_err("byte-proportional fuel must halt amplification");
    assert!(matches!(amp_err.kind, HaltKind::OutOfFuel { .. }), "got {amp_err:?}");

    // 6. Undecided branch arm `pc` and stuck-RHS assign preserve valid depths in residue.
    let pc_assign = r#"
demand {
    I64 x = 0;
    if (given(descend disk { read("flag") })) {
        Bytes u = must(descend disk { read("u") });
        x = 1;
        u
    } else {
        b""
    }
};
"#;
    let pc_unit = parse_and_check(pc_assign);
    let pc_res = bury(&pc_unit, 10_000).unwrap();
    let pc_out = pc_res.as_unit(&pc_unit);
    assert!(check(&pc_out).is_empty(), "undecided branch pc faults: {:?}", check(&pc_out));
}
