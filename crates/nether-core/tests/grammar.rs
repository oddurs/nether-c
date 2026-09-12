//! The proof for *nether-core: the IR*: the IR expresses every construct in
//! `spec/04-grammar.md`, and nothing else.
//!
//! Three tests, and they close the loop from both ends.
//!
//! 1. Every production named in §04 is accounted for by name, in a table that
//!    is compared against the productions actually in the file. Adding a
//!    production to the grammar fails this test until someone says what the IR
//!    does with it.
//! 2. Every form in the IR is reachable from one sample unit. The walk that
//!    proves it matches exhaustively, so adding a form to the IR fails to
//!    compile until it is both named and built here.
//! 3. The prelude and capability tables in `prim.rs` and `depth.rs` are read
//!    back out of `spec/09-prelude.md` and compared. A table typed out by hand
//!    is a table that can drift.

use std::collections::{BTreeMap, BTreeSet};

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Expr, ExprKind, Field, FuncDef, FuncId, GlobalDef,
    GlobalId, Literal, LocalDef, LocalId, Place, Prim, Proj, Refusal, Rite, Span, Stmt, StructDef,
    Type, UnOp, Unit,
};

// ── the sample unit ─────────────────────────────────────────────────────────
//
// What it stands for, near enough that a reader can check it:
//
//   struct Header { I64 len; Bytes tag; };
//   typedef I64[16] Row;                  // an alias; it does not survive
//
//   Str name = "kernel.nc";
//
//   I64 head_len(Header h) @0 { return h.len; }
//
//   Answer<Bytes> load(Str path, Row window) @3
//   {
//     I64 total = 0;
//     for (I64 i = 0; i < 16; i += 1) {
//       if (window[i] < 0) { continue; }
//       total += window[i];
//     }
//     b"nc";                              // deposits
//     while (true) { break; }
//     Shade<Bytes> hidden = descend net { shade must(get("https://example.invalid")) };
//     Cairn id = seal hidden;
//     Bytes body = opaque look hidden;
//     Bool ok = refusal(read(path)) == absent && !(total == 0);
//     return read(path);
//   }
//
//   demand load(name, window);

fn span() -> Span {
    Span::default()
}

fn e(kind: ExprKind, ty: Type, depth: Depth) -> Expr {
    Expr { kind, ty, depth, span: span() }
}

fn pure(kind: ExprKind, ty: Type) -> Expr {
    e(kind, ty, Depth::PURE)
}

fn int(n: i64) -> Expr {
    pure(ExprKind::Literal(Literal::Int(n)), Type::Int)
}

fn local(id: u32, ty: Type) -> Expr {
    pure(ExprKind::Local(LocalId(id)), ty)
}

fn row() -> Type {
    Type::Array { elem: Box::new(Type::Int), len: Some(16) }
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

/// `read(path)` — at depth 3, which is where it leaves a hole.
fn read_path() -> Expr {
    e(
        ExprKind::Call {
            callee: Box::new(prim(Prim::Read, vec![Type::Str], answer_bytes())),
            args: vec![local(0, Type::Str)],
        },
        answer_bytes(),
        Depth::DISK,
    )
}

/// Every binary operator, folded into one expression. The grammar gives each
/// precedence level its own production; this is what they all lower to.
fn every_binop() -> Expr {
    const OPS: [BinOp; 10] = [
        BinOp::Add,
        BinOp::Sub,
        BinOp::Mul,
        BinOp::Div,
        BinOp::Rem,
        BinOp::BitAnd,
        BinOp::BitOr,
        BinOp::BitXor,
        BinOp::Shl,
        BinOp::Shr,
    ];
    const COMPARISONS: [BinOp; 6] =
        [BinOp::Eq, BinOp::Ne, BinOp::Lt, BinOp::Le, BinOp::Gt, BinOp::Ge];

    let arithmetic = OPS.into_iter().fold(int(1), |acc, op| {
        pure(ExprKind::Binary { op, lhs: Box::new(acc), rhs: Box::new(int(2)) }, Type::Int)
    });
    COMPARISONS.into_iter().fold(arithmetic, |acc, op| {
        let cmp =
            pure(ExprKind::Binary { op, lhs: Box::new(acc), rhs: Box::new(int(3)) }, Type::Bool);
        // Fold the Bool back to an I64 so the next comparison has an operand.
        pure(
            ExprKind::Select {
                cond: Box::new(cmp),
                then: Box::new(int(4)),
                otherwise: Box::new(int(5)),
            },
            Type::Int,
        )
    })
}

/// `for (I64 i = 0; i < 16; i += 1) { if (window[i] < 0) { continue; } total += window[i]; }`
///
/// The `for`'s counter is hoisted into the enclosing block; what is left is a
/// loop that carries its own step.
fn the_for_loop() -> Vec<Stmt> {
    let i = || local(2, Type::Int);
    let window_i = || {
        pure(ExprKind::Index { base: Box::new(local(1, row())), index: Box::new(i()) }, Type::Int)
    };

    let guard = pure(
        ExprKind::Select {
            cond: Box::new(pure(
                ExprKind::Binary {
                    op: BinOp::Lt,
                    lhs: Box::new(window_i()),
                    rhs: Box::new(int(0)),
                },
                Type::Bool,
            )),
            then: Box::new(pure(ExprKind::Continue, Type::Unit)),
            otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
        },
        Type::Unit,
    );

    // `total += window[i]` — a compound assignment is an assignment of a sum.
    let accumulate = pure(
        ExprKind::Assign {
            place: Place { local: LocalId(3), path: Vec::new() },
            value: Box::new(pure(
                ExprKind::Binary {
                    op: BinOp::Add,
                    lhs: Box::new(local(3, Type::Int)),
                    rhs: Box::new(window_i()),
                },
                Type::Int,
            )),
        },
        Type::Unit,
    );

    let body = pure(
        ExprKind::Block(Block {
            stmts: vec![Stmt::Expr(guard), Stmt::Expr(accumulate)],
            tail: None,
            span: span(),
        }),
        Type::Unit,
    );

    let step = pure(
        ExprKind::Assign {
            place: Place { local: LocalId(2), path: Vec::new() },
            value: Box::new(pure(
                ExprKind::Binary { op: BinOp::Add, lhs: Box::new(i()), rhs: Box::new(int(1)) },
                Type::Int,
            )),
        },
        Type::Unit,
    );

    vec![
        Stmt::Let { local: LocalId(2), value: int(0) },
        Stmt::Expr(pure(
            ExprKind::Loop { body: Box::new(body), step: Some(Box::new(step)) },
            Type::Unit,
        )),
    ]
}

/// The three rites that are not `descend`, and the descent that feeds them.
fn the_rites() -> Vec<Stmt> {
    let fetched = e(
        ExprKind::Call {
            callee: Box::new(prim(Prim::Get, vec![Type::Str], answer_bytes())),
            args: vec![pure(
                ExprKind::Literal(Literal::Str("https://example.invalid".into())),
                Type::Str,
            )],
        },
        answer_bytes(),
        Depth::NET,
    );
    let unwrapped = e(
        ExprKind::Call {
            callee: Box::new(prim(Prim::Must, vec![answer_bytes()], Type::Bytes)),
            args: vec![fetched],
        },
        Type::Bytes,
        Depth::NET,
    );
    let shaded = pure(
        ExprKind::Rite { rite: Rite::Shade, operand: Box::new(unwrapped) },
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) },
    );
    let descent = pure(
        ExprKind::Descend { capability: Capability::Net, body: Box::new(shaded) },
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) },
    );

    let hidden = || local(4, Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) });
    let sealed =
        pure(ExprKind::Rite { rite: Rite::Seal, operand: Box::new(hidden()) }, Type::Cairn);
    // What `seal` becomes once burial can finish it. There is no syntax for
    // one, which is why it is here and not in the sample source above.
    let named = pure(ExprKind::Literal(Literal::Cairn([0; 32])), Type::Cairn);
    let looked = e(
        ExprKind::Rite { rite: Rite::Look, operand: Box::new(hidden()) },
        Type::Bytes,
        Depth::NET,
    );
    let barrier = e(
        ExprKind::Rite { rite: Rite::Opaque, operand: Box::new(looked) },
        Type::Bytes,
        Depth::NET,
    );

    vec![
        Stmt::Let { local: LocalId(4), value: descent },
        Stmt::Let { local: LocalId(5), value: sealed },
        Stmt::Let { local: LocalId(6), value: barrier },
        Stmt::Expr(named),
    ]
}

/// `refusal(read(path)) == absent && !(total == 0)`
fn the_answer_check() -> Stmt {
    let which = e(
        ExprKind::Call {
            callee: Box::new(prim(Prim::Refusal, vec![answer_bytes()], Type::Refusal)),
            args: vec![read_path()],
        },
        Type::Refusal,
        Depth::DISK,
    );
    let is_absent = e(
        ExprKind::Binary {
            op: BinOp::Eq,
            lhs: Box::new(which),
            rhs: Box::new(pure(
                ExprKind::Literal(Literal::Refusal(Refusal::Absent)),
                Type::Refusal,
            )),
        },
        Type::Bool,
        Depth::DISK,
    );
    let nonzero = pure(
        ExprKind::Unary {
            op: UnOp::Not,
            operand: Box::new(pure(
                ExprKind::Binary {
                    op: BinOp::Eq,
                    lhs: Box::new(local(3, Type::Int)),
                    rhs: Box::new(int(0)),
                },
                Type::Bool,
            )),
        },
        Type::Bool,
    );
    // `a && b` is `if (a) { b } else { false }`.
    Stmt::Let {
        local: LocalId(7),
        value: e(
            ExprKind::Select {
                cond: Box::new(is_absent),
                then: Box::new(nonzero),
                otherwise: Box::new(pure(ExprKind::Literal(Literal::Bool(false)), Type::Bool)),
            },
            Type::Bool,
            Depth::DISK,
        ),
    }
}

fn load_body() -> Block {
    let mut stmts = vec![Stmt::Let { local: LocalId(3), value: int(0) }];
    stmts.extend(the_for_loop());

    // A bare expression statement whose value is not U0 deposits it.
    stmts.push(Stmt::Expr(pure(ExprKind::Literal(Literal::Bytes(b"nc".to_vec())), Type::Bytes)));

    // `while (true) { break; }`
    stmts.push(Stmt::Expr(pure(
        ExprKind::Loop {
            body: Box::new(pure(
                ExprKind::Select {
                    cond: Box::new(pure(ExprKind::Literal(Literal::Bool(true)), Type::Bool)),
                    then: Box::new(pure(ExprKind::Break, Type::Unit)),
                    otherwise: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
                },
                Type::Unit,
            )),
            step: None,
        },
        Type::Unit,
    )));

    stmts.push(Stmt::Expr(every_binop()));
    stmts.extend(the_rites());
    stmts.push(the_answer_check());

    // A negated, bit-flipped integer, so that the remaining unary operators
    // are somewhere.
    stmts.push(Stmt::Expr(pure(
        ExprKind::Unary {
            op: UnOp::Neg,
            operand: Box::new(pure(
                ExprKind::Unary { op: UnOp::BitNot, operand: Box::new(local(3, Type::Int)) },
                Type::Int,
            )),
        },
        Type::Int,
    )));

    // A call to another function in this unit, through a field projection, so
    // that both are exercised: `head_len(h)` is in `head_len` itself below.
    stmts.push(Stmt::Expr(e(
        ExprKind::Return(Some(Box::new(read_path()))),
        Type::Unit,
        Depth::DISK,
    )));

    Block { stmts, tail: None, span: span() }
}

fn head_len() -> FuncDef {
    let field = pure(
        ExprKind::Field { base: Box::new(local(0, Type::Struct("Header".into()))), index: 0 },
        Type::Int,
    );
    FuncDef {
        name: "head_len".into(),
        params: vec![LocalId(0)],
        ret: Type::Int,
        ret_depth: Depth::PURE,
        asserted_ret: None,
        latent: Depth::PURE,
        asserted_latent: Some(Depth::PURE),
        locals: vec![LocalDef {
            name: "h".into(),
            ty: Type::Struct("Header".into()),
            asserted: None,
            span: span(),
        }],
        body: Block {
            stmts: vec![Stmt::Expr(pure(ExprKind::Return(Some(Box::new(field))), Type::Unit))],
            tail: None,
            span: span(),
        },
        span: span(),
    }
}

fn load() -> FuncDef {
    let names = ["path", "window", "i", "total", "hidden", "id", "body", "ok"];
    let types = [
        Type::Str,
        row(),
        Type::Int,
        Type::Int,
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) },
        Type::Cairn,
        Type::Bytes,
        Type::Bool,
    ];
    let locals = names
        .into_iter()
        .zip(types)
        .map(|(name, ty)| LocalDef { name: name.into(), ty, asserted: None, span: span() })
        .collect();

    FuncDef {
        name: "load".into(),
        params: vec![LocalId(0), LocalId(1)],
        ret: answer_bytes(),
        ret_depth: Depth::DISK,
        asserted_ret: None,
        latent: Depth::DISK,
        asserted_latent: Some(Depth::DISK),
        locals,
        body: load_body(),
        span: span(),
    }
}

fn sample() -> Unit {
    // `demand load(name, window)` — through the function's own name, and a
    // unit-level binding, so both reference forms are exercised. There is no
    // `window` at unit level; a slice of nothing stands in for it, which the
    // grammar allows and the checker will not. This is a proof about shape.
    let call = e(
        ExprKind::Call {
            callee: Box::new(pure(
                ExprKind::Func(FuncId(1)),
                Type::Fn {
                    params: vec![Type::Str, row()],
                    latent: Depth::DISK,
                    result: Box::new(answer_bytes()),
                    result_depth: Depth::DISK,
                },
            )),
            args: vec![
                pure(ExprKind::Global(GlobalId(0)), Type::Str),
                pure(
                    ExprKind::Index {
                        base: Box::new(pure(
                            ExprKind::Global(GlobalId(1)),
                            Type::Array { elem: Box::new(row()), len: None },
                        )),
                        index: Box::new(int(0)),
                    },
                    row(),
                ),
            ],
        },
        answer_bytes(),
        Depth::DISK,
    );

    Unit {
        structs: vec![StructDef {
            name: "Header".into(),
            fields: vec![
                Field { name: "len".into(), ty: Type::Int, asserted: None, span: span() },
                Field {
                    name: "tag".into(),
                    ty: Type::Bytes,
                    asserted: Some(Depth::PURE),
                    span: span(),
                },
            ],
            span: span(),
        }],
        funcs: vec![head_len(), load()],
        globals: vec![
            GlobalDef {
                name: "name".into(),
                ty: Type::Str,
                asserted: None,
                value: pure(ExprKind::Literal(Literal::Str("kernel.nc".into())), Type::Str),
                span: span(),
            },
            GlobalDef {
                name: "windows".into(),
                ty: Type::Array { elem: Box::new(row()), len: None },
                asserted: None,
                value: pure(
                    ExprKind::Rite {
                        rite: Rite::Opaque,
                        operand: Box::new(pure(ExprKind::Literal(Literal::Unit), Type::Unit)),
                    },
                    Type::Unit,
                ),
                span: span(),
            },
        ],
        demands: vec![Demand { value: call, span: span() }],
    }
}

// ── the walk ────────────────────────────────────────────────────────────────
//
// Every match here is exhaustive and none of them has a wildcard, so a new
// form in the IR is a compile error in this file until it is named and built.

#[derive(Default)]
struct Seen(BTreeSet<String>);

impl Seen {
    fn mark(&mut self, kind: &str, form: &str) {
        self.0.insert(format!("{kind}::{form}"));
    }

    fn ty(&mut self, t: &Type) {
        self.mark(
            "Type",
            match t {
                Type::Unit => "Unit",
                Type::Bool => "Bool",
                Type::Int => "Int",
                Type::Bytes => "Bytes",
                Type::Str => "Str",
                Type::Cairn => "Cairn",
                Type::Shade { .. } => "Shade",
                Type::Answer(_) => "Answer",
                Type::Refusal => "Refusal",
                Type::Struct(_) => "Struct",
                Type::Array { .. } => "Array",
                Type::Fn { .. } => "Fn",
            },
        );
        match t {
            Type::Unit
            | Type::Bool
            | Type::Int
            | Type::Bytes
            | Type::Str
            | Type::Cairn
            | Type::Refusal
            | Type::Struct(_) => {}
            Type::Shade { inner, .. } | Type::Answer(inner) => self.ty(inner),
            Type::Array { elem, .. } => self.ty(elem),
            Type::Fn { params, result, .. } => {
                for p in params {
                    self.ty(p);
                }
                self.ty(result);
            }
        }
    }

    fn literal(&mut self, l: &Literal) {
        self.mark(
            "Literal",
            match l {
                Literal::Unit => "Unit",
                Literal::Bool(_) => "Bool",
                Literal::Int(_) => "Int",
                Literal::Bytes(_) => "Bytes",
                Literal::Str(_) => "Str",
                Literal::Refusal(_) => "Refusal",
                Literal::Cairn(_) => "Cairn",
            },
        );
    }

    fn place(&mut self, p: &Place) {
        for step in &p.path {
            match step {
                Proj::Field(_) => self.mark("Proj", "Field"),
                Proj::Index(i) => {
                    self.mark("Proj", "Index");
                    self.expr(i);
                }
            }
        }
    }

    fn block(&mut self, blk: &Block) {
        for s in &blk.stmts {
            match s {
                Stmt::Let { value, .. } => {
                    self.mark("Stmt", "Let");
                    self.expr(value);
                }
                Stmt::Expr(x) => {
                    self.mark("Stmt", "Expr");
                    self.expr(x);
                }
            }
        }
        if let Some(t) = &blk.tail {
            self.expr(t);
        }
    }

    fn expr(&mut self, x: &Expr) {
        self.ty(&x.ty);
        self.mark("ExprKind", form_of(&x.kind));
        match &x.kind {
            ExprKind::Literal(l) => self.literal(l),
            ExprKind::Local(_)
            | ExprKind::Global(_)
            | ExprKind::Func(_)
            | ExprKind::Prim(_)
            | ExprKind::Break
            | ExprKind::Continue => {}
            ExprKind::Call { callee, args } => {
                self.expr(callee);
                for a in args {
                    self.expr(a);
                }
            }
            ExprKind::Unary { op, operand } => {
                self.mark(
                    "UnOp",
                    match op {
                        UnOp::Neg => "Neg",
                        UnOp::Not => "Not",
                        UnOp::BitNot => "BitNot",
                    },
                );
                self.expr(operand);
            }
            ExprKind::Binary { op, lhs, rhs } => {
                self.mark("BinOp", binop_name(*op));
                self.expr(lhs);
                self.expr(rhs);
            }
            ExprKind::Field { base, .. } => {
                self.mark("Proj", "Field");
                self.expr(base);
            }
            ExprKind::Index { base, index } => {
                self.mark("Proj", "Index");
                self.expr(base);
                self.expr(index);
            }
            ExprKind::Assign { place, value } => {
                self.place(place);
                self.expr(value);
            }
            ExprKind::Block(blk) => self.block(blk),
            ExprKind::Select { cond, then, otherwise } => {
                self.expr(cond);
                self.expr(then);
                self.expr(otherwise);
            }
            ExprKind::Loop { body, step } => {
                self.expr(body);
                if let Some(s) = step {
                    self.expr(s);
                }
            }
            ExprKind::Return(v) => {
                if let Some(v) = v {
                    self.expr(v);
                }
            }
            ExprKind::Descend { body, .. } => self.expr(body),
            ExprKind::Rite { rite, operand } => {
                self.mark(
                    "Rite",
                    match rite {
                        Rite::Seal => "Seal",
                        Rite::Shade => "Shade",
                        Rite::Look => "Look",
                        Rite::Opaque => "Opaque",
                    },
                );
                self.expr(operand);
            }
        }
    }

    fn unit(&mut self, u: &Unit) {
        for s in &u.structs {
            for f in &s.fields {
                self.ty(&f.ty);
            }
        }
        for f in &u.funcs {
            self.ty(&f.ret);
            for l in &f.locals {
                self.ty(&l.ty);
            }
            self.block(&f.body);
        }
        for g in &u.globals {
            self.ty(&g.ty);
            self.expr(&g.value);
        }
        for d in &u.demands {
            self.expr(&d.value);
        }
    }
}

fn form_of(kind: &ExprKind) -> &'static str {
    match kind {
        ExprKind::Literal(_) => "Literal",
        ExprKind::Local(_) => "Local",
        ExprKind::Global(_) => "Global",
        ExprKind::Func(_) => "Func",
        ExprKind::Prim(_) => "Prim",
        ExprKind::Call { .. } => "Call",
        ExprKind::Unary { .. } => "Unary",
        ExprKind::Binary { .. } => "Binary",
        ExprKind::Field { .. } => "Field",
        ExprKind::Index { .. } => "Index",
        ExprKind::Assign { .. } => "Assign",
        ExprKind::Block(_) => "Block",
        ExprKind::Select { .. } => "Select",
        ExprKind::Loop { .. } => "Loop",
        ExprKind::Break => "Break",
        ExprKind::Continue => "Continue",
        ExprKind::Return(_) => "Return",
        ExprKind::Descend { .. } => "Descend",
        ExprKind::Rite { .. } => "Rite",
    }
}

fn binop_name(op: BinOp) -> &'static str {
    match op {
        BinOp::Add => "Add",
        BinOp::Sub => "Sub",
        BinOp::Mul => "Mul",
        BinOp::Div => "Div",
        BinOp::Rem => "Rem",
        BinOp::BitAnd => "BitAnd",
        BinOp::BitOr => "BitOr",
        BinOp::BitXor => "BitXor",
        BinOp::Shl => "Shl",
        BinOp::Shr => "Shr",
        BinOp::Eq => "Eq",
        BinOp::Ne => "Ne",
        BinOp::Lt => "Lt",
        BinOp::Le => "Le",
        BinOp::Gt => "Gt",
        BinOp::Ge => "Ge",
    }
}

/// Every form the IR has. Kept by hand so that the walk above and this list
/// have to agree: one of them is what the IR can express, the other is what
/// this file claims it can.
const EVERY_FORM: &[&str] = &[
    "BinOp::Add",
    "BinOp::BitAnd",
    "BinOp::BitOr",
    "BinOp::BitXor",
    "BinOp::Div",
    "BinOp::Eq",
    "BinOp::Ge",
    "BinOp::Gt",
    "BinOp::Le",
    "BinOp::Lt",
    "BinOp::Mul",
    "BinOp::Ne",
    "BinOp::Rem",
    "BinOp::Shl",
    "BinOp::Shr",
    "BinOp::Sub",
    "ExprKind::Assign",
    "ExprKind::Binary",
    "ExprKind::Block",
    "ExprKind::Break",
    "ExprKind::Call",
    "ExprKind::Continue",
    "ExprKind::Descend",
    "ExprKind::Field",
    "ExprKind::Func",
    "ExprKind::Global",
    "ExprKind::Index",
    "ExprKind::Literal",
    "ExprKind::Local",
    "ExprKind::Loop",
    "ExprKind::Prim",
    "ExprKind::Return",
    "ExprKind::Rite",
    "ExprKind::Select",
    "ExprKind::Unary",
    "Literal::Bool",
    "Literal::Cairn",
    "Literal::Bytes",
    "Literal::Int",
    "Literal::Refusal",
    "Literal::Str",
    "Literal::Unit",
    "Proj::Field",
    "Proj::Index",
    "Rite::Look",
    "Rite::Opaque",
    "Rite::Seal",
    "Rite::Shade",
    "Stmt::Expr",
    "Stmt::Let",
    "Type::Answer",
    "Type::Array",
    "Type::Bool",
    "Type::Bytes",
    "Type::Cairn",
    "Type::Fn",
    "Type::Int",
    "Type::Refusal",
    "Type::Shade",
    "Type::Str",
    "Type::Struct",
    "Type::Unit",
    "UnOp::BitNot",
    "UnOp::Neg",
    "UnOp::Not",
];

#[test]
fn every_form_in_the_ir_is_exercised() {
    let mut seen = Seen::default();
    seen.unit(&sample());

    let expected: BTreeSet<String> = EVERY_FORM.iter().map(|s| (*s).to_string()).collect();
    let missing: Vec<_> = expected.difference(&seen.0).collect();
    let extra: Vec<_> = seen.0.difference(&expected).collect();

    assert!(missing.is_empty(), "the sample unit never builds: {missing:?}");
    assert!(extra.is_empty(), "built but not listed in EVERY_FORM: {extra:?}");
}

// ── §04, read back ──────────────────────────────────────────────────────────

fn spec(section: &str) -> String {
    let path = format!("{}/../../spec/{section}", env!("CARGO_MANIFEST_DIR"));
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("cannot read {path}: {e}"))
}

/// Every production in §04, and what the IR does with it.
const COVERAGE: &[(&str, &str)] = &[
    ("unit", "Unit"),
    ("item", "Unit's four vectors"),
    ("demand_stmt", "Demand"),
    ("struct_decl", "StructDef"),
    ("field", "Field"),
    ("typedef_decl", "folded: an alias is replaced by the type it names"),
    ("func_decl", "FuncDef"),
    ("params", "FuncDef::params"),
    ("param", "LocalDef"),
    ("latent", "FuncDef::asserted"),
    ("let_decl", "GlobalDef at unit level, Stmt::Let inside a block"),
    ("type", "Type, with the written depth in Asserted"),
    ("type_atom", "Type"),
    ("block", "Block"),
    ("stmt", "Stmt, once if/while/for have become ExprKind"),
    ("expr", "Expr"),
    ("assign", "ExprKind::Assign"),
    ("assign_op", "folded: `x op= e` is an assignment of `x op e`"),
    ("logical_or", "folded: ExprKind::Select, which short-circuits"),
    ("logical_and", "folded: ExprKind::Select, which short-circuits"),
    ("bit_or", "BinOp::BitOr"),
    ("bit_xor", "BinOp::BitXor"),
    ("bit_and", "BinOp::BitAnd"),
    ("equality", "BinOp::Eq, BinOp::Ne"),
    ("relational", "BinOp::Lt, Le, Gt, Ge"),
    ("shift", "BinOp::Shl, BinOp::Shr"),
    ("additive", "BinOp::Add, BinOp::Sub"),
    ("multiply", "BinOp::Mul, Div, Rem"),
    ("unary", "ExprKind::Unary and ExprKind::Rite"),
    ("postfix", "ExprKind::Call, Index, Field"),
    ("call_suffix", "ExprKind::Call"),
    ("index_suffix", "ExprKind::Index"),
    ("field_suffix", "ExprKind::Field"),
    ("args", "ExprKind::Call's args"),
    ("primary", "ExprKind::Literal, Local, Global, Func, Prim, Block"),
    ("descend_expr", "ExprKind::Descend"),
];

fn productions(markdown: &str) -> BTreeSet<String> {
    let mut found = BTreeSet::new();
    let mut in_ebnf = false;
    for line in markdown.lines() {
        if line.starts_with("```") {
            in_ebnf = line.trim() == "```ebnf";
            continue;
        }
        if !in_ebnf {
            continue;
        }
        let Some((lhs, _)) = line.split_once(":=") else { continue };
        let name = lhs.trim();
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_lowercase() || c == '_') {
            found.insert(name.to_string());
        }
    }
    found
}

#[test]
fn every_production_in_section_04_has_a_form() {
    let in_spec = productions(&spec("04-grammar.md"));
    assert!(!in_spec.is_empty(), "found no productions; the grammar moved");

    let covered: BTreeSet<String> = COVERAGE.iter().map(|(p, _)| (*p).to_string()).collect();
    let missing: Vec<_> = in_spec.difference(&covered).collect();
    let stale: Vec<_> = covered.difference(&in_spec).collect();

    assert!(missing.is_empty(), "§04 has productions the IR says nothing about: {missing:?}");
    assert!(stale.is_empty(), "COVERAGE names productions §04 no longer has: {stale:?}");
}

// ── §09, read back ──────────────────────────────────────────────────────────

/// Every `name(args) @d;` in a code block, as the specification writes it.
fn prelude_signatures(markdown: &str) -> BTreeMap<String, u8> {
    let mut found = BTreeMap::new();
    for line in markdown.lines() {
        let Some((before, after)) = line.rsplit_once(')') else { continue };
        let Some((head, _)) = before.split_once('(') else { continue };
        let Some(at) = after.find('@') else { continue };
        let Some(depth) = after[at + 1..].chars().next().and_then(|c| c.to_digit(10)) else {
            continue;
        };
        let name = head.rsplit([' ', '\t']).next().unwrap_or_default();
        if !name.is_empty() {
            found.insert(name.to_string(), u8::try_from(depth).unwrap());
        }
    }
    found
}

#[test]
fn the_prelude_is_the_one_in_section_09() {
    let in_spec = prelude_signatures(&spec("09-prelude.md"));
    let in_code: BTreeMap<String, u8> =
        Prim::ALL.iter().map(|p| (p.name().to_string(), p.latent().get())).collect();
    assert_eq!(in_code, in_spec, "the prelude table and §09 disagree");
}

/// The rows of §9.1: `| `name` | stratum | … |`.
fn capability_table(markdown: &str) -> BTreeMap<String, u8> {
    let mut found = BTreeMap::new();
    for line in markdown.lines() {
        let mut cells = line.split('|').map(str::trim);
        let (Some(""), Some(name), Some(stratum)) = (cells.next(), cells.next(), cells.next())
        else {
            continue;
        };
        let Some(name) = name.strip_prefix('`').and_then(|n| n.strip_suffix('`')) else {
            continue;
        };
        let Ok(stratum) = stratum.parse::<u8>() else { continue };
        found.insert(name.to_string(), stratum);
    }
    found
}

#[test]
fn the_capabilities_are_the_ones_in_section_09() {
    let in_spec = capability_table(&spec("09-prelude.md"));
    let in_code: BTreeMap<String, u8> =
        Capability::ALL.iter().map(|c| (c.name().to_string(), c.stratum().get())).collect();
    assert_eq!(in_code, in_spec, "the capability table and §9.1 disagree");
}

#[test]
fn the_refusals_are_the_ones_in_section_09() {
    let markdown = spec("09-prelude.md");
    for r in Refusal::ALL {
        assert!(
            markdown.contains(&format!("{r};")) || markdown.contains(&format!("{r}\n")),
            "§09 does not bind `{r}`"
        );
    }
}
