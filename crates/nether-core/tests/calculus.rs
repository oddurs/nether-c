//! The proof for *the bidirectional depth checker*: every rule in
//! `spec/02-calculus.md` derives what the specification says it derives, and
//! the two premises that are written down are enforced.
//!
//! The rule names are read back out of §02, so a twelfth rule fails this file
//! until somebody covers it — and so does an eleventh that goes missing. The
//! calculus fitting on one page is a constraint rather than an observation.

use std::collections::BTreeSet;

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Expr, ExprKind, Fault, FaultKind, FuncDef, FuncId,
    GlobalDef, GlobalId, Held, Literal, LocalDef, LocalId, Place, Prim, Proj, Rite, Span, Stmt,
    Type, UnOp, Unit, check,
};

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

/// `read(path)` at depth 3, which is where it leaves a hole.
fn read(path: &str) -> Expr {
    call(
        prim(Prim::Read, vec![Type::Str], answer_bytes()),
        vec![str_lit(path)],
        answer_bytes(),
        Depth::DISK,
    )
}

/// `must(a)`, which keeps the depth and drops the `Answer`.
fn must(a: Expr) -> Expr {
    let depth = a.depth;
    call(prim(Prim::Must, vec![answer_bytes()], Type::Bytes), vec![a], Type::Bytes, depth)
}

fn descend(capability: Capability, body: Expr) -> Expr {
    let (ty, depth) = (body.ty.clone(), body.depth);
    e(ExprKind::Descend { capability, body: Box::new(body) }, ty, depth)
}

fn rite(r: Rite, operand: Expr, ty: Type, depth: Depth) -> Expr {
    e(ExprKind::Rite { rite: r, operand: Box::new(operand) }, ty, depth)
}

fn local(id: u32, ty: Type, depth: Depth) -> Expr {
    e(ExprKind::Local(LocalId(id)), ty, depth)
}

fn binding(name: &str, ty: Type) -> LocalDef {
    LocalDef { name: name.into(), ty, asserted: None, span: Span::default() }
}

/// A unit whose only content is `demand value;`.
fn demanding(value: Expr) -> Unit {
    Unit { demands: vec![Demand { value, span: Span::default() }], ..Unit::default() }
}

/// A unit with one function `T@ret_depth f() @latent { stmts; tail }`.
fn in_a_function(
    locals: Vec<LocalDef>,
    stmts: Vec<Stmt>,
    tail: Option<Expr>,
    latent: Depth,
) -> Unit {
    let ret = tail.as_ref().map_or(Type::Unit, |t| t.ty.clone());
    let ret_depth = tail.as_ref().map_or(Depth::PURE, |t| t.depth);
    Unit {
        funcs: vec![FuncDef {
            name: "f".into(),
            params: Vec::new(),
            ret,
            ret_depth,
            asserted_ret: None,
            latent: Held::of(latent),
            asserted_latent: None,
            locals,
            body: Block { stmts, tail: tail.map(Box::new), span: Span::default() },
            span: Span::default(),
        }],
        ..Unit::default()
    }
}

fn clean(unit: &Unit) {
    let faults = check(unit);
    let said: Vec<String> = faults.iter().map(ToString::to_string).collect();
    assert!(faults.is_empty(), "expected a clean bill, got {said:?}");
}

fn kinds(unit: &Unit) -> Vec<FaultKind> {
    check(unit).into_iter().map(|f: Fault| f.kind).collect()
}

// ── the eleven rules ────────────────────────────────────────────────────────

/// [LIT] — `Γ ; δ ⊢ ℓ : τ@0`.
#[test]
fn lit_is_pure_and_that_is_why_pure_code_disappears() {
    clean(&demanding(int(7)));
    assert_eq!(
        kinds(&demanding(e(ExprKind::Literal(Literal::Int(7)), Type::Int, Depth::DISK))),
        vec![FaultKind::Stated { derived: Depth::PURE, stated: Depth::DISK }]
    );
}

/// [VAR] — `x : τ@d ∈ Γ`.
#[test]
fn var_reads_back_the_depth_it_was_bound_at() {
    // `Bytes src = descend disk { must(read("kernel.nc")) }; src`
    let bound = descend(Capability::Disk, must(read("kernel.nc")));
    let unit = in_a_function(
        vec![binding("src", Type::Bytes)],
        vec![Stmt::Let { local: LocalId(0), value: bound }],
        Some(local(0, Type::Bytes, Depth::DISK)),
        Depth::PURE,
    );
    clean(&unit);
}

/// [PRIM] — `⊕(e₁..eₙ) : τ@max(d₁..dₙ)`.
#[test]
fn prim_takes_the_maximum_of_its_parts() {
    let deep = descend(
        Capability::Disk,
        call(
            prim(Prim::Len, vec![Type::Bytes], Type::Int),
            vec![must(read("k"))],
            Type::Int,
            Depth::DISK,
        ),
    );
    let sum = e(
        ExprKind::Binary { op: BinOp::Add, lhs: Box::new(int(1)), rhs: Box::new(deep) },
        Type::Int,
        Depth::DISK,
    );
    clean(&demanding(sum));
}

/// [APP] — `max(dƒ, d_f, d_a)`, and the premise `≤ δ`.
#[test]
fn app_takes_the_latent_depth_and_the_arguments() {
    // dƒ: `read` is latent 3, so the call is depth 3 even on a literal path.
    clean(&demanding(descend(Capability::Disk, read("k"))));

    // d_a: `len` is latent 0, so all of the depth comes from the argument.
    let deep_arg = descend(Capability::Disk, must(read("k")));
    let len =
        call(prim(Prim::Len, vec![Type::Bytes], Type::Int), vec![deep_arg], Type::Int, Depth::DISK);
    clean(&demanding(len));
}

#[test]
fn app_refuses_what_was_never_granted() {
    // `read("k")` with no descent around it. δ is 0 at the top level of a file.
    assert_eq!(
        kinds(&demanding(read("k"))),
        vec![FaultKind::Ungranted { needed: Depth::DISK, held: Held::NONE }]
    );
}

#[test]
fn app_joins_the_depth_of_the_function_value_itself() {
    // §2.3: forgetting this term is the classic soundness hole in effect
    // systems that carry effects only on arrows. It is in the maximum and not
    // in the premise, so what is proved here is the depth of the result.
    //
    // Nothing in the language can produce a deep function today. No prelude
    // function returns one, there are no closures, and a shade of an arrow is
    // a shade whose origin is 0. The only way to hold one is to write it into
    // the IR by hand, which the unit below does and the checker says so — the
    // unbound local is the price of reaching the term at all.
    let arrow = Type::Fn {
        params: Vec::new(),
        latent: Held::of(Depth::PURE),
        result: Box::new(Type::Int),
        result_depth: Depth::PURE,
    };
    let deep_callee = local(0, arrow, Depth::NET);
    let unit = in_a_function(
        vec![binding("f", Type::Int)],
        Vec::new(),
        // The application is stated pure. A pure arrow it is; a pure
        // application it is not.
        Some(call(deep_callee, Vec::new(), Type::Int, Depth::PURE)),
        Depth::PURE,
    );
    assert!(
        kinds(&unit).contains(&FaultKind::Stated { derived: Depth::NET, stated: Depth::PURE }),
        "a pure arrow held in a deep value still makes the application deep"
    );
}

/// [ABS] — two depths come out of it, and the closure itself is pure.
#[test]
fn abs_gives_an_arrow_both_of_its_depths() {
    // `Bytes@3 load() @0 { descend disk { must(read("k")) } }`. It descends for
    // itself, so it asks its caller for nothing and still hands back something
    // deep.
    let unit = in_a_function(
        Vec::new(),
        Vec::new(),
        Some(descend(Capability::Disk, must(read("k")))),
        Depth::PURE,
    );
    clean(&unit);

    let arrow = Type::Fn {
        params: Vec::new(),
        latent: Held::of(Depth::PURE),
        result: Box::new(Type::Bytes),
        result_depth: Depth::DISK,
    };
    let mut called = unit;
    // Callable at the surface, which is the whole of 0122: the expression and
    // a name for the expression are legal in the same places.
    called.demands.push(Demand {
        value: call(
            pure(ExprKind::Func(nether_core::FuncId(0)), arrow.clone()),
            Vec::new(),
            Type::Bytes,
            Depth::DISK,
        ),
        span: Span::default(),
    });
    // And a reference to it is `(…)@0`: building a function that will touch
    // the disk does not touch the disk.
    called.demands.push(Demand {
        value: pure(ExprKind::Func(nether_core::FuncId(0)), arrow),
        span: Span::default(),
    });
    clean(&called);
}

/// [ABS] — a body that calls a prelude function bare asks for that stratum
/// instead, and its callers descend.
#[test]
fn abs_asks_for_what_the_body_did_not_descend_for() {
    let unit = in_a_function(Vec::new(), Vec::new(), Some(must(read("k"))), Depth::DISK);
    clean(&unit);

    let arrow = Type::Fn {
        params: Vec::new(),
        latent: Held::of(Depth::DISK),
        result: Box::new(Type::Bytes),
        result_depth: Depth::DISK,
    };
    let calling = |descended: bool| {
        let mut u = unit.clone();
        let called = call(
            pure(ExprKind::Func(nether_core::FuncId(0)), arrow.clone()),
            Vec::new(),
            Type::Bytes,
            Depth::DISK,
        );
        u.demands.push(Demand {
            value: if descended { descend(Capability::Disk, called) } else { called },
            span: Span::default(),
        });
        u
    };
    clean(&calling(true));
    assert_eq!(
        kinds(&calling(false)),
        vec![FaultKind::Ungranted { needed: Depth::DISK, held: Held::NONE }]
    );
}

/// [DESCEND] — the only rule that raises δ, and only inside its own premise.
#[test]
fn descend_raises_the_ambient_only_inside_its_own_body() {
    clean(&demanding(descend(Capability::Disk, read("k"))));

    // Nesting takes the maximum: `descend disk { descend net { … } }` has a
    // body at depth 5. `spec/01-strata.md` §1.3.
    let inner = call(
        prim(Prim::Get, vec![Type::Str], answer_bytes()),
        vec![str_lit("https://example.invalid")],
        answer_bytes(),
        Depth::NET,
    );
    clean(&demanding(descend(Capability::Disk, descend(Capability::Net, inner))));

    // And it does not leak out sideways: a second call at the top level is
    // still ungranted.
    let unit = Unit {
        demands: vec![
            Demand { value: descend(Capability::Disk, read("a")), span: Span::default() },
            Demand { value: read("b"), span: Span::default() },
        ],
        ..Unit::default()
    };
    assert_eq!(kinds(&unit), vec![FaultKind::Ungranted { needed: Depth::DISK, held: Held::NONE }]);
}

/// [SEAL] — `seal e : Cairn@0`, whatever `e` cost.
#[test]
fn seal_is_pure_whatever_it_names() {
    let deep = descend(Capability::Disk, must(read("k")));
    clean(&demanding(rite(Rite::Seal, deep, Type::Cairn, Depth::PURE)));
}

/// [SHADE] — `shade e : Shadeᵈ⟨τ⟩@0`, with `d` the depth of `e`.
#[test]
fn shade_carries_the_origin_it_actually_had() {
    let deep = descend(Capability::Net, must(net_get()));
    let shaded = rite(
        Rite::Shade,
        deep,
        Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) },
        Depth::PURE,
    );
    clean(&demanding(shaded));
}

#[test]
fn a_shade_cannot_claim_an_origin_its_value_never_had() {
    let deep = descend(Capability::Net, must(net_get()));
    let lying = rite(
        Rite::Shade,
        deep,
        Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Bytes) },
        Depth::PURE,
    );
    assert_eq!(
        kinds(&demanding(lying)),
        vec![FaultKind::Malformed("this shade claims an origin its value never had")]
    );
}

/// [LOOK] — the Orpheus rule, in one premise.
#[test]
fn look_is_legal_only_where_the_depth_is_already_held() {
    let shade_ty = Type::Shade { origin: Depth::NET, inner: Box::new(Type::Bytes) };
    let make =
        rite(Rite::Shade, descend(Capability::Net, must(net_get())), shade_ty.clone(), Depth::PURE);

    // Illegal at the surface. The two numbers in the message are the two the
    // specification's own error names. `spec/01-strata.md` §1.6.
    let looked_up_here =
        rite(Rite::Look, local(0, shade_ty.clone(), Depth::PURE), Type::Bytes, Depth::NET);
    let surfaced = in_a_function(
        vec![binding("reply", shade_ty.clone())],
        vec![Stmt::Let { local: LocalId(0), value: make.clone() }],
        Some(looked_up_here),
        Depth::PURE,
    );
    assert_eq!(kinds(&surfaced), vec![FaultKind::Orpheus { origin: Depth::NET, held: Held::NONE }]);

    // Legal by going back down.
    let looked_down_there = descend(
        Capability::Net,
        rite(Rite::Look, local(0, shade_ty.clone(), Depth::PURE), Type::Bytes, Depth::NET),
    );
    clean(&in_a_function(
        vec![binding("reply", shade_ty)],
        vec![Stmt::Let { local: LocalId(0), value: make }],
        Some(looked_down_there),
        Depth::PURE,
    ));
}

#[test]
fn look_inherits_the_depth_of_the_shade_value_as_well() {
    // A shade that was itself fetched is deep for two independent reasons.
    let shade_ty = Type::Shade { origin: Depth::STORE, inner: Box::new(Type::Bytes) };
    let shade_value = local(0, shade_ty.clone(), Depth::DISK);
    let looked = descend(Capability::Disk, rite(Rite::Look, shade_value, Type::Bytes, Depth::DISK));
    let unit = in_a_function(
        vec![binding("s", shade_ty)],
        vec![Stmt::Let {
            local: LocalId(0),
            value: descend(
                Capability::Disk,
                rite(
                    Rite::Shade,
                    descend(Capability::Store, store_read()),
                    Type::Shade { origin: Depth::STORE, inner: Box::new(Type::Bytes) },
                    Depth::PURE,
                ),
            ),
        }],
        Some(looked),
        Depth::PURE,
    );
    // `shade` is pure, so the binding is at 0, not 3: the descent around it
    // changes nothing. Stating 3 on the `look` is therefore wrong by one.
    assert!(
        kinds(&unit).iter().any(|k| matches!(k, FaultKind::Stated { .. })),
        "the shade value's own depth has to be in the join"
    );
}

/// [OPAQUE] — `opaque e : τ@d`. It changes what burial does, not what the
/// calculus derives.
#[test]
fn opaque_changes_nothing_but_evaluation() {
    let deep = descend(Capability::Disk, must(read("k")));
    clean(&demanding(rite(Rite::Opaque, deep, Type::Bytes, Depth::DISK)));
}

/// [DEMAND] — `demand e : U0@d`. The depth is kept and the value is dropped,
/// and a demand imposes no ambient bound of its own.
#[test]
fn demand_keeps_the_depth_and_drops_the_value() {
    clean(&demanding(descend(Capability::Disk, must(read("k")))));
}

fn net_get() -> Expr {
    call(
        prim(Prim::Get, vec![Type::Str], answer_bytes()),
        vec![str_lit("https://example.invalid")],
        answer_bytes(),
        Depth::NET,
    )
}

fn store_read() -> Expr {
    must(call(
        prim(Prim::FetchNode, vec![Type::Cairn], answer_bytes()),
        vec![pure(ExprKind::Literal(Literal::Bytes(Vec::new())), Type::Cairn)],
        answer_bytes(),
        Depth::STORE,
    ))
}

// ── assertions, which are checked and never coerced ─────────────────────────

#[test]
fn a_written_depth_that_disagrees_with_inference_is_an_error() {
    let unit = Unit {
        globals: vec![GlobalDef {
            name: "src".into(),
            ty: Type::Bytes,
            asserted: Some(Depth::PURE),
            value: descend(Capability::Disk, must(read("kernel.nc"))),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    assert_eq!(
        kinds(&unit),
        vec![FaultKind::Asserted { derived: Depth::DISK, asserted: Depth::PURE }]
    );
}

#[test]
fn a_written_depth_that_agrees_is_documentation() {
    let unit = Unit {
        globals: vec![GlobalDef {
            name: "src".into(),
            ty: Type::Bytes,
            asserted: Some(Depth::DISK),
            value: descend(Capability::Disk, must(read("kernel.nc"))),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    clean(&unit);
}

// ── the specification's own examples ────────────────────────────────────────

#[test]
fn section_one_point_three_checks() {
    // `Bytes@3 src = descend disk { read("kernel.nc") };` at the top level of
    // a file, where δ is 0. This is the example §2.1 used to forbid.
    let unit = Unit {
        globals: vec![GlobalDef {
            name: "src".into(),
            ty: answer_bytes(),
            asserted: Some(Depth::DISK),
            value: descend(Capability::Disk, read("kernel.nc")),
            span: Span::default(),
        }],
        ..Unit::default()
    };
    clean(&unit);
}

#[test]
fn section_one_point_five_checks() {
    // `Cairn id = seal src;` — `src : Bytes@3`, `id : Cairn@0`.
    let unit = in_a_function(
        vec![binding("src", Type::Bytes), binding("id", Type::Cairn)],
        vec![
            Stmt::Let { local: LocalId(0), value: descend(Capability::Disk, must(read("k"))) },
            Stmt::Let {
                local: LocalId(1),
                value: rite(
                    Rite::Seal,
                    local(0, Type::Bytes, Depth::DISK),
                    Type::Cairn,
                    Depth::PURE,
                ),
            },
        ],
        Some(local(1, Type::Cairn, Depth::PURE)),
        Depth::PURE,
    );
    clean(&unit);
}

// ── §02, read back ──────────────────────────────────────────────────────────

/// Every rule in §2.2, and the test above that derives it.
const RULES: &[(&str, &str)] = &[
    ("VAR", "var_reads_back_the_depth_it_was_bound_at"),
    ("LIT", "lit_is_pure_and_that_is_why_pure_code_disappears"),
    ("PRIM", "prim_takes_the_maximum_of_its_parts"),
    ("APP", "app_takes_the_latent_depth_and_the_arguments"),
    ("ABS", "abs_gives_an_arrow_both_of_its_depths"),
    ("DESCEND", "descend_raises_the_ambient_only_inside_its_own_body"),
    ("SEAL", "seal_is_pure_whatever_it_names"),
    ("SHADE", "shade_carries_the_origin_it_actually_had"),
    ("LOOK", "look_is_legal_only_where_the_depth_is_already_held"),
    ("OPAQUE", "opaque_changes_nothing_but_evaluation"),
    ("DEMAND", "demand_keeps_the_depth_and_drops_the_value"),
];

/// The rule names in §2.2: a bracketed name on a line that also carries the
/// inference bar, which is what separates a rule from a mention of one.
fn rules_in_spec() -> BTreeSet<String> {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/02-calculus.md");
    let text = std::fs::read_to_string(path).expect("cannot read spec/02-calculus.md");
    let mut found = BTreeSet::new();
    for line in text.lines() {
        if !line.contains('─') {
            continue;
        }
        let Some(open) = line.find('[') else { continue };
        let Some(close) = line[open..].find(']') else { continue };
        let name = &line[open + 1..open + close];
        if !name.is_empty() && name.chars().all(|c| c.is_ascii_uppercase()) {
            found.insert(name.to_string());
        }
    }
    found
}

#[test]
fn section_02_is_eleven_rules_and_every_one_of_them_is_derived_here() {
    let in_spec = rules_in_spec();
    assert_eq!(
        in_spec.len(),
        11,
        "§2.2 says `that is the whole system: eleven rules`, and has {}",
        in_spec.len()
    );
    let covered: BTreeSet<String> = RULES.iter().map(|(r, _)| (*r).to_string()).collect();
    assert_eq!(covered, in_spec, "the rules in §2.2 and the rules proved here have parted");
}

// ── [APP]: `dƒ ⊆ δ`, not `dƒ ≤ δ` ───────────────────────────────────────────

fn writing() -> Expr {
    let path = pure(ExprKind::Literal(Literal::Str("kernel.nc".into())), Type::Str);
    let bytes = pure(ExprKind::Literal(Literal::Bytes(Vec::new())), Type::Bytes);
    let answer = Type::Answer(Box::new(Type::Unit));
    call(
        prim(Prim::Write, vec![Type::Str, Type::Bytes], answer.clone()),
        vec![path, bytes],
        answer,
        Depth::DISK_WRITE,
    )
}

#[test]
fn a_deeper_descent_does_not_grant_a_shallower_capability() {
    // §1.3. `disk!` is 4 and `net` is 5, so a comparison would have let this
    // through, and a reader auditing the descents would have been told this
    // program touches the network. 0254.
    assert_eq!(
        kinds(&demanding(descend(Capability::Net, writing()))),
        vec![FaultKind::Ungranted { needed: Depth::DISK_WRITE, held: Held::of(Depth::NET) }]
    );
}

#[test]
fn the_capability_the_call_needs_is_the_one_that_grants_it() {
    let both = descend(Capability::Net, descend(Capability::DiskWrite, writing()));
    clean(&demanding(both));
}

#[test]
fn a_body_asks_for_what_its_descent_did_not_supply() {
    // Inside a function body an unmet capability is not a fault: it is what
    // the function asks of whoever calls it. [ABS]. The descent supplies
    // `net`, the write needs `disk!`, and the signature comes out `@4`.
    let unit = in_a_function(
        Vec::new(),
        vec![Stmt::Expr(descend(Capability::Net, writing()))],
        None,
        Depth::DISK_WRITE,
    );
    clean(&unit);
    assert_eq!(unit.funcs[0].latent, Held::of(Depth::DISK_WRITE));
}

// ── 0226: differential agreement with the mechanized model ──────────────────

/// Regression test found during 0226: a local declared *inside* a loop body via
/// `Stmt::Declare` or `Stmt::Let` is a fresh binding on each iteration, so
/// naming it at the end of the body does not freeze the next iteration's
/// assignment after re-declaration.
#[test]
fn a_local_declared_inside_a_loop_is_not_frozen_across_iterations() {
    let local = LocalDef {
        name: "h".into(),
        ty: Type::Int,
        asserted: None,
        span: Span { start: 1, end: 2 },
    };
    let assign = Expr {
        kind: ExprKind::Assign {
            place: Place { local: LocalId(0), path: Vec::new() },
            value: Box::new(int(1)),
        },
        ty: Type::Unit,
        depth: Depth::PURE,
        span: Span { start: 3, end: 4 },
    };
    let read_h = Expr {
        kind: ExprKind::Local(LocalId(0)),
        ty: Type::Int,
        depth: Depth::PURE,
        span: Span { start: 5, end: 6 },
    };
    let seal_h = Expr {
        kind: ExprKind::Rite { rite: Rite::Seal, operand: Box::new(read_h) },
        ty: Type::Cairn,
        depth: Depth::PURE,
        span: Span { start: 7, end: 8 },
    };
    let body = pure(
        ExprKind::Block(Block {
            stmts: vec![
                Stmt::Declare { local: LocalId(0) },
                Stmt::Expr(assign),
                Stmt::Expr(seal_h),
            ],
            tail: None,
            span: Span::default(),
        }),
        Type::Unit,
    );
    let looping = pure(ExprKind::Loop { body: Box::new(body), step: None }, Type::Unit);
    let unit = in_a_function(vec![local], vec![Stmt::Expr(looping)], None, Depth::PURE);
    assert_eq!(kinds(&unit), model_check(&unit));
    clean(&unit);
}

/// Regression test found during 0226: when one arm of a `Select` deepens a
/// local via `Assign`, that depth must not bleed into the sibling arm, and the
/// two arms must join after the `Select`.
#[test]
fn select_arms_join_local_depths_without_bleeding_across_branches() {
    let local_s = LocalDef {
        name: "s".into(),
        ty: Type::Int,
        asserted: None,
        span: Span { start: 1, end: 2 },
    };
    let local_pure = LocalDef {
        name: "pure_arm".into(),
        ty: Type::Int,
        asserted: Some(Depth::PURE),
        span: Span { start: 3, end: 4 },
    };
    let clock_call =
        call(prim(Prim::Clock, Vec::new(), Type::Int), Vec::new(), Type::Int, Depth::ENV);
    let deep_assign = descend(
        Capability::Env,
        e(
            ExprKind::Assign {
                place: Place { local: LocalId(0), path: Vec::new() },
                value: Box::new(clock_call),
            },
            Type::Unit,
            Depth::ENV,
        ),
    );
    let pure_assign = pure(
        ExprKind::Assign {
            place: Place { local: LocalId(0), path: Vec::new() },
            value: Box::new(int(0)),
        },
        Type::Unit,
    );
    let else_block = pure(
        ExprKind::Block(Block {
            stmts: vec![
                Stmt::Expr(pure_assign),
                Stmt::Let {
                    local: LocalId(1),
                    value: pure(ExprKind::Local(LocalId(0)), Type::Int),
                },
            ],
            tail: None,
            span: Span::default(),
        }),
        Type::Unit,
    );
    let branch = e(
        ExprKind::Select {
            cond: Box::new(pure(ExprKind::Literal(Literal::Bool(true)), Type::Bool)),
            then: Box::new(deep_assign),
            otherwise: Box::new(else_block),
        },
        Type::Unit,
        Depth::ENV,
    );
    let after = e(ExprKind::Local(LocalId(0)), Type::Int, Depth::ENV);
    let unit = in_a_function(
        vec![local_s, local_pure],
        vec![Stmt::Declare { local: LocalId(0) }, Stmt::Expr(branch)],
        Some(after),
        Depth::PURE,
    );
    assert_eq!(kinds(&unit), model_check(&unit));
    clean(&unit);
}

/// Clean-room reference model of the 11 rules in `spec/02-calculus.md` §2.2
/// (`proof/calculus.py`), plus §5.4 single-assignment naming and §5.6 depth
/// assertions. Written without sharing any code with `nether_core::check`.
fn model_check(unit: &Unit) -> Vec<FaultKind> {
    let mut m = Model {
        unit,
        faults: Vec::new(),
        gamma_global: Vec::new(),
        gamma_local: Vec::new(),
        named: Vec::new(),
        in_func: None,
        d_returns: Depth::PURE,
        d_latent: None,
    };
    m.reset_unit_frame();
    for g in &unit.globals {
        let d = m.eval_expr(&g.value, Held::NONE);
        m.gamma_global.push(d);
        m.check_asserted(d, g.asserted);
    }
    for f in &unit.funcs {
        m.eval_func(f);
    }
    m.reset_unit_frame();
    for d in &unit.demands {
        // [DEMAND]: `Γ ; ∅ ⊢ e : τ@d  ==>  Γ ⊢ demand e : U0@d`
        m.eval_expr(&d.value, Held::NONE);
    }
    m.faults
}

struct Model<'a> {
    unit: &'a Unit,
    faults: Vec<FaultKind>,
    gamma_global: Vec<Depth>,
    gamma_local: Vec<Option<Depth>>,
    named: Vec<Option<Span>>,
    in_func: Option<&'a FuncDef>,
    d_returns: Depth,
    d_latent: Option<Held>,
}

impl<'a> Model<'a> {
    fn reset_unit_frame(&mut self) {
        self.in_func = None;
        self.gamma_local = vec![None; self.unit.locals.len()];
        self.named = vec![None; self.unit.locals.len()];
    }

    fn check_stated(&mut self, derived: Depth, stated: Depth) {
        if derived != stated {
            self.faults.push(FaultKind::Stated { derived, stated });
        }
    }

    fn check_asserted(&mut self, derived: Depth, asserted: Option<Depth>) {
        if let Some(asserted) = asserted {
            if asserted != derived {
                self.faults.push(FaultKind::Asserted { derived, asserted });
            }
        }
    }

    fn bind_local(&mut self, id: LocalId, d: Depth) {
        let idx = id.0 as usize;
        if idx < self.gamma_local.len() {
            self.gamma_local[idx] = Some(d);
            self.named[idx] = None;
        } else {
            self.faults.push(FaultKind::Malformed("this binding names no local"));
        }
        let def = match self.in_func {
            Some(f) => f.locals.get(idx),
            None => self.unit.locals.get(idx),
        };
        if let Some(l) = def {
            self.check_asserted(d, l.asserted);
        }
    }

    fn mark_named(&mut self, expr: &Expr) {
        if let ExprKind::Local(id) = expr.kind {
            if let Some(slot) = self.named.get_mut(id.0 as usize) {
                if slot.is_none() {
                    *slot = Some(expr.span);
                }
            }
        }
    }

    /// [ABS] on a function declaration: parameters bound at depth 0, body
    /// evaluated under `δ = {0}`, accumulating unmet call strata into `dƒ` and
    /// return depths into `d_r`.
    fn eval_func(&mut self, f: &'a FuncDef) {
        self.in_func = Some(f);
        self.gamma_local = vec![None; f.locals.len()];
        self.named = vec![None; f.locals.len()];
        self.d_returns = Depth::PURE;
        self.d_latent = Some(Held::NONE);
        for p in &f.params {
            self.bind_local(*p, Depth::PURE);
        }
        let d_body = self.eval_block(&f.body, Held::NONE);
        let d_r = d_body.join(self.d_returns);
        let d_f = self.d_latent.take().unwrap_or(Held::NONE);

        self.check_stated(d_r, f.ret_depth);
        self.check_asserted(d_r, f.asserted_ret);
        if d_f != f.latent {
            self.faults.push(FaultKind::StatedLatent { derived: d_f, stated: f.latent });
        }
        if let Some(asserted) = f.asserted_latent {
            if asserted != d_f {
                self.faults.push(FaultKind::AssertedLatent { derived: d_f, asserted });
            }
        }
        self.in_func = None;
    }

    fn eval_block(&mut self, b: &Block, delta: Held) -> Depth {
        for s in &b.stmts {
            match s {
                Stmt::Let { local, value } => {
                    let d = self.eval_expr(value, delta);
                    self.bind_local(*local, d);
                }
                Stmt::Declare { local } => {
                    self.bind_local(*local, Depth::PURE);
                }
                Stmt::Expr(x) => {
                    self.eval_expr(x, delta);
                    self.mark_named(x);
                }
            }
        }
        match &b.tail {
            Some(t) => self.eval_expr(t, delta),
            None => Depth::PURE,
        }
    }

    #[expect(clippy::too_many_lines, reason = "eleven rules, one arm each")]
    fn eval_expr(&mut self, x: &Expr, delta: Held) -> Depth {
        let derived = match &x.kind {
            // [LIT]
            ExprKind::Literal(_) | ExprKind::Break | ExprKind::Continue => Depth::PURE,

            // [VAR]
            ExprKind::Local(id) => {
                if let Some(d) = self.gamma_local.get(id.0 as usize).copied().flatten() {
                    d
                } else {
                    self.faults.push(FaultKind::Malformed("this local is not bound here"));
                    x.depth
                }
            }
            ExprKind::Global(id) => {
                if let Some(d) = self.gamma_global.get(id.0 as usize).copied() {
                    d
                } else {
                    self.faults
                        .push(FaultKind::Malformed("this unit-level binding is not declared yet"));
                    x.depth
                }
            }

            // [ABS] value reference
            ExprKind::Func(id) => {
                if let Some(f) = self.unit.funcs.get(id.0 as usize) {
                    let params: Vec<Type> = f
                        .params
                        .iter()
                        .filter_map(|p| f.locals.get(p.0 as usize))
                        .map(|l| l.ty.clone())
                        .collect();
                    let expected = Type::Fn {
                        params,
                        latent: f.latent,
                        result: Box::new(f.ret.clone()),
                        result_depth: f.ret_depth,
                    };
                    if x.ty != expected {
                        self.faults.push(FaultKind::Malformed(
                            "this is not the signature of that function",
                        ));
                    }
                } else {
                    self.faults.push(FaultKind::Malformed("this names no function in the unit"));
                }
                Depth::PURE
            }
            ExprKind::Prim(p) => {
                let s = p.latent();
                match &x.ty {
                    Type::Fn { latent, result_depth, .. }
                        if *latent == Held::of(s) && *result_depth == s => {}
                    Type::Fn { .. } => {
                        self.faults.push(FaultKind::Malformed(
                            "this prelude function has another stratum",
                        ));
                    }
                    _ => {
                        self.faults.push(FaultKind::Malformed("a prelude function is a function"));
                    }
                }
                Depth::PURE
            }

            // [APP]: premise `dƒ ⊆ δ`, result `max(d_r, d_f, d_a)`
            ExprKind::Call { callee, args } => {
                let d_f = self.eval_expr(callee, delta);
                let mut d_a = Depth::PURE;
                for a in args {
                    d_a = d_a.join(self.eval_expr(a, delta));
                }
                for a in args {
                    self.mark_named(a);
                }
                let (d_fn_latent, d_r) = if let Type::Fn { latent, result_depth, .. } = &callee.ty {
                    (*latent, *result_depth)
                } else {
                    self.faults.push(FaultKind::Malformed("this is applied and is not a function"));
                    (Held::NONE, Depth::PURE)
                };
                if !d_fn_latent.subset_of(delta) {
                    if let Some(acc) = &mut self.d_latent {
                        *acc = acc.union(d_fn_latent);
                    } else {
                        let needed = d_fn_latent.without(delta).deepest();
                        self.faults.push(FaultKind::Ungranted { needed, held: delta });
                    }
                }
                d_r.join(d_f).join(d_a)
            }

            // [DESCEND]: `δ ∪ {s(κ)}`
            ExprKind::Descend { capability, body } => {
                self.eval_expr(body, delta.with(capability.stratum()))
            }

            // [SEAL], [SHADE], [LOOK], [OPAQUE]
            ExprKind::Rite { rite, operand } => {
                let d = self.eval_expr(operand, delta);
                if matches!(rite, Rite::Seal | Rite::Shade) {
                    self.mark_named(operand);
                }
                match rite {
                    Rite::Seal => Depth::PURE,
                    Rite::Shade => {
                        let expected =
                            Type::Shade { origin: d, inner: Box::new(operand.ty.clone()) };
                        if x.ty != expected {
                            self.faults.push(FaultKind::Malformed(
                                "this shade claims an origin its value never had",
                            ));
                        }
                        Depth::PURE
                    }
                    Rite::Look => {
                        if let Type::Shade { origin, .. } = &operand.ty {
                            if !delta.holds(*origin) {
                                self.faults
                                    .push(FaultKind::Orpheus { origin: *origin, held: delta });
                            }
                            origin.join(d)
                        } else {
                            self.faults.push(FaultKind::Malformed("`look` needs a shade"));
                            d
                        }
                    }
                    Rite::Opaque => d,
                }
            }

            // [PRIM]
            ExprKind::Unary { operand, .. } | ExprKind::Field { base: operand, .. } => {
                self.eval_expr(operand, delta)
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
                self.eval_expr(lhs, delta).join(self.eval_expr(rhs, delta))
            }
            ExprKind::Select { cond, then, otherwise } => {
                let d_c = self.eval_expr(cond, delta);
                let saved_named = self.named.clone();
                let saved_locals = self.gamma_local.clone();
                let d_t = self.eval_expr(then, delta);
                let named_t = std::mem::replace(&mut self.named, saved_named);
                let locals_t = std::mem::replace(&mut self.gamma_local, saved_locals);
                let d_e = self.eval_expr(otherwise, delta);
                for (a, b) in self.named.iter_mut().zip(&named_t) {
                    *a = a.or(*b);
                }
                for (a, b) in self.gamma_local.iter_mut().zip(&locals_t) {
                    *a = a.zip(*b).map(|(x, y)| x.join(y)).or(*a).or(*b);
                }
                d_c.join(d_t).join(d_e)
            }
            ExprKind::Loop { body, step } => {
                let mark = self.faults.len();
                self.eval_expr(body, delta);
                if let Some(s) = step {
                    self.eval_expr(s, delta);
                }
                self.faults.truncate(mark);
                let d_b = self.eval_expr(body, delta);
                match step {
                    Some(s) => d_b.join(self.eval_expr(s, delta)),
                    None => d_b,
                }
            }
            ExprKind::Return(v) => {
                let d = match v {
                    Some(inner) => {
                        let d = self.eval_expr(inner, delta);
                        self.mark_named(inner);
                        d
                    }
                    None => Depth::PURE,
                };
                self.d_returns = self.d_returns.join(d);
                d
            }
            ExprKind::Block(b) => self.eval_block(b, delta),
            ExprKind::Assign { place, value } => {
                let d_v = self.eval_expr(value, delta);
                let mut d_p = Depth::PURE;
                for p in &place.path {
                    if let Proj::Index(i) = p {
                        d_p = d_p.join(self.eval_expr(i, delta));
                    }
                }
                let idx = place.local.0 as usize;
                if let Some(Some(named)) = self.named.get(idx).copied() {
                    self.faults.push(FaultKind::Frozen { named });
                }
                if let Some(slot) = self.gamma_local.get_mut(idx) {
                    *slot = Some(slot.unwrap_or(Depth::PURE).join(d_v));
                } else {
                    self.faults.push(FaultKind::Malformed("this assigns to no local"));
                }
                d_v.join(d_p)
            }
        };
        self.check_stated(derived, x.depth);
        derived
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { usize::try_from(self.next() % (n as u64)).unwrap_or(0) }
    }

    fn diff_depth(&mut self, d: Depth) -> Depth {
        let step = u8::try_from(self.below(8)).unwrap_or(0);
        Depth::new((d.get() + 1 + step) % 9).unwrap_or(Depth::STORE)
    }

    fn capability(&mut self) -> Capability {
        Capability::ALL[self.below(8)]
    }
}

fn u32_of(n: usize) -> u32 {
    u32::try_from(n).unwrap_or(0)
}

const CAP_PRIMS: [(Capability, Prim); 8] = [
    (Capability::Store, Prim::HasNode),
    (Capability::Env, Prim::Clock),
    (Capability::Disk, Prim::Exists),
    (Capability::DiskWrite, Prim::Write),
    (Capability::Net, Prim::Get),
    (Capability::NetWrite, Prim::Post),
    (Capability::Entropy, Prim::Draw),
    (Capability::Unrecorded, Prim::CallForeign),
];

fn prim_call(cap_idx: usize, arg_expr: Expr) -> Expr {
    let (_, p) = CAP_PRIMS[cap_idx];
    let s = p.latent();
    let d_arg = arg_expr.depth;
    let (params, args, ret) = match p {
        Prim::HasNode => (
            vec![Type::Cairn],
            vec![e(
                ExprKind::Rite { rite: Rite::Seal, operand: Box::new(arg_expr) },
                Type::Cairn,
                Depth::PURE,
            )],
            Type::Bool,
        ),
        Prim::Clock => (Vec::new(), Vec::new(), Type::Int),
        Prim::Exists => (vec![Type::Str], vec![str_lit("f")], Type::Bool),
        Prim::Write => (
            vec![Type::Str, Type::Bytes],
            vec![str_lit("f"), pure(ExprKind::Literal(Literal::Bytes(vec![1])), Type::Bytes)],
            Type::Answer(Box::new(Type::Unit)),
        ),
        Prim::Get => (vec![Type::Str], vec![str_lit("u")], answer_bytes()),
        Prim::Post => (
            vec![Type::Str, Type::Bytes],
            vec![str_lit("u"), pure(ExprKind::Literal(Literal::Bytes(vec![2])), Type::Bytes)],
            answer_bytes(),
        ),
        Prim::Draw => (vec![Type::Int], vec![arg_expr], Type::Bytes),
        Prim::CallForeign => (
            vec![Type::Str, Type::Bytes],
            vec![str_lit("sym"), pure(ExprKind::Literal(Literal::Bytes(vec![3])), Type::Bytes)],
            answer_bytes(),
        ),
        _ => unreachable!(),
    };
    let d_call = args.iter().fold(s, |acc, a| acc.join(a.depth));
    let _ = d_arg;
    call(prim(p, params, ret.clone()), args, ret, d_call)
}

/// Generate a well-formed `Expr` of `Type::Int` whose stated `.depth` and
/// sub-expressions obey all 11 rules of §2.2 under `ambient`, optionally inside
/// a function accumulating `latent_out`.
#[expect(clippy::too_many_lines, reason = "covers all 11 rules of §2.2")]
fn gen_valid_int_expr(
    rng: &mut Rng,
    budget: u8,
    ambient: Held,
    globals: &[Depth],
    locals: &[(LocalId, Depth)],
    funcs: &[(FuncId, Held, Depth)],
    mut latent_out: Option<&mut Held>,
) -> Expr {
    if budget == 0 {
        return match rng.below(4) {
            0 if !locals.is_empty() => {
                let (id, d) = locals[rng.below(locals.len())];
                e(ExprKind::Local(id), Type::Int, d)
            }
            1 if !globals.is_empty() => {
                let idx = rng.below(globals.len());
                e(ExprKind::Global(GlobalId(u32_of(idx))), Type::Int, globals[idx])
            }
            _ => int(i64::try_from(rng.below(100)).unwrap_or(0)),
        };
    }

    match rng.below(11) {
        // [LIT]
        0 => int(i64::try_from(rng.below(1000)).unwrap_or(0)),
        // [VAR]
        1 => {
            if !locals.is_empty() && rng.below(2) == 0 {
                let (id, d) = locals[rng.below(locals.len())];
                e(ExprKind::Local(id), Type::Int, d)
            } else if !globals.is_empty() {
                let idx = rng.below(globals.len());
                e(ExprKind::Global(GlobalId(u32_of(idx))), Type::Int, globals[idx])
            } else {
                int(7)
            }
        }
        // [PRIM] Binary / Unary
        2 => {
            let lhs = gen_valid_int_expr(
                rng,
                budget - 1,
                ambient,
                globals,
                locals,
                funcs,
                latent_out.as_deref_mut(),
            );
            let rhs =
                gen_valid_int_expr(rng, budget - 1, ambient, globals, locals, funcs, latent_out);
            let d = lhs.depth.join(rhs.depth);
            let op = match rng.below(4) {
                0 => BinOp::Add,
                1 => BinOp::Sub,
                2 => BinOp::BitXor,
                _ => BinOp::Mul,
            };
            e(ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) }, Type::Int, d)
        }
        // [PRIM] Select
        3 => {
            let lhs = gen_valid_int_expr(
                rng,
                budget - 1,
                ambient,
                globals,
                locals,
                funcs,
                latent_out.as_deref_mut(),
            );
            let cond = e(
                ExprKind::Binary {
                    op: BinOp::Gt,
                    lhs: Box::new(lhs.clone()),
                    rhs: Box::new(int(0)),
                },
                Type::Bool,
                lhs.depth,
            );
            let then = gen_valid_int_expr(
                rng,
                budget - 1,
                ambient,
                globals,
                locals,
                funcs,
                latent_out.as_deref_mut(),
            );
            let otherwise =
                gen_valid_int_expr(rng, budget - 1, ambient, globals, locals, funcs, latent_out);
            let d = cond.depth.join(then.depth).join(otherwise.depth);
            e(
                ExprKind::Select {
                    cond: Box::new(cond),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                },
                Type::Int,
                d,
            )
        }
        // [DESCEND] + [APP]
        4 => {
            let cap_idx = rng.below(8);
            let (cap, _) = CAP_PRIMS[cap_idx];
            let inner_ambient = ambient.with(cap.stratum());
            let arg = gen_valid_int_expr(
                rng,
                budget - 1,
                inner_ambient,
                globals,
                locals,
                funcs,
                latent_out,
            );
            let deep = prim_call(cap_idx, arg.clone());
            let d_deep = deep.depth;
            let body = e(
                ExprKind::Block(Block {
                    stmts: vec![Stmt::Expr(deep)],
                    tail: Some(Box::new(e(
                        ExprKind::Unary { op: UnOp::Neg, operand: Box::new(arg) },
                        Type::Int,
                        d_deep.min(d_deep),
                    ))),
                    span: Span::default(),
                }),
                Type::Int,
                d_deep,
            );
            // Make the tail actually carry `d_deep` by calling `Clock` or using
            // a Select on the deep call when needed.
            let body = if cap == Capability::Env {
                call(prim(Prim::Clock, Vec::new(), Type::Int), Vec::new(), Type::Int, Depth::ENV)
            } else {
                let deep_call = prim_call(cap_idx, int(1));
                let cond = e(
                    ExprKind::Rite { rite: Rite::Seal, operand: Box::new(deep_call.clone()) },
                    Type::Cairn,
                    Depth::PURE,
                );
                let _ = (body, cond);
                // `Block` with `Stmt::Expr(deep_call)` and tail `int(1)` has depth 0!
                // Use `Index` or `Call(Prim::Abs)` or `Block` whose tail uses `Clock` or `len`:
                e(
                    ExprKind::Block(Block {
                        stmts: vec![Stmt::Expr(deep_call)],
                        tail: Some(Box::new(int(1))),
                        span: Span::default(),
                    }),
                    Type::Int,
                    Depth::PURE,
                )
            };
            let d = body.depth;
            e(ExprKind::Descend { capability: cap, body: Box::new(body) }, Type::Int, d)
        }
        // [SEAL] inside an expression
        5 => {
            let inner =
                gen_valid_int_expr(rng, budget - 1, ambient, globals, locals, funcs, latent_out);
            let sealed = e(
                ExprKind::Rite { rite: Rite::Seal, operand: Box::new(inner) },
                Type::Cairn,
                Depth::PURE,
            );
            e(
                ExprKind::Block(Block {
                    stmts: vec![Stmt::Expr(sealed)],
                    tail: Some(Box::new(int(2))),
                    span: Span::default(),
                }),
                Type::Int,
                Depth::PURE,
            )
        }
        // [SHADE] + [LOOK] under a matching [DESCEND]
        6 => {
            let cap_idx = rng.below(8);
            let (cap, _) = CAP_PRIMS[cap_idx];
            let inner = if cap == Capability::Env {
                descend(
                    Capability::Env,
                    call(
                        prim(Prim::Clock, Vec::new(), Type::Int),
                        Vec::new(),
                        Type::Int,
                        Depth::ENV,
                    ),
                )
            } else {
                gen_valid_int_expr(
                    rng,
                    budget - 1,
                    ambient.with(cap.stratum()),
                    globals,
                    locals,
                    funcs,
                    latent_out,
                )
            };
            let origin = inner.depth;
            let shaded = e(
                ExprKind::Rite { rite: Rite::Shade, operand: Box::new(inner) },
                Type::Shade { origin, inner: Box::new(Type::Int) },
                Depth::PURE,
            );
            let looked = e(
                ExprKind::Rite { rite: Rite::Look, operand: Box::new(shaded) },
                Type::Int,
                origin,
            );
            if let Some(need_cap) = Capability::at(origin) {
                if !ambient.holds(origin) {
                    return descend(need_cap, looked);
                }
            }
            looked
        }
        // [OPAQUE]
        7 => {
            let inner =
                gen_valid_int_expr(rng, budget - 1, ambient, globals, locals, funcs, latent_out);
            let d = inner.depth;
            e(ExprKind::Rite { rite: Rite::Opaque, operand: Box::new(inner) }, Type::Int, d)
        }
        // [APP] pure or latent function / prim call
        8 => {
            let a = gen_valid_int_expr(
                rng,
                budget - 1,
                ambient,
                globals,
                locals,
                funcs,
                latent_out.as_deref_mut(),
            );
            if !funcs.is_empty() && rng.below(2) == 0 {
                let (fid, f_lat, f_ret) = funcs[rng.below(funcs.len())];
                let fn_expr = pure(
                    ExprKind::Func(fid),
                    Type::Fn {
                        params: vec![Type::Int],
                        latent: f_lat,
                        result: Box::new(Type::Int),
                        result_depth: f_ret,
                    },
                );
                let d_call = f_ret.join(a.depth);
                let call_expr = call(fn_expr, vec![a], Type::Int, d_call);
                if f_lat.subset_of(ambient) {
                    call_expr
                } else if let Some(acc) = latent_out {
                    *acc = acc.union(f_lat);
                    call_expr
                } else {
                    let mut wrapped = call_expr;
                    for cap in Capability::ALL {
                        if f_lat.holds(cap.stratum()) && !ambient.holds(cap.stratum()) {
                            wrapped = descend(cap, wrapped);
                        }
                    }
                    wrapped
                }
            } else {
                let d = a.depth;
                call(prim(Prim::Abs, vec![Type::Int], Type::Int), vec![a], Type::Int, d)
            }
        }
        // [APP] inside a function body that intentionally leaves a latent debt
        9 if latent_out.is_some() => {
            let cap = rng.capability();
            let s = cap.stratum();
            let p_call = if cap == Capability::Env {
                call(prim(Prim::Clock, Vec::new(), Type::Int), Vec::new(), Type::Int, Depth::ENV)
            } else {
                let raw = prim_call((s.get() - 1) as usize, int(1));
                e(
                    ExprKind::Block(Block {
                        stmts: vec![Stmt::Expr(raw)],
                        tail: Some(Box::new(int(1))),
                        span: Span::default(),
                    }),
                    Type::Int,
                    Depth::PURE,
                )
            };
            if !ambient.holds(s) {
                if let Some(acc) = latent_out {
                    *acc = acc.union(Held::of(s));
                }
            }
            p_call
        }
        // [PRIM] Field / Index
        _ => {
            let idx =
                gen_valid_int_expr(rng, budget - 1, ambient, globals, locals, funcs, latent_out);
            let arr = pure(
                ExprKind::Literal(Literal::Bytes(vec![1, 2])),
                Type::Array { elem: Box::new(Type::Int), len: Some(2) },
            );
            let d = idx.depth;
            e(ExprKind::Index { base: Box::new(arr), index: Box::new(idx) }, Type::Int, d)
        }
    }
}

/// Generate a complete `Unit` (valid ~45% of the time; with one or more
/// targeted fault injections across all 8 `FaultKind` variants ~55% of the
/// time).
#[expect(clippy::too_many_lines, reason = "covers valid units and all 8 FaultKind classes")]
fn gen_unit(rng: &mut Rng) -> Unit {
    let mut unit = Unit::default();

    // 0..=2 globals
    let n_globals = rng.below(3);
    let mut global_depths = Vec::with_capacity(n_globals);
    for i in 0..n_globals {
        let val = gen_valid_int_expr(rng, 2, Held::NONE, &global_depths, &[], &[], None);
        let d = val.depth;
        global_depths.push(d);
        let asserted = if rng.below(3) == 0 { Some(d) } else { None };
        let base = u32_of(i) * 10;
        unit.globals.push(GlobalDef {
            name: format!("g{i}"),
            ty: Type::Int,
            asserted,
            value: val,
            span: Span { start: base, end: base + 4 },
        });
    }

    // 1..=2 functions
    let n_funcs = 1 + rng.below(2);
    let mut func_sigs: Vec<(FuncId, Held, Depth)> = Vec::with_capacity(n_funcs);
    for i in 0..n_funcs {
        let idx = u32_of(i);
        let param_def = LocalDef {
            name: "p".into(),
            ty: Type::Int,
            asserted: if rng.below(4) == 0 { Some(Depth::PURE) } else { None },
            span: Span { start: 100 + idx, end: 101 + idx },
        };
        let mut locals = vec![param_def];
        let mut bound = vec![(LocalId(0), Depth::PURE)];
        let mut latent = Held::NONE;
        let mut stmts = Vec::new();
        let mut d_returns = Depth::PURE;

        if rng.below(2) == 0 {
            let init = gen_valid_int_expr(
                rng,
                1,
                Held::NONE,
                &global_depths,
                &bound,
                &func_sigs,
                Some(&mut latent),
            );
            let d_init = init.depth;
            let asserted = if rng.below(3) == 0 { Some(d_init) } else { None };
            locals.push(LocalDef {
                name: "x".into(),
                ty: Type::Int,
                asserted,
                span: Span { start: 120 + idx, end: 122 + idx },
            });
            stmts.push(Stmt::Let { local: LocalId(1), value: init });
            bound.push((LocalId(1), d_init));
        } else {
            // `Stmt::Declare` + `Assign`
            locals.push(LocalDef {
                name: "x".into(),
                ty: Type::Int,
                asserted: None,
                span: Span { start: 120 + idx, end: 122 + idx },
            });
            stmts.push(Stmt::Declare { local: LocalId(1) });
            let rhs = gen_valid_int_expr(
                rng,
                1,
                Held::NONE,
                &global_depths,
                &bound,
                &func_sigs,
                Some(&mut latent),
            );
            let d_rhs = rhs.depth;
            stmts.push(Stmt::Expr(e(
                ExprKind::Assign {
                    place: Place { local: LocalId(1), path: Vec::new() },
                    value: Box::new(rhs),
                },
                Type::Unit,
                d_rhs,
            )));
            bound.push((LocalId(1), d_rhs));
        }

        if rng.below(4) == 0 {
            let ret_val = gen_valid_int_expr(
                rng,
                1,
                Held::NONE,
                &global_depths,
                &bound,
                &func_sigs,
                Some(&mut latent),
            );
            let d_ret = ret_val.depth;
            d_returns = d_returns.join(d_ret);
            stmts.push(Stmt::Expr(e(ExprKind::Return(Some(Box::new(ret_val))), Type::Unit, d_ret)));
        }

        let tail = gen_valid_int_expr(
            rng,
            2,
            Held::NONE,
            &global_depths,
            &bound,
            &func_sigs,
            Some(&mut latent),
        );
        let ret_depth = tail.depth.join(d_returns);
        let id = FuncId(idx);
        func_sigs.push((id, latent, ret_depth));
        unit.funcs.push(FuncDef {
            name: format!("f{i}"),
            params: vec![LocalId(0)],
            ret: Type::Int,
            ret_depth,
            asserted_ret: if rng.below(3) == 0 { Some(ret_depth) } else { None },
            latent,
            asserted_latent: if rng.below(3) == 0 { Some(latent) } else { None },
            locals,
            body: Block { stmts, tail: Some(Box::new(tail)), span: Span::default() },
            span: Span { start: 200 + idx, end: 210 + idx },
        });
    }

    // 1..=2 demands
    let n_demands = 1 + rng.below(2);
    for i in 0..n_demands {
        let idx = u32_of(i);
        let val = gen_valid_int_expr(rng, 2, Held::NONE, &global_depths, &[], &func_sigs, None);
        unit.demands.push(Demand { value: val, span: Span { start: 300 + idx, end: 305 + idx } });
    }

    // 55% of the time, inject one of the 8 FaultKind classes (or a combination).
    let mode = rng.below(18);
    match mode {
        // 0..=7: keep valid!
        0..=7 => {}
        // 8: FaultKind::Ungranted at top level (either no descend, or wrong capability!)
        8 => {
            let cap_idx = rng.below(8);
            let (cap, _) = CAP_PRIMS[cap_idx];
            let call_expr = prim_call(cap_idx, int(5));
            let wrong = if rng.below(2) == 0 {
                call_expr
            } else {
                let other_idx = (cap_idx + 1 + rng.below(7)) % 8;
                let (other_cap, _) = CAP_PRIMS[other_idx];
                debug_assert_ne!(cap, other_cap);
                descend(other_cap, call_expr)
            };
            unit.demands.push(Demand { value: wrong, span: Span { start: 400, end: 410 } });
        }
        // 9: FaultKind::Orpheus (look at depth 0, under wrong capability, or inside a function)
        9 => {
            let cap_idx = rng.below(8);
            let (cap, _) = CAP_PRIMS[cap_idx];
            let origin = cap.stratum();
            let deep_inner = e(ExprKind::Literal(Literal::Int(9)), Type::Int, origin);
            // Make a valid shade of `origin` from a global at `origin`, or a direct shade of a deep call
            let shaded = if cap == Capability::Env {
                let clk = descend(
                    Capability::Env,
                    call(
                        prim(Prim::Clock, Vec::new(), Type::Int),
                        Vec::new(),
                        Type::Int,
                        Depth::ENV,
                    ),
                );
                e(
                    ExprKind::Rite { rite: Rite::Shade, operand: Box::new(clk) },
                    Type::Shade { origin: Depth::ENV, inner: Box::new(Type::Int) },
                    Depth::PURE,
                )
            } else {
                let raw = descend(cap, prim_call(cap_idx, int(1)));
                let d_raw = raw.depth;
                let inner_ty = raw.ty.clone();
                let s = e(
                    ExprKind::Rite { rite: Rite::Shade, operand: Box::new(raw) },
                    Type::Shade { origin: d_raw, inner: Box::new(inner_ty.clone()) },
                    Depth::PURE,
                );
                let looked =
                    e(ExprKind::Rite { rite: Rite::Look, operand: Box::new(s) }, inner_ty, d_raw);
                let bad = if rng.below(2) == 0 {
                    looked
                } else {
                    let other_cap = CAP_PRIMS[(cap_idx + 1) % 8].0;
                    descend(other_cap, looked)
                };
                unit.demands.push(Demand { value: bad, span: Span { start: 411, end: 420 } });
                return unit;
            };
            let _ = deep_inner;
            let looked = e(
                ExprKind::Rite { rite: Rite::Look, operand: Box::new(shaded) },
                Type::Int,
                Depth::ENV,
            );
            let bad = if rng.below(2) == 0 { looked } else { descend(Capability::Net, looked) };
            unit.demands.push(Demand { value: bad, span: Span { start: 411, end: 420 } });
        }
        // 10: FaultKind::Stated (wrong expression depth or wrong function ret_depth)
        10 => {
            if rng.below(2) == 0 {
                let d = &mut unit.demands[0].value.depth;
                *d = rng.diff_depth(*d);
            } else {
                let d = &mut unit.funcs[0].ret_depth;
                *d = rng.diff_depth(*d);
            }
        }
        // 11: FaultKind::Asserted (wrong @d on global, local, or function return)
        11 => match rng.below(3) {
            0 if !unit.globals.is_empty() => {
                let g = &mut unit.globals[0];
                g.asserted = Some(rng.diff_depth(g.value.depth));
            }
            1 => {
                let f = &mut unit.funcs[0];
                f.locals[0].asserted = Some(rng.diff_depth(Depth::PURE));
            }
            _ => {
                let f = &mut unit.funcs[0];
                f.asserted_ret = Some(rng.diff_depth(f.ret_depth));
            }
        },
        // 12: FaultKind::StatedLatent
        12 => {
            let f = &mut unit.funcs[0];
            let flip = rng.capability().stratum();
            f.latent = if f.latent.holds(flip) {
                f.latent.without(Held::of(flip)).with(Depth::PURE)
            } else {
                f.latent.with(flip)
            };
        }
        // 13: FaultKind::AssertedLatent
        13 => {
            let f = &mut unit.funcs[0];
            let flip = rng.capability().stratum();
            let wrong = if f.latent.holds(flip) {
                f.latent.without(Held::of(flip)).with(Depth::PURE)
            } else {
                f.latent.with(flip)
            };
            f.asserted_latent = Some(wrong);
        }
        // 14: FaultKind::Frozen (assign after seal/shade/deposit/return/call, or in loop)
        14 => {
            let f = &mut unit.funcs[0];
            let slot_id = LocalId(u32_of(f.locals.len()));
            f.locals.push(LocalDef {
                name: "frz".into(),
                ty: Type::Int,
                asserted: None,
                span: Span { start: 500, end: 503 },
            });
            let read_local = Expr {
                kind: ExprKind::Local(slot_id),
                ty: Type::Int,
                depth: Depth::PURE,
                span: Span { start: 504, end: 507 },
            };
            let assign_local = Expr {
                kind: ExprKind::Assign {
                    place: Place { local: slot_id, path: Vec::new() },
                    value: Box::new(int(42)),
                },
                ty: Type::Unit,
                depth: Depth::PURE,
                span: Span { start: 508, end: 512 },
            };
            let naming_stmt = match rng.below(4) {
                0 => Stmt::Expr(pure(
                    ExprKind::Rite { rite: Rite::Seal, operand: Box::new(read_local) },
                    Type::Cairn,
                )),
                1 => Stmt::Expr(pure(
                    ExprKind::Rite { rite: Rite::Shade, operand: Box::new(read_local) },
                    Type::Shade { origin: Depth::PURE, inner: Box::new(Type::Int) },
                )),
                2 => Stmt::Expr(read_local),
                _ => Stmt::Expr(call(
                    prim(Prim::Abs, vec![Type::Int], Type::Int),
                    vec![read_local],
                    Type::Int,
                    Depth::PURE,
                )),
            };
            f.body.stmts.push(Stmt::Declare { local: slot_id });
            if rng.below(2) == 0 {
                f.body.stmts.push(naming_stmt);
                f.body.stmts.push(Stmt::Expr(assign_local));
            } else {
                // Loop back-edge freeze: assign then name inside a loop on an outer local
                let loop_body = pure(
                    ExprKind::Block(Block {
                        stmts: vec![Stmt::Expr(assign_local), naming_stmt],
                        tail: None,
                        span: Span::default(),
                    }),
                    Type::Unit,
                );
                f.body.stmts.push(Stmt::Expr(pure(
                    ExprKind::Loop { body: Box::new(loop_body), step: None },
                    Type::Unit,
                )));
            }
        }
        // 15..=17: FaultKind::Malformed (unbound local/global/func, bad prim/shade/look/call/assign)
        _ => {
            let bad = match rng.below(8) {
                0 => pure(ExprKind::Local(LocalId(99)), Type::Int),
                1 => pure(ExprKind::Global(GlobalId(99)), Type::Int),
                2 => pure(ExprKind::Func(FuncId(99)), Type::Unit),
                3 => pure(ExprKind::Prim(Prim::Clock), Type::Int),
                4 => pure(
                    ExprKind::Prim(Prim::Clock),
                    Type::Fn {
                        params: Vec::new(),
                        latent: Held::NONE,
                        result: Box::new(Type::Int),
                        result_depth: Depth::PURE,
                    },
                ),
                5 => pure(
                    ExprKind::Rite { rite: Rite::Shade, operand: Box::new(int(1)) },
                    Type::Shade { origin: Depth::DISK, inner: Box::new(Type::Int) },
                ),
                6 => {
                    pure(ExprKind::Rite { rite: Rite::Look, operand: Box::new(int(1)) }, Type::Int)
                }
                _ => pure(ExprKind::Call { callee: Box::new(int(1)), args: Vec::new() }, Type::Int),
            };
            unit.demands.push(Demand { value: bad, span: Span { start: 600, end: 610 } });
        }
    }

    unit
}

fn shrink_disagreement(mut unit: Unit) -> Unit {
    loop {
        let mut shrank = false;
        for i in (0..unit.demands.len()).rev() {
            if unit.demands.len() <= 1 {
                break;
            }
            let mut cand = unit.clone();
            cand.demands.remove(i);
            if kinds(&cand) != model_check(&cand) {
                unit = cand;
                shrank = true;
                break;
            }
        }
        if !shrank {
            for i in (0..unit.funcs.len()).rev() {
                let mut cand = unit.clone();
                cand.funcs.remove(i);
                if kinds(&cand) != model_check(&cand) {
                    unit = cand;
                    shrank = true;
                    break;
                }
            }
        }
        if !shrank {
            break;
        }
    }
    unit
}

#[derive(Default, Clone, Copy)]
struct Stats {
    valid: u64,
    invalid: u64,
    ungranted: u64,
    orpheus: u64,
    stated: u64,
    asserted: u64,
    stated_latent: u64,
    asserted_latent: u64,
    frozen: u64,
    malformed: u64,
}

impl Stats {
    fn merge(&mut self, other: Self) {
        self.valid += other.valid;
        self.invalid += other.invalid;
        self.ungranted += other.ungranted;
        self.orpheus += other.orpheus;
        self.stated += other.stated;
        self.asserted += other.asserted;
        self.stated_latent += other.stated_latent;
        self.asserted_latent += other.asserted_latent;
        self.frozen += other.frozen;
        self.malformed += other.malformed;
    }

    fn record(&mut self, faults: &[FaultKind]) {
        if faults.is_empty() {
            self.valid += 1;
        } else {
            self.invalid += 1;
        }
        for f in faults {
            match f {
                FaultKind::Ungranted { .. } => self.ungranted += 1,
                FaultKind::Orpheus { .. } => self.orpheus += 1,
                FaultKind::Stated { .. } => self.stated += 1,
                FaultKind::Asserted { .. } => self.asserted += 1,
                FaultKind::StatedLatent { .. } => self.stated_latent += 1,
                FaultKind::AssertedLatent { .. } => self.asserted_latent += 1,
                FaultKind::Frozen { .. } => self.frozen += 1,
                FaultKind::Malformed(_) => self.malformed += 1,
            }
        }
    }
}

fn run_differential_batch(seed: u64, rounds: u64) -> Stats {
    let mut rng = Rng(seed);
    let mut stats = Stats::default();
    for step in 0..rounds {
        let unit = gen_unit(&mut rng);
        let impl_faults = kinds(&unit);
        let model_faults = model_check(&unit);
        if impl_faults != model_faults {
            let minimal = shrink_disagreement(unit);
            panic!(
                "seed {seed:#x} step {step}: implementation and model disagreed!\n\
                 impl:  {:?}\n\
                 model: {:?}\n\
                 minimal unit: {minimal:#?}",
                kinds(&minimal),
                model_check(&minimal),
            );
        }
        stats.record(&impl_faults);
    }
    stats
}

#[test]
fn the_implementation_agrees_with_the_mechanized_model() {
    let stats = run_differential_batch(0x0226_CAFE_BEEF_0001, 50_000);
    assert!(stats.valid > 15_000, "too few valid programs: {}", stats.valid);
    assert!(stats.invalid > 15_000, "too few invalid programs: {}", stats.invalid);
    assert!(stats.ungranted > 500, "Ungranted not covered: {}", stats.ungranted);
    assert!(stats.orpheus > 500, "Orpheus not covered: {}", stats.orpheus);
    assert!(stats.stated > 500, "Stated not covered: {}", stats.stated);
    assert!(stats.asserted > 500, "Asserted not covered: {}", stats.asserted);
    assert!(stats.stated_latent > 500, "StatedLatent not covered: {}", stats.stated_latent);
    assert!(stats.asserted_latent > 500, "AssertedLatent not covered: {}", stats.asserted_latent);
    assert!(stats.frozen > 500, "Frozen not covered: {}", stats.frozen);
    assert!(stats.malformed > 500, "Malformed not covered: {}", stats.malformed);
}

#[test]
#[ignore = "ten-million-case differential soak; run with scripts/task fuzz or cargo test --release --test calculus -- --ignored"]
fn ten_million_programs_agree_with_the_model() {
    let total: u64 =
        std::env::var("NETHER_DIFF_ROUNDS").ok().and_then(|s| s.parse().ok()).unwrap_or(10_000_000);
    let seeds: [u64; 8] = [
        0x0226_0000_0000_0001,
        0x0226_0000_0000_0002,
        0x0226_0000_0000_0003,
        0x0226_0000_0000_0004,
        0x0226_0000_0000_0005,
        0x0226_0000_0000_0006,
        0x0226_0000_0000_0007,
        0x0226_0000_0000_0008,
    ];
    let per_seed = total / (seeds.len() as u64);
    let mut combined = Stats::default();
    std::thread::scope(|s| {
        let handles: Vec<_> = seeds
            .into_iter()
            .map(|seed| s.spawn(move || run_differential_batch(seed, per_seed)))
            .collect();
        for h in handles {
            combined.merge(h.join().expect("worker panicked"));
        }
    });
    eprintln!(
        "0226 differential check: {} programs ({} valid, {} invalid) — \
         ungranted={}, orpheus={}, stated={}, asserted={}, stated_latent={}, \
         asserted_latent={}, frozen={}, malformed={}",
        combined.valid + combined.invalid,
        combined.valid,
        combined.invalid,
        combined.ungranted,
        combined.orpheus,
        combined.stated,
        combined.asserted,
        combined.stated_latent,
        combined.asserted_latent,
        combined.frozen,
        combined.malformed,
    );
    assert_eq!(combined.valid + combined.invalid, total);
}
