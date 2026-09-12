//! The proof for *seal, shade and look*: the Orpheus rule behaves as the codex
//! settled it, including the error text.
//!
//! The error text is not copied here. It is read out of `spec/01-strata.md`
//! §1.6 and compared character for character, so the specification is the
//! fixture and a change to either side fails the build.

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Expr, ExprKind, FaultKind, FuncDef, FuncId, Literal,
    LocalDef, LocalId, Prim, Rite, Span, Stmt, Type, Unit, check, report,
};

// ── the file §1.6's error is about ──────────────────────────────────────────
//
//   Shade<Bytes> reply = descend net { shade must(get("…/index.json")) };
//   …
//   I64   n       = len(look(reply));
//
// Thirteen lines of nothing, so that the interesting one is line 14 and the
// location the renderer computes is the location the specification prints.

const FILLER: &str = "//\n";
const LOOK_LINE: &str = "I64   n       = len(look(reply));\n";
const URL: &str = "https://example.invalid/index.json";

fn source() -> String {
    FILLER.repeat(13) + LOOK_LINE
}

/// The span of `look(reply)` inside [`source`].
fn look_span() -> Span {
    let start = FILLER.len() * 13 + LOOK_LINE.find("look(reply)").expect("the line moved");
    Span { start: u32::try_from(start).unwrap(), end: u32::try_from(start).unwrap() + 11 }
}

// ── building ────────────────────────────────────────────────────────────────

fn e(kind: ExprKind, ty: Type, depth: Depth) -> Expr {
    Expr { kind, ty, depth, span: Span::default() }
}

fn pure(kind: ExprKind, ty: Type) -> Expr {
    e(kind, ty, Depth::PURE)
}

fn answer_bytes() -> Type {
    Type::Answer(Box::new(Type::Bytes))
}

fn shade_of_bytes(origin: Depth) -> Type {
    Type::Shade { origin, inner: Box::new(Type::Bytes) }
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

fn descend(capability: Capability, body: Expr) -> Expr {
    let (ty, depth) = (body.ty.clone(), body.depth);
    e(ExprKind::Descend { capability, body: Box::new(body) }, ty, depth)
}

fn rite(r: Rite, operand: Expr, ty: Type, depth: Depth) -> Expr {
    e(ExprKind::Rite { rite: r, operand: Box::new(operand) }, ty, depth)
}

fn reply(depth: Depth) -> Expr {
    e(ExprKind::Local(LocalId(0)), shade_of_bytes(Depth::NET), depth)
}

/// `descend net { shade must(get(URL)) }` — the binding the error points back
/// to. The whole thing is at depth 0: a shade is pure, whatever it holds.
fn the_shading() -> Expr {
    let fetched = call(
        prim(Prim::Get, vec![Type::Str], answer_bytes()),
        vec![pure(ExprKind::Literal(Literal::Str(URL.into())), Type::Str)],
        answer_bytes(),
        Depth::NET,
    );
    let unwrapped = call(
        prim(Prim::Must, vec![answer_bytes()], Type::Bytes),
        vec![fetched],
        Type::Bytes,
        Depth::NET,
    );
    descend(Capability::Net, rite(Rite::Shade, unwrapped, shade_of_bytes(Depth::NET), Depth::PURE))
}

/// `look(reply)`, at the span §1.6 underlines.
fn the_look() -> Expr {
    let mut looked = rite(Rite::Look, reply(Depth::PURE), Type::Bytes, Depth::NET);
    looked.span = look_span();
    looked
}

/// `len(look(reply))`.
fn the_statement(looked: Expr) -> Stmt {
    Stmt::Expr(call(
        prim(Prim::Len, vec![Type::Bytes], Type::Int),
        vec![looked],
        Type::Int,
        Depth::NET,
    ))
}

fn stamp(stmts: Vec<Stmt>, locals: Vec<LocalDef>, latent: Depth) -> Unit {
    let mut all = vec![LocalDef {
        name: "reply".into(),
        ty: shade_of_bytes(Depth::NET),
        asserted: None,
        span: Span::default(),
    }];
    all.extend(locals);
    let mut body = vec![Stmt::Let { local: LocalId(0), value: the_shading() }];
    body.extend(stmts);
    Unit {
        funcs: vec![FuncDef {
            name: "stamp".into(),
            params: Vec::new(),
            ret: Type::Unit,
            ret_depth: Depth::PURE,
            asserted_ret: None,
            latent,
            asserted_latent: None,
            locals: all,
            body: Block { stmts: body, tail: None, span: Span::default() },
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

// ── the error text ──────────────────────────────────────────────────────────

/// The fenced block in §1.6 that starts with `error:`.
fn the_error_in_the_codex() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/01-strata.md");
    let text = std::fs::read_to_string(path).expect("cannot read spec/01-strata.md");
    let mut block: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        // Any fence, of any language: a ```c that is treated as content puts
        // the scanner inside out for the rest of the file.
        if line.starts_with("```") {
            if inside && block.first().is_some_and(|l| l.starts_with("error:")) {
                return block.join("\n");
            }
            inside = !inside;
            block.clear();
            continue;
        }
        if inside {
            block.push(line);
        }
    }
    panic!("§1.6 no longer prints an error");
}

#[test]
fn the_error_is_the_one_printed_in_the_codex() {
    let unit = stamp(vec![the_statement(the_look())], Vec::new(), Depth::PURE);
    let faults = check(&unit);
    assert_eq!(faults.len(), 1, "{faults:?}");
    assert_eq!(faults[0].kind, FaultKind::Orpheus { origin: Depth::NET, ambient: Depth::PURE });

    let printed = report(&faults[0].diagnostic(), &source(), "stamp.nc");
    assert_eq!(printed.trim_end(), the_error_in_the_codex());
}

#[test]
fn the_blame_is_the_call_that_went_there_and_not_the_binding() {
    let unit = stamp(vec![the_statement(the_look())], Vec::new(), Depth::PURE);
    let blame = check(&unit).remove(0).blame.expect("nothing was blamed");
    assert_eq!(blame.what, "get", "the shade came from `get`, three lines up");
}

// ── the rule ────────────────────────────────────────────────────────────────

#[test]
fn a_shade_may_be_carried_up_out_of_any_stratum() {
    // Binding it at the surface is the whole point: `descend net { shade … }`
    // is depth 0, so nothing about holding it requires the network.
    assert!(check(&stamp(Vec::new(), Vec::new(), Depth::PURE)).is_empty());
}

#[test]
fn a_shade_may_be_stored_passed_compared_and_sealed() {
    let sealed = rite(Rite::Seal, reply(Depth::PURE), Type::Cairn, Depth::PURE);
    let copied = reply(Depth::PURE);
    let compared = e(
        ExprKind::Binary {
            op: BinOp::Eq,
            lhs: Box::new(reply(Depth::PURE)),
            rhs: Box::new(reply(Depth::PURE)),
        },
        Type::Bool,
        Depth::PURE,
    );

    let mut unit = stamp(
        vec![
            Stmt::Let { local: LocalId(1), value: sealed },
            Stmt::Let { local: LocalId(2), value: copied },
            Stmt::Expr(compared),
        ],
        vec![
            LocalDef {
                name: "witness".into(),
                ty: Type::Cairn,
                asserted: Some(Depth::PURE),
                span: Span::default(),
            },
            LocalDef {
                name: "again".into(),
                ty: shade_of_bytes(Depth::NET),
                asserted: None,
                span: Span::default(),
            },
        ],
        Depth::PURE,
    );

    // And passed: `keep(reply)`.
    unit.funcs.push(FuncDef {
        name: "keep".into(),
        params: vec![LocalId(0)],
        ret: Type::Unit,
        ret_depth: Depth::PURE,
        asserted_ret: None,
        latent: Depth::PURE,
        asserted_latent: None,
        locals: vec![LocalDef {
            name: "s".into(),
            ty: shade_of_bytes(Depth::NET),
            asserted: None,
            span: Span::default(),
        }],
        body: Block { stmts: Vec::new(), tail: None, span: Span::default() },
        span: Span::default(),
    });
    let keep = pure(
        ExprKind::Func(FuncId(1)),
        Type::Fn {
            params: vec![shade_of_bytes(Depth::NET)],
            latent: Depth::PURE,
            result: Box::new(Type::Unit),
            result_depth: Depth::PURE,
        },
    );
    unit.funcs[0].body.stmts.push(Stmt::Expr(call(
        keep,
        vec![reply(Depth::PURE)],
        Type::Unit,
        Depth::PURE,
    )));

    let faults = check(&unit);
    assert!(faults.is_empty(), "{faults:?}");
}

#[test]
fn you_may_only_look_by_going_back_down() {
    let inside = descend(Capability::Net, the_look());
    let unit = stamp(vec![the_statement(inside)], Vec::new(), Depth::PURE);
    let faults = check(&unit);
    assert!(faults.is_empty(), "{faults:?}");
}

#[test]
fn one_stratum_short_is_short() {
    // `disk` is 3 and the shade came from 5. Descending is not enough; it has
    // to be the descent that reaches where the value came from.
    let not_far_enough = descend(Capability::Disk, the_look());
    let unit = stamp(vec![the_statement(not_far_enough)], Vec::new(), Depth::PURE);
    assert_eq!(
        check(&unit).into_iter().map(|f| f.kind).collect::<Vec<_>>(),
        vec![FaultKind::Orpheus { origin: Depth::NET, ambient: Depth::DISK }]
    );
}

#[test]
fn deeper_than_the_origin_is_fine() {
    let deeper = descend(Capability::NetWrite, the_look());
    let unit = stamp(vec![the_statement(deeper)], Vec::new(), Depth::PURE);
    assert!(check(&unit).is_empty());
}

// ── the two forms that were rejected ────────────────────────────────────────

#[test]
fn it_does_not_stain_the_enclosing_scope() {
    // A legal look inside a descent leaves the ambient depth where it found
    // it. The `read` after it still has to be paid for, and the function ends
    // up asking its caller for `disk` — stratum 3, not stratum 5. Had the
    // descent re-stained the scope, the `read` would have been sitting inside
    // stratum 5 and the function would ask for nothing at all.
    // `spec/90-rationale.md` §90.2.
    let legal = descend(Capability::Net, the_look());
    let after = call(
        prim(Prim::Read, vec![Type::Str], answer_bytes()),
        vec![pure(ExprKind::Literal(Literal::Str("k".into())), Type::Str)],
        answer_bytes(),
        Depth::DISK,
    );
    let stmts = vec![the_statement(legal), Stmt::Expr(after)];

    let asks_for_disk = stamp(stmts.clone(), Vec::new(), Depth::DISK);
    let faults = check(&asks_for_disk);
    assert!(faults.is_empty(), "{faults:?}");

    let asks_for_nothing = stamp(stmts, Vec::new(), Depth::PURE);
    assert_eq!(
        check(&asks_for_nothing).into_iter().map(|f| f.kind).collect::<Vec<_>>(),
        vec![FaultKind::Stated { derived: Depth::DISK, stated: Depth::PURE }]
    );
}

#[test]
fn it_does_not_taint_the_binding() {
    // The illegal look is one fault and nothing else. `reply` is still a
    // depth-0 shade afterwards, and sealing it still gives `Cairn@0`.
    let sealed = rite(Rite::Seal, reply(Depth::PURE), Type::Cairn, Depth::PURE);
    let unit = stamp(
        vec![the_statement(the_look()), Stmt::Let { local: LocalId(1), value: sealed }],
        vec![LocalDef {
            name: "witness".into(),
            ty: Type::Cairn,
            asserted: Some(Depth::PURE),
            span: Span::default(),
        }],
        Depth::PURE,
    );
    assert_eq!(
        check(&unit).into_iter().map(|f| f.kind).collect::<Vec<_>>(),
        vec![FaultKind::Orpheus { origin: Depth::NET, ambient: Depth::PURE }]
    );
}

// ── seal ────────────────────────────────────────────────────────────────────

#[test]
fn seal_on_a_shade_is_legal_and_names_the_value_inside() {
    // §1.5. A cairn is a name, and a name is pure: the result is `Cairn@0`
    // whether it names a literal or a fetch.
    let unit = Unit {
        demands: vec![Demand {
            value: rite(Rite::Seal, the_shading(), Type::Cairn, Depth::PURE),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    assert!(check(&unit).is_empty());
}
