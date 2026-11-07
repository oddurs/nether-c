//! The proof *normalization by evaluation* owes.
//!
//! Not a unit test. It is the observable fact that settles the item, and it
//! prints its measurement so the number lands in the pull request rather than
//! in somebody's memory.
//!
//! `#[ignore]`d, because two and a half million evaluation steps is not worth
//! doing on every commit. `scripts/task proofs` runs it. The cheap version,
//! `fib(20)`, runs with everything else.

use std::collections::HashSet;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};
use std::time::Instant;

use nether_bury::{Answers, Halt, Residue, bury_on};
use nether_core::{
    BinOp, Block, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, Held, Literal, LocalDef, LocalId,
    Span, Stmt, Type, Unit, check,
};
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse, print};

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

/// The cairn these units pretend to have been lowered from.
fn source_cairn() -> Cairn {
    Cairn::of_encoded(b"a source")
}

/// `bury`, with a source for the spans to belong to.
fn bury(unit: &Unit, fuel: u64) -> Result<Residue, Halt> {
    nether_bury::bury(unit, source_cairn(), fuel)
}

const MIXED: &str = r#"
I64 fib(I64 n) {
  if (n < 2) { return n; }
  return fib(n - 1) + fib(n - 2);
}
U0 say(I64 n) { "fib\n"; fib(n); }
Bytes@3 note = must(descend disk { read("note.txt") });
I64 shared = fib(9);

demand say(12);
demand fib(13);
demand shared + 1;
demand concat(note, b"!");
demand say(11);
demand descend disk { read("note.txt") };
demand shared + fib(10);
demand fib(12);
demand must(descend disk { read("absent.txt") });
"#;

fn trace_cairn(unit: &Unit, r: &Residue) -> Cairn {
    let printed = print(&r.as_unit(unit));
    let residue = Stored::Value(Value::Bytes(printed.into_bytes())).cairn();
    let depth = r.questions().into_iter().fold(r.depth.get(), |d, n| match n {
        Node::Hole { stratum, .. } => d.max(*stratum),
        _ => d,
    });
    Node::Trace {
        residue,
        holes: r.holes.clone(),
        unrecorded: false,
        depth,
        witnesses: Vec::new(),
        deposits: r.deposits.clone(),
        source: source_cairn(),
    }
    .cairn()
}

/// A thousand parallel burials of one program produce one distinct cairn. 0205.
#[test]
#[ignore = "a thousand parallel burials; run it with scripts/task proofs"]
fn a_thousand_parallel_burials_produce_one_cairn() {
    let unit = lower(&parse(MIXED.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&unit).is_empty());

    let serial =
        bury_on(&unit, source_cairn(), 50_000_000, &Answers::none(), 1).expect("serial buries");
    let want_cairn = trace_cairn(&unit, &serial);
    let want_questions = serial.questions();

    let counts = [2usize, 3, 4, 6, 8, 12, 16];
    let mut cairns = HashSet::new();
    let stop = AtomicBool::new(false);
    let started = Instant::now();

    std::thread::scope(|s| {
        for contender in 0..4usize {
            let stop = &stop;
            s.spawn(move || {
                let mut tick = contender;
                while !stop.load(Relaxed) {
                    tick = tick.wrapping_add(1);
                    if tick % 16 == 0 {
                        std::thread::yield_now();
                    }
                }
            });
        }
        for run in 0..1_000usize {
            let threads = counts[run % counts.len()];
            for _ in 0..(run % 5) {
                std::thread::yield_now();
            }
            let got = bury_on(&unit, source_cairn(), 50_000_000, &Answers::none(), threads)
                .expect("parallel buries");
            assert_eq!(got.questions(), want_questions, "run {run} ({threads} threads)");
            assert_eq!(got.named, serial.named, "run {run} ({threads} threads)");
            cairns.insert(trace_cairn(&unit, &got));
        }
        stop.store(true, Relaxed);
    });
    let elapsed = started.elapsed();

    println!();
    println!("  0205  1000 parallel burials across threads {counts:?}");
    println!("        {} distinct cairn(s)  serial {}", cairns.len(), want_cairn.short());
    println!(
        "        {} steps  {} objects  {} holes  in {elapsed:.2?}",
        serial.fuel_spent,
        serial.named.len(),
        serial.holes.len()
    );
    println!();

    assert_eq!(cairns.len(), 1);
    assert!(cairns.contains(&want_cairn));
}
