//! The proof *normalization by evaluation* owes.
//!
//! Not a unit test. It is the observable fact that settles the item, and it
//! prints its measurement so the number lands in the pull request rather than
//! in somebody's memory.
//!
//! `#[ignore]`d, because two and a half million evaluation steps is not worth
//! doing on every commit. `scripts/task proofs` runs it. The cheap version,
//! `fib(20)`, runs with everything else.

use std::time::Instant;

use nether_core::{
    BinOp, Block, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, Literal, LocalDef, LocalId, Span,
    Stmt, Type, Unit, bury, check,
};

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

fn call(callee: Expr, args: Vec<Expr>, result: Type, depth: Depth) -> Expr {
    e(ExprKind::Call { callee: Box::new(callee), args }, result, depth)
}

/// Anything at all that is still a call, anywhere in the residue.
fn calls(x: &Expr) -> usize {
    let here = usize::from(matches!(x.kind, ExprKind::Call { .. }));
    here + match &x.kind {
        ExprKind::Binary { lhs, rhs, .. } => calls(lhs) + calls(rhs),
        ExprKind::Unary { operand, .. } | ExprKind::Rite { operand, .. } => calls(operand),
        ExprKind::Call { callee, args } => calls(callee) + args.iter().map(calls).sum::<usize>(),
        _ => 0,
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
        latent: Depth::PURE,
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
            latent: Depth::PURE,
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

#[test]
#[ignore = "two and a half million steps; run it with scripts/task proofs"]
fn fib_of_thirty_burns_to_a_literal() {
    let unit = fib_demanding(30);
    assert!(check(&unit).is_empty());

    let started = Instant::now();
    let residue = bury(&unit, 100_000_000).expect("fib(30) is finite");
    let took = started.elapsed();

    assert_eq!(residue.demands.len(), 1);
    assert_eq!(residue.demands[0].kind, ExprKind::Literal(Literal::Int(832_040)));
    assert_eq!(calls(&residue.demands[0]), 0, "the residue still contains a call");
    assert_eq!(residue.depth, Depth::PURE);

    let spent = residue.fuel_spent;
    let left = calls(&residue.demands[0]);
    println!();
    println!("  fib(30) → 832040");
    println!("    {spent:>12} evaluation steps");
    println!("    {took:>12.2?} wall clock");
    println!("    {left:>12} calls left in the residue");
    println!();
}
