//! The proof for *the bidirectional depth checker*: every rule in
//! `spec/02-calculus.md` derives what the specification says it derives, and
//! the two premises that are written down are enforced.
//!
//! The rule names are read back out of §02, so a twelfth rule fails this file
//! until somebody covers it — and so does an eleventh that goes missing. The
//! calculus fitting on one page is a constraint rather than an observation.

use std::collections::BTreeSet;

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Expr, ExprKind, Fault, FaultKind, FuncDef, GlobalDef,
    Literal, LocalDef, LocalId, Prim, Rite, Span, Stmt, Type, Unit, check,
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
        Type::Fn { params, latent: p.latent(), result: Box::new(result), result_depth: p.latent() },
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
            latent,
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
        vec![FaultKind::Ungranted { needed: Depth::DISK, ambient: Depth::PURE }]
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
        latent: Depth::PURE,
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
        latent: Depth::PURE,
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
        latent: Depth::DISK,
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
        vec![FaultKind::Ungranted { needed: Depth::DISK, ambient: Depth::PURE }]
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
    assert_eq!(
        kinds(&unit),
        vec![FaultKind::Ungranted { needed: Depth::DISK, ambient: Depth::PURE }]
    );
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
    assert_eq!(
        kinds(&surfaced),
        vec![FaultKind::Orpheus { origin: Depth::NET, ambient: Depth::PURE }]
    );

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
