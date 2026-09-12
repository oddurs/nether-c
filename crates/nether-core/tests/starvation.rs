//! The proof for *fuel and starvation diagnostics*: a program that would
//! unroll forever fails in bounded time and names the loop, not the leaf.
//!
//! And the arithmetic underneath it. §6.4 says what a step is, so the counts
//! here are worked out by hand from the source rather than read off a run.

use nether_core::{
    BinOp, Block, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, GlobalDef, GlobalId, Grinding,
    HaltKind, Literal, LocalDef, LocalId, MAX_FRAMES, Rite, Span, Stmt, Type, Unit, bury, report,
};

// ── the file the fuel error is about ────────────────────────────────────────

const FILLER: &str = "//\n";
const SPIN_LINE: &str = "U0 spin() { while (true) { } }\n";

fn source() -> String {
    FILLER.repeat(2) + SPIN_LINE
}

fn spin_span() -> Span {
    let start = FILLER.len() * 2 + SPIN_LINE.find("while").expect("the line moved");
    let start = u32::try_from(start).unwrap();
    Span { start, end: start + u32::try_from("while (true) { }".len()).unwrap() }
}

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
    pure(ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, ty)
}

fn demanding(value: Expr) -> Unit {
    Unit { demands: vec![Demand { value, span: Span::default() }], ..Unit::default() }
}

/// `while (true) { }` — nothing in it, and no way out of it.
fn spinning() -> Unit {
    let mut looping = pure(
        ExprKind::Loop {
            body: Box::new(pure(
                ExprKind::Block(Block { stmts: Vec::new(), tail: None, span: Span::default() }),
                Type::Unit,
            )),
            step: None,
        },
        Type::Unit,
    );
    looping.span = spin_span();
    demanding(looping)
}

/// `I64 down(I64 n) { return down(n + 1); }` — and a call to it.
fn falling() -> Unit {
    let arrow = Type::Fn {
        params: vec![Type::Int],
        latent: Depth::PURE,
        result: Box::new(Type::Int),
        result_depth: Depth::PURE,
    };
    let me = || pure(ExprKind::Func(FuncId(0)), arrow.clone());
    let deeper = pure(
        ExprKind::Call {
            callee: Box::new(me()),
            args: vec![add(
                BinOp::Add,
                pure(ExprKind::Local(LocalId(0)), Type::Int),
                int(1),
                Type::Int,
            )],
        },
        Type::Int,
    );
    Unit {
        funcs: vec![FuncDef {
            name: "down".into(),
            params: vec![LocalId(0)],
            ret: Type::Int,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Depth::PURE,
            asserted_latent: None,
            locals: vec![LocalDef {
                name: "n".into(),
                ty: Type::Int,
                asserted: None,
                span: Span { start: 0, end: 4 },
            }],
            body: Block {
                stmts: vec![Stmt::Expr(pure(ExprKind::Return(Some(Box::new(deeper))), Type::Unit))],
                tail: None,
                span: Span::default(),
            },
            span: Span { start: 0, end: 4 },
        }],
        demands: vec![Demand {
            value: pure(ExprKind::Call { callee: Box::new(me()), args: vec![int(0)] }, Type::Int),
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

/// `I64 count(I64 n) { if (n <= 0) { return 0; } return count(n - 1); }`
fn counting(from: i64) -> Unit {
    let arrow = Type::Fn {
        params: vec![Type::Int],
        latent: Depth::PURE,
        result: Box::new(Type::Int),
        result_depth: Depth::PURE,
    };
    let me = || pure(ExprKind::Func(FuncId(0)), arrow.clone());
    let n = || pure(ExprKind::Local(LocalId(0)), Type::Int);
    let base = pure(
        ExprKind::Block(Block {
            stmts: vec![Stmt::Expr(pure(ExprKind::Return(Some(Box::new(int(0)))), Type::Unit))],
            tail: None,
            span: Span::default(),
        }),
        Type::Unit,
    );
    let guard = pure(
        ExprKind::Select {
            cond: Box::new(add(BinOp::Le, n(), int(0), Type::Bool)),
            then: Box::new(base),
            otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
        },
        Type::Unit,
    );
    let down = pure(
        ExprKind::Return(Some(Box::new(pure(
            ExprKind::Call {
                callee: Box::new(me()),
                args: vec![add(BinOp::Sub, n(), int(1), Type::Int)],
            },
            Type::Int,
        )))),
        Type::Unit,
    );
    Unit {
        funcs: vec![FuncDef {
            name: "count".into(),
            params: vec![LocalId(0)],
            ret: Type::Int,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent: Depth::PURE,
            asserted_latent: None,
            locals: vec![LocalDef {
                name: "n".into(),
                ty: Type::Int,
                asserted: None,
                span: Span::default(),
            }],
            body: Block {
                stmts: vec![Stmt::Expr(guard), Stmt::Expr(down)],
                tail: None,
                span: Span::default(),
            },
            span: Span::default(),
        }],
        demands: vec![Demand {
            value: pure(
                ExprKind::Call { callee: Box::new(me()), args: vec![int(from)] },
                Type::Int,
            ),
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

// ── the proof ───────────────────────────────────────────────────────────────

#[test]
fn a_loop_that_would_never_stop_fails_in_bounded_time_naming_the_loop() {
    let halt = bury(&spinning(), 1_000).expect_err("this does not stop");
    assert!(matches!(halt.kind, HaltKind::OutOfFuel { spent: 1_000 }));
    assert_eq!(halt.grinding, Some(Grinding::Loop { turns: 999 }));
    assert_eq!(halt.span, spin_span(), "it pointed at the leaf, not the loop");
}

#[test]
fn the_fuel_error_has_the_shape_the_codex_prints() {
    let halt = bury(&spinning(), 1_000).unwrap_err();
    let printed = report(&halt.diagnostic(), &source(), "spin.nc");
    let lines: Vec<&str> = printed.lines().collect();

    assert_eq!(lines[0], "error: burial ran out of fuel after 1,000 steps");
    assert_eq!(lines[1], " --> spin.nc:3:13");
    assert_eq!(lines[3], "3 | U0 spin() { while (true) { } }");
    assert!(lines[4].contains("^^^^^^^^^^^^^^^^ unrolled 999 times"), "{}", lines[4]);
    assert!(lines[6].contains("`opaque` barrier"), "{}", lines[6]);
}

#[test]
fn runaway_recursion_names_the_function_and_how_deep_it_went() {
    // Fuel does not bound this one — a recursion that never returns runs out
    // of frames before it runs out of steps, whatever the budget. §6.4 makes
    // that the implementation's limit to state and to report.
    let halt = bury(&falling(), 100_000_000).expect_err("this does not stop either");
    assert_eq!(halt.kind, HaltKind::TooDeep { limit: MAX_FRAMES });
    let Some(Grinding::Calls { deep, name }) = halt.grinding else {
        panic!("blamed the wrong thing: {:?}", halt.grinding)
    };
    assert_eq!(name, "down");
    assert_eq!(deep, u64::from(MAX_FRAMES));
}

#[test]
fn a_recursion_that_stops_inside_the_limit_is_not_troubled_by_it() {
    // The limit is a limit and not a style. One frame inside it is fine and
    // one frame outside says so rather than falling over — and the one that is
    // fine is two thousand deep, which is the point of burial having a stack
    // of its own instead of borrowing its caller's.
    let inside = i64::from(MAX_FRAMES) - 2;
    assert!(bury(&counting(inside), 100_000_000).is_ok());
    let halt = bury(&counting(inside + 4), 100_000_000).unwrap_err();
    assert_eq!(halt.kind, HaltKind::TooDeep { limit: MAX_FRAMES });
}

#[test]
fn the_limit_is_reported_and_not_crashed_into() {
    let halt = bury(&falling(), 100_000_000).unwrap_err();
    let d = halt.diagnostic();
    assert_eq!(d.headline, format!("burial went more than {MAX_FRAMES} calls deep"));
    assert!(d.label.is_some_and(|l| l.contains("`down` called 2,048 deep")));
    assert!(d.note.is_some_and(|n| n.contains("limit of the implementation")));
}

// ── what a step is ──────────────────────────────────────────────────────────

fn spent(unit: &Unit) -> u64 {
    bury(unit, 1_000_000).expect("this finishes").fuel_spent
}

#[test]
fn a_step_is_one_evaluation_of_one_expression_node() {
    // One literal is one node.
    assert_eq!(spent(&demanding(int(1))), 1);
    // `1 + 2` is three: the sum and both sides of it.
    assert_eq!(spent(&demanding(add(BinOp::Add, int(1), int(2), Type::Int))), 3);
    // `(1 + 2) + 3` is five.
    let nested = add(BinOp::Add, add(BinOp::Add, int(1), int(2), Type::Int), int(3), Type::Int);
    assert_eq!(spent(&demanding(nested)), 5);
}

#[test]
fn a_binding_named_twice_costs_its_value_once() {
    // `I64 g = 1 + 2;  demand g + g;`
    //
    // Three for the sum and its two `g`s, and three more for `1 + 2`, once.
    // Six, not nine.
    let g = || pure(ExprKind::Global(GlobalId(0)), Type::Int);
    let unit = Unit {
        globals: vec![GlobalDef {
            name: "g".into(),
            ty: Type::Int,
            asserted: None,
            value: add(BinOp::Add, int(1), int(2), Type::Int),
            span: Span::default(),
        }],
        demands: vec![Demand {
            value: add(BinOp::Add, g(), g(), Type::Int),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    assert_eq!(spent(&unit), 6);
}

#[test]
fn opaque_costs_one_step_and_what_is_inside_it_costs_nothing() {
    let inside = add(
        BinOp::Add,
        add(BinOp::Add, int(1), int(2), Type::Int),
        add(BinOp::Add, int(3), int(4), Type::Int),
        Type::Int,
    );
    assert_eq!(spent(&demanding(inside.clone())), 7);
    let barrier = pure(ExprKind::Rite { rite: Rite::Opaque, operand: Box::new(inside) }, Type::Int);
    assert_eq!(spent(&demanding(barrier)), 1);
}

#[test]
fn a_residue_costs_nothing_for_what_is_already_reduced() {
    // The staging law is affordable as well as true: burying a residue does
    // not pay again for the part that is already a literal.
    let first = add(
        BinOp::Add,
        add(BinOp::Add, int(1), int(2), Type::Int),
        add(BinOp::Add, int(3), int(4), Type::Int),
        Type::Int,
    );
    let once = bury(&demanding(first), 1_000).unwrap();
    assert_eq!(once.fuel_spent, 7);

    let again = Unit {
        demands: once
            .demands
            .iter()
            .map(|d| Demand { value: d.clone(), span: Span::default() })
            .collect(),
        ..Unit::default()
    };
    assert_eq!(bury(&again, 1_000).unwrap().fuel_spent, 1);
}

// ── starvation, which is the other thing ────────────────────────────────────

#[test]
fn starvation_points_at_the_expression_and_offers_nothing() {
    // §9.9: there is no rescue, no try, and there will not be one. The error
    // says so rather than suggesting a flag, because there is no flag.
    let halt = bury(&demanding(add(BinOp::Div, int(1), int(0), Type::Int)), 100).unwrap_err();
    assert_eq!(halt.kind, HaltKind::Starved("this divides by zero"));
    assert_eq!(halt.grinding, None);

    let d = halt.diagnostic();
    assert_eq!(d.headline, "this divides by zero");
    assert!(d.note.is_some_and(|n| n.contains("not catchable")));
    assert_eq!(d.label, None);
}
