//! The proof for *holes and residualisation*: what survives burial, and in
//! what form.
//!
//! A hole carries its call, its stratum and its source span. What it does not
//! carry is its dependents — it cannot, because a node is named by its content
//! and one that listed the things waiting on it would get a new name every
//! time something came to wait. §6.3.

use nether_bury::{Residue, bury};
use nether_core::{
    BinOp, Capability, Demand, Depth, Expr, ExprKind, Literal, Prim, Rite, Span, Type, Unit, check,
};
use nether_ledger::{Cairn, Node, Stored, Value};

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

fn str_lit(s: &str) -> Expr {
    pure(ExprKind::Literal(Literal::Str(s.into())), Type::Str)
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

fn read_of(path: Expr, span: Span) -> Expr {
    let mut c = call(
        prim(Prim::Read, vec![Type::Str], answer_bytes()),
        vec![path],
        answer_bytes(),
        Depth::DISK,
    );
    c.span = span;
    c
}

fn reading(path: &str) -> Expr {
    read_of(str_lit(path), Span::default())
}

fn descending(body: Expr) -> Expr {
    let (ty, depth) = (body.ty.clone(), body.depth);
    e(ExprKind::Descend { capability: Capability::Disk, body: Box::new(body) }, ty, depth)
}

fn demanding(values: Vec<Expr>) -> Unit {
    Unit {
        demands: values.into_iter().map(|value| Demand { value, span: Span::default() }).collect(),
        ..Unit::default()
    }
}

fn source() -> Cairn {
    Cairn::of_encoded(b"a source")
}

fn buried(unit: &Unit) -> Residue {
    let faults = check(unit);
    assert!(faults.is_empty(), "the unit does not check: {faults:?}");
    bury(unit, source(), 100_000).expect("this was supposed to finish")
}

// ── holes ───────────────────────────────────────────────────────────────────

#[test]
fn a_question_the_world_can_answer_becomes_a_hole() {
    let at = Span { start: 40, end: 52 };
    let r = buried(&demanding(vec![descending(read_of(str_lit("main.nc"), at))]));

    assert_eq!(r.holes.len(), 1);
    let [Node::Hole { call, stratum, span, depends }] = r.questions()[..] else {
        panic!("not one hole: {:?}", r.questions())
    };
    assert_eq!(call.function, "read");
    assert_eq!(*stratum, 3);
    assert_eq!(span.source, source(), "a span in a trace names its source by cairn");
    assert_eq!((span.start, span.end), (40, 52));

    // A hole does not record its dependents, and within one burial it records
    // no dependencies either: a hole is only formed once every argument is a
    // finished value, so nothing it needs can still be waiting.
    assert!(depends.is_empty());

    // The argument is in the ledger beside it, so the question is readable
    // without the program that asked it.
    assert_eq!(r.value(call.args[0]), Some(&Value::Str("main.nc".into())));
}

#[test]
fn a_holes_arguments_are_finished() {
    // §6.3: `read(concat(dir, name))` does not leave a hole containing a
    // `concat`. It leaves a hole containing the path.
    let joined = call(
        prim(Prim::Concat, vec![Type::Bytes, Type::Bytes], Type::Bytes),
        vec![str_lit("lib/"), str_lit("table.nc")],
        Type::Str,
        Depth::PURE,
    );
    let r = buried(&demanding(vec![descending(read_of(joined, Span::default()))]));

    let [Node::Hole { call, .. }] = r.questions()[..] else { panic!() };
    assert_eq!(r.value(call.args[0]), Some(&Value::Bytes(b"lib/table.nc".to_vec())));
}

#[test]
fn the_same_question_twice_is_one_hole() {
    // What makes exhumation cheap: reading the same file twice is one
    // question, asked once. §6.3.
    let r = buried(&demanding(vec![
        descending(reading("k")),
        descending(reading("k")),
        descending(reading("k")),
    ]));
    assert_eq!(r.holes.len(), 1);
}

#[test]
fn two_different_questions_are_two_holes_in_the_order_they_were_found() {
    let r = buried(&demanding(vec![descending(reading("b")), descending(reading("a"))]));
    assert_eq!(r.holes.len(), 2);
    let asked: Vec<String> = r
        .questions()
        .iter()
        .map(|n| match n {
            Node::Hole { call, .. } => match r.value(call.args[0]) {
                Some(Value::Str(s)) => s.clone(),
                other => panic!("{other:?}"),
            },
            other => panic!("{other:?}"),
        })
        .collect();
    assert_eq!(asked, vec!["b".to_string(), "a".to_string()], "source order, not sorted");
}

#[test]
fn a_question_whose_argument_is_not_here_yet_is_not_a_question_yet() {
    // `read(must(read("dir")))`. The inner one can be asked now; the outer one
    // is not a question the world could answer, because nobody knows what it
    // would be asking about.
    let inner = call(
        prim(Prim::Must, vec![answer_bytes()], Type::Str),
        vec![reading("dir")],
        Type::Str,
        Depth::DISK,
    );
    let r = buried(&demanding(vec![descending(read_of(inner, Span::default()))]));

    let [Node::Hole { call, .. }] = r.questions()[..] else { panic!() };
    assert_eq!(r.value(call.args[0]), Some(&Value::Str("dir".into())));
}

#[test]
fn a_loop_that_residualises_still_says_what_it_would_ask() {
    // The loop is abandoned and residualised whole, so burying the residue
    // runs it again from the same state and asks the same thing. Dropping the
    // question would hide the capability somebody needs to grant.
    let body = pure(
        ExprKind::Block(nether_core::Block {
            stmts: vec![nether_core::Stmt::Expr(descending(reading("k")))],
            tail: None,
            span: Span::default(),
        }),
        Type::Unit,
    );
    let looping = pure(ExprKind::Loop { body: Box::new(body), step: None }, Type::Unit);
    let r = buried(&demanding(vec![looping]));

    assert!(matches!(r.demands[0].kind, ExprKind::Loop { .. }));
    assert_eq!(r.holes.len(), 1);
}

// ── seal ────────────────────────────────────────────────────────────────────

#[test]
fn seal_of_a_value_that_is_here_becomes_its_name() {
    let sealed = pure(ExprKind::Rite { rite: Rite::Seal, operand: Box::new(int(7)) }, Type::Cairn);
    let r = buried(&demanding(vec![sealed]));

    let want = Value::Int(7).cairn();
    assert_eq!(r.demands[0].kind, ExprKind::Literal(Literal::Cairn(*want.as_bytes())));
    // And what it names is in the ledger, so the name resolves.
    assert_eq!(r.value(want), Some(&Value::Int(7)));
}

#[test]
fn seal_of_something_the_world_has_not_answered_stays_a_seal() {
    let sealed = pure(
        ExprKind::Rite { rite: Rite::Seal, operand: Box::new(descending(reading("k"))) },
        Type::Cairn,
    );
    let r = buried(&demanding(vec![sealed]));
    assert!(matches!(r.demands[0].kind, ExprKind::Rite { rite: Rite::Seal, .. }));
}

#[test]
fn sealing_a_shade_names_the_shade_and_not_what_is_inside_it() {
    // §1.5: `seal` on a shade names the shade. §7.1 puts the stratum it came
    // out of in its encoding, so the two names are different, and reaching
    // through an opaque thing for a name would be a hole in it.
    let shaded = pure(
        ExprKind::Rite { rite: Rite::Shade, operand: Box::new(int(7)) },
        Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Int) },
    );
    let sealed = pure(ExprKind::Rite { rite: Rite::Seal, operand: Box::new(shaded) }, Type::Cairn);
    let r = buried(&demanding(vec![sealed]));

    let inside = Value::Int(7).cairn();
    let want = Value::Shade { origin: 0, value: inside }.cairn();
    assert_eq!(r.demands[0].kind, ExprKind::Literal(Literal::Cairn(*want.as_bytes())));
    assert_ne!(want, inside, "the shade and what it holds have the same name");
}

#[test]
fn two_shades_are_equal_when_their_stratum_and_their_value_are() {
    // §5.3, which compares canonical encodings, and §7.1, which puts the
    // origin stratum in a shade's. Comparing them does not count as looking at
    // them: it reveals only what `seal` on each would reveal anyway.
    let shade = |n: i64| {
        pure(
            ExprKind::Rite { rite: Rite::Shade, operand: Box::new(int(n)) },
            Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Int) },
        )
    };
    let same = pure(
        ExprKind::Binary { op: BinOp::Eq, lhs: Box::new(shade(7)), rhs: Box::new(shade(7)) },
        Type::Bool,
    );
    let different = pure(
        ExprKind::Binary { op: BinOp::Eq, lhs: Box::new(shade(7)), rhs: Box::new(shade(8)) },
        Type::Bool,
    );
    let r = buried(&demanding(vec![same, different]));
    assert_eq!(r.demands[0].kind, ExprKind::Literal(Literal::Bool(true)));
    assert_eq!(r.demands[1].kind, ExprKind::Literal(Literal::Bool(false)));

    // Two shades out of two different strata are not comparable here: the
    // checker will not let a stage-one burial hold a finished deep value at
    // all, so the case where the origins differ is proved where it lives, in
    // the encoding. `nether-ledger`'s `a_shades_origin_is_part_of_its_name`.
}

// ── the graph ───────────────────────────────────────────────────────────────

#[test]
fn everything_burial_names_is_named_by_its_content() {
    let r = buried(&demanding(vec![descending(reading("k")), descending(reading("j"))]));
    for (cairn, what) in &r.named {
        assert_eq!(*cairn, what.cairn(), "filed under a name that is not its own");
    }

    // And the graph is acyclic by construction: nothing names anything that
    // did not already exist when it was made. The one exception is the source
    // itself, which burial was handed and did not make.
    let mut made: Vec<Cairn> = vec![source()];
    for (cairn, what) in &r.named {
        if let Stored::Node(node) = what {
            for referenced in node.references() {
                assert!(
                    made.contains(&referenced),
                    "a node names something that did not exist when it was made"
                );
            }
        }
        made.push(*cairn);
    }
}
