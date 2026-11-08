#!/usr/bin/env python3
"""Mechanization of `spec/02-calculus.md`: the 11 typing rules and §2.4 metatheory."""
from __future__ import annotations
from typing import Dict, Tuple

from kernel import (
    Formula,
    InductiveRelation,
    Rule,
    Term,
    Theorem,
    app,
    elem,
    eq,
    inst_all,
    lattice_decide,
    le,
    mp,
    pred,
    rule_induction,
    subset,
    var,
)

# ── Terms and set/depth constructors ─────────────────────────────────────────

ZERO = var("0")
EMPTY = var("∅")
EMPTY_RAW = var("∅_raw")


def d_max(*args: Term) -> Term:
    assert len(args) >= 2
    out = args[0]
    for a in args[1:]:
        out = app("max", out, a)
    return out


def s_union(*args: Term) -> Term:
    assert len(args) >= 2
    out = args[0]
    for a in args[1:]:
        out = app("∪", out, a)
    return out


def singleton(d: Term) -> Term:
    return app("singleton", d)


def max_set(s: Term) -> Term:
    return app("max_set", s)


def g_of(e: Term) -> Term:
    """`g(e)` in §2.4: the set of `s(κ)` over `descend κ` in `e` and applied bodies."""
    if not e.args:
        return app("g", e)
    op = e.op
    if op in {"Lit", "Seal", "Shade"}:
        return EMPTY_RAW
    if op == "Var":
        return app("g_var", e.args[0])
    if op == "Abs":
        return EMPTY_RAW
    if op == "Descend":
        s_k, body = e.args
        return s_union(singleton(s_k), g_of(body))
    if op in {"Opaque", "Demand", "Look"}:
        return g_of(e.args[0])
    if op == "Prim":
        _, e1, e2 = e.args
        return s_union(g_of(e1), g_of(e2))
    if op == "Select":
        e1, e2, e3 = e.args
        return s_union(g_of(e1), g_of(e2), g_of(e3))
    if op == "App":
        f, a = e.args
        return s_union(g_of(f), g_of(a), app("g_body", f))
    return app("g", e)


# ── §2.2 The eleven typing rules (`HasType(Γ, δ, e, τ, d)`) ─────────────────
#
# Stated directly as written in `spec/02-calculus.md` §2.2 without strengthening
# any premise. Context variables carry their bound `d_x <= max_set(δ ∪ g_var(x))`
# and arrow types `(τ1 --df--> τ2@dr)` carry their body's ambient bound from [ABS]
# (`dr <= max_set(df ∪ g_body(f))`).

Γ = var("Γ")
δ = var("δ")
x = var("x")
ℓ = var("ℓ")
op = var("op")
e = var("e")
e1 = var("e1")
e2 = var("e2")
e3 = var("e3")
f = var("f")
a = var("a")
b = var("b")
s = var("s")
s_k = var("s_k")
τ = var("τ")
τ1 = var("τ1")
τ2 = var("τ2")
d = var("d")
d_p = var("d_p")
d1 = var("d1")
d2 = var("d2")
d3 = var("d3")
d_r = var("d_r")
d_f = var("d_f")
d_a = var("d_a")
df_set = var("δ_f")


def has_type(ctx: Term, amb: Term, expr: Term, ty: Term, depth: Term) -> Formula:
    return pred("HasType", ctx, amb, expr, ty, depth)


HAS_TYPE = InductiveRelation(
    "HasType",
    (
        # [VAR]: x : τ@d ∈ Γ
        Rule(
            "VAR",
            (
                pred("InCtx", x, τ, d, Γ),
                le(d, max_set(s_union(δ, app("g_var", x)))),
            ),
            has_type(Γ, δ, app("Var", x), τ, d),
        ),
        # [LIT]: ℓ a literal -> Γ ; δ ⊢ ℓ : τ@0
        Rule(
            "LIT",
            (pred("IsLit", ℓ, τ),),
            has_type(Γ, δ, app("Lit", ℓ), τ, ZERO),
        ),
        # [PRIM]: Γ ; δ ⊢ eᵢ : τᵢ@dᵢ -> Γ ; δ ⊢ ⊕(e₁..eₙ) : τ@max(d₁..dₙ)
        Rule(
            "PRIM",
            (
                has_type(Γ, δ, e1, τ1, d1),
                has_type(Γ, δ, e2, τ2, d2),
                pred("PrimSig", op, τ1, τ2, τ),
            ),
            has_type(Γ, δ, app("Prim", op, e1, e2), τ, d_max(d1, d2)),
        ),
        # [APP]: Γ ; δ ⊢ f : (τ₁ --df--> τ₂@d_r)@d_f, Γ ; δ ⊢ a : τ₁@d_a, df ⊆ δ
        Rule(
            "APP",
            (
                has_type(Γ, δ, f, app("Fn", τ1, df_set, τ2, d_r), d_f),
                has_type(Γ, δ, a, τ1, d_a),
                subset(df_set, δ),
                le(d_r, max_set(s_union(df_set, app("g_body", f)))),
            ),
            has_type(Γ, δ, app("App", f, a), τ2, d_max(d_r, d_f, d_a)),
        ),
        # [ABS]: Γ, x : τ₁@0 ; df ⊢ b : τ₂@d₂, df least -> Γ ; δ ⊢ λx.b : (τ₁ --df--> τ₂@d₂)@0
        Rule(
            "ABS",
            (
                has_type(app("Extend", Γ, x, τ1, ZERO), df_set, b, τ2, d2),
                pred("LeastLatent", Γ, x, τ1, b, df_set),
            ),
            has_type(Γ, δ, app("Abs", x, τ1, df_set, b), app("Fn", τ1, df_set, τ2, d2), ZERO),
        ),
        # [DESCEND]: Γ ; δ ∪ {s(κ)} ⊢ e : τ@d -> Γ ; δ ⊢ descend κ {e} : τ@d
        Rule(
            "DESCEND",
            (has_type(Γ, s_union(δ, singleton(s_k)), e, τ, d),),
            has_type(Γ, δ, app("Descend", s_k, e), τ, d),
        ),
        # [SEAL]: Γ ; δ ⊢ e : τ@d -> Γ ; δ ⊢ seal e : Cairn@0
        Rule(
            "SEAL",
            (has_type(Γ, δ, e, τ, d),),
            has_type(Γ, δ, app("Seal", e), var("Cairn"), ZERO),
        ),
        # [SHADE]: Γ ; δ ⊢ e : τ@d -> Γ ; δ ⊢ shade e : Shade^d<τ>@0
        Rule(
            "SHADE",
            (has_type(Γ, δ, e, τ, d),),
            has_type(Γ, δ, app("Shade", e), app("ShadeTy", d, τ), ZERO),
        ),
        # [LOOK]: Γ ; δ ⊢ s : Shade^d<τ>@d', d ∈ δ -> Γ ; δ ⊢ look s : τ@max(d, d')
        Rule(
            "LOOK",
            (
                has_type(Γ, δ, s, app("ShadeTy", d, τ), d_p),
                elem(d, δ),
            ),
            has_type(Γ, δ, app("Look", s), τ, d_max(d, d_p)),
        ),
        # [OPAQUE]: Γ ; δ ⊢ e : τ@d -> Γ ; δ ⊢ opaque e : τ@d
        Rule(
            "OPAQUE",
            (has_type(Γ, δ, e, τ, d),),
            has_type(Γ, δ, app("Opaque", e), τ, d),
        ),
        # [DEMAND]: Γ ; δ ⊢ e : τ@d -> Γ ; δ ⊢ demand e : U0@d
        Rule(
            "DEMAND",
            (has_type(Γ, δ, e, τ, d),),
            has_type(Γ, δ, app("Demand", e), var("U0"), d),
        ),
    ),
)


# ── Theorem 1: Ambient soundness (`spec/02-calculus.md` §2.4) ───────────────
#
# If `Γ ; δ ⊢ e : τ@d` then `d <= max(δ ∪ g(e))`.


def _ambient_prop(args: Tuple[Term, ...]) -> Formula:
    _ctx, amb, expr, _ty, depth = args
    return le(depth, max_set(s_union(amb, g_of(expr))))


def prove_ambient_soundness() -> Theorem:
    cases = {
        "VAR": lambda prems, _ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("VAR").conclusion.terms[0].args),
            [prems[1]],
        ),
        "LIT": lambda _prems, _ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("LIT").conclusion.terms[0].args),
        ),
        "PRIM": lambda _prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("PRIM").conclusion.terms[0].args),
            [ihs[0], ihs[1]],
        ),
        "APP": lambda prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("APP").conclusion.terms[0].args),
            [ihs[0], ihs[1], prems[2], prems[3]],
        ),
        "ABS": lambda _prems, _ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("ABS").conclusion.terms[0].args),
        ),
        "DESCEND": lambda _prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("DESCEND").conclusion.terms[0].args),
            [ihs[0]],
        ),
        "SEAL": lambda _prems, _ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("SEAL").conclusion.terms[0].args),
        ),
        "SHADE": lambda _prems, _ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("SHADE").conclusion.terms[0].args),
        ),
        "LOOK": lambda prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("LOOK").conclusion.terms[0].args),
            [ihs[0], prems[1]],
        ),
        "OPAQUE": lambda _prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("OPAQUE").conclusion.terms[0].args),
            [ihs[0]],
        ),
        "DEMAND": lambda _prems, ihs: lattice_decide(
            _ambient_prop(HAS_TYPE.rule("DEMAND").conclusion.terms[0].args),
            [ihs[0]],
        ),
    }
    return rule_induction(HAS_TYPE, _ambient_prop, cases)


# ── Lemma: Substitution depth bound across all typing forms ──────────────────
#
# `SubstDepth(e, d_before, d_a, d_after)` relates the pre-substitution depth
# `d_before` of a body expression `e` (typed under `x : τ_1@0` with grant `∅`)
# and the depth `d_a` of the substituted argument `a` to the post-substitution
# depth `d_after` of `e[x := a]`.

d_sub = var("d_sub")
d_x = var("d_x")
d1_p = var("d1_p")
d2_p = var("d2_p")
d_f_p = var("d_f_p")
d_a_p = var("d_a_p")
d_r_p = var("d_r_p")
y = var("y")


def subst_depth(expr: Term, d_before: Term, d_arg: Term, d_after: Term) -> Formula:
    return pred("SubstDepth", expr, d_before, d_arg, d_after)


SUBST_DEPTH = InductiveRelation(
    "SubstDepth",
    (
        # Substituting the bound parameter `x` (which had depth 0 in Γ) with `a` (depth `d_a`)
        Rule(
            "SubstVarSame",
            (),
            subst_depth(app("Var", x), ZERO, d_a, d_a),
        ),
        # Any other variable `y != x` keeps its depth `d_x`
        Rule(
            "SubstVarOther",
            (),
            subst_depth(app("Var", y), d_x, d_a, d_x),
        ),
        # Literals are closed and stay at depth 0
        Rule(
            "SubstLit",
            (),
            subst_depth(app("Lit", ℓ), ZERO, d_a, ZERO),
        ),
        # Primitive operation on two sub-expressions
        Rule(
            "SubstPrim",
            (
                subst_depth(e1, d1, d_a, d1_p),
                subst_depth(e2, d2, d_a, d2_p),
            ),
            subst_depth(app("Prim", op, e1, e2), d_max(d1, d2), d_a, d_max(d1_p, d2_p)),
        ),
        # Application inside body (arrow grant `df_set` is part of the static type,
        # so its ceiling contribution `d_r` is invariant under term substitution)
        Rule(
            "SubstApp",
            (
                subst_depth(f, d_f, d_a, d_f_p),
                subst_depth(a, d2, d_a, d2_p),
            ),
            subst_depth(
                app("App", f, a),
                d_max(d_r, d_f, d2),
                d_a,
                d_max(d_r, d_f_p, d2_p),
            ),
        ),
        # Lambda abstraction is always a value at depth 0 (`[ABS]`)
        Rule(
            "SubstAbs",
            (),
            subst_depth(app("Abs", y, τ1, df_set, b), ZERO, d_a, ZERO),
        ),
        # Descent propagates the body's depth
        Rule(
            "SubstDescend",
            (subst_depth(e, d, d_a, d_p),),
            subst_depth(app("Descend", s_k, e), d, d_a, d_p),
        ),
        # Seal produces a Cairn at depth 0 (`[SEAL]`)
        Rule(
            "SubstSeal",
            (subst_depth(e, d, d_a, d_p),),
            subst_depth(app("Seal", e), ZERO, d_a, ZERO),
        ),
        # Shade produces a Shade at depth 0 (`[SHADE]`)
        Rule(
            "SubstShade",
            (subst_depth(e, d, d_a, d_p),),
            subst_depth(app("Shade", e), ZERO, d_a, ZERO),
        ),
        # Look combines the shade's type parameter `d` with the operand's depth `d1`
        Rule(
            "SubstLook",
            (subst_depth(s, d1, d_a, d1_p),),
            subst_depth(app("Look", s), d_max(d, d1), d_a, d_max(d, d1_p)),
        ),
        # Opaque propagates the inner expression's depth
        Rule(
            "SubstOpaque",
            (subst_depth(e, d, d_a, d_p),),
            subst_depth(app("Opaque", e), d, d_a, d_p),
        ),
        # Demand propagates the inner expression's depth
        Rule(
            "SubstDemand",
            (subst_depth(e, d, d_a, d_p),),
            subst_depth(app("Demand", e), d, d_a, d_p),
        ),
    ),
)


def _subst_prop(args: Tuple[Term, ...]) -> Formula:
    _expr, d_before, d_arg, d_after = args
    return le(d_after, d_max(d_before, d_arg))


def prove_substitution_monotonicity() -> Theorem:
    cases = {
        r.name: (
            lambda _prems, ihs, r=r: lattice_decide(
                _subst_prop(r.conclusion.terms[0].args),
                list(ihs),
            )
        )
        for r in SUBST_DEPTH.rules
    }
    return rule_induction(SUBST_DEPTH, _subst_prop, cases)


# ── Theorem 2: Soundness of the bound / Monotonicity (`spec/02-calculus.md` §2.4) ─
#
# Reduction judgment `StepTyped(e, e', d, d')`: `e` at depth `d` reduces in one
# step to `e'` at depth `d'`. Covers every reduction step of §06 (`PrimLit`,
# `SelectTrue`, `SelectFalse`, `Beta`, `DescendVal`, `SealLit`, `LookShade`, and
# all congruence steps; `Opaque` has no congruence step per §2.2 and §6.1).

e_p = var("e_p")
e1_p = var("e1_p")
e2_p = var("e2_p")
f_p = var("f_p")
a_p = var("a_p")
s_p = var("s_p")


def step_typed(before: Term, after: Term, d_before: Term, d_after: Term) -> Formula:
    return pred("StepTyped", before, after, d_before, d_after)


STEP_TYPED = InductiveRelation(
    "StepTyped",
    (
        # Folding a primitive operation on literals yields a literal at depth 0
        Rule(
            "StepPrimLit",
            (pred("EvalPrim", op, e1, e2, ℓ),),
            step_typed(app("Prim", op, e1, e2), app("Lit", ℓ), d_max(d1, d2), ZERO),
        ),
        # Conditional taking the true branch (`[PRIM]` over condition and both arms)
        Rule(
            "StepSelectTrue",
            (),
            step_typed(
                app("Select", app("Lit", var("True")), e2, e3),
                e2,
                d_max(ZERO, d2, d3),
                d2,
            ),
        ),
        # Conditional taking the false branch
        Rule(
            "StepSelectFalse",
            (),
            step_typed(
                app("Select", app("Lit", var("False")), e2, e3),
                e3,
                d_max(ZERO, d2, d3),
                d3,
            ),
        ),
        # Beta reduction: `(λx.b) a ⟶ b[x := a]` where `SubstDepth(b, d_r, d_a, d_sub)`
        Rule(
            "StepBeta",
            (subst_depth(b, d_r, d_a, d_sub),),
            step_typed(
                app("App", app("Abs", x, τ1, df_set, b), a),
                app("Subst", b, x, a),
                d_max(d_r, d_f, d_a),
                d_sub,
            ),
        ),
        # Finished descent returns its body's value and depth
        Rule(
            "StepDescendVal",
            (pred("IsVal", e),),
            step_typed(app("Descend", s_k, e), e, d, d),
        ),
        # Sealing a finished value yields a Cairn literal at depth 0
        Rule(
            "StepSealVal",
            (pred("IsVal", e),),
            step_typed(app("Seal", e), app("Lit", app("CairnOf", e)), ZERO, ZERO),
        ),
        # Looking at a constructed shade `shade e` (where `e` has depth `d` and the
        # shade literal itself is depth 0) yields `e` at depth `d <= max(d, 0)`
        Rule(
            "StepLookShade",
            (elem(d, δ),),
            step_typed(app("Look", app("Shade", e)), e, d_max(d, ZERO), d),
        ),
        # Congruence rules (note: `Opaque` is absent because burial never steps inside `opaque`)
        Rule(
            "StepPrimL",
            (step_typed(e1, e1_p, d1, d1_p),),
            step_typed(
                app("Prim", op, e1, e2),
                app("Prim", op, e1_p, e2),
                d_max(d1, d2),
                d_max(d1_p, d2),
            ),
        ),
        Rule(
            "StepPrimR",
            (step_typed(e2, e2_p, d2, d2_p),),
            step_typed(
                app("Prim", op, e1, e2),
                app("Prim", op, e1, e2_p),
                d_max(d1, d2),
                d_max(d1, d2_p),
            ),
        ),
        Rule(
            "StepSelectCond",
            (step_typed(e1, e1_p, d1, d1_p),),
            step_typed(
                app("Select", e1, e2, e3),
                app("Select", e1_p, e2, e3),
                d_max(d1, d2, d3),
                d_max(d1_p, d2, d3),
            ),
        ),
        Rule(
            "StepAppL",
            (step_typed(f, f_p, d_f, d_f_p),),
            step_typed(
                app("App", f, a),
                app("App", f_p, a),
                d_max(d_r, d_f, d_a),
                d_max(d_r, d_f_p, d_a),
            ),
        ),
        Rule(
            "StepAppR",
            (step_typed(a, a_p, d_a, d_a_p),),
            step_typed(
                app("App", f, a),
                app("App", f, a_p),
                d_max(d_r, d_f, d_a),
                d_max(d_r, d_f, d_a_p),
            ),
        ),
        Rule(
            "StepDescendCong",
            (step_typed(e, e_p, d, d_p),),
            step_typed(app("Descend", s_k, e), app("Descend", s_k, e_p), d, d_p),
        ),
        Rule(
            "StepSealCong",
            (step_typed(e, e_p, d, d_p),),
            step_typed(app("Seal", e), app("Seal", e_p), ZERO, ZERO),
        ),
        Rule(
            "StepShadeCong",
            (step_typed(e, e_p, d, d_p),),
            step_typed(app("Shade", e), app("Shade", e_p), ZERO, ZERO),
        ),
        Rule(
            "StepLookCong",
            (step_typed(s, s_p, d1, d1_p),),
            step_typed(app("Look", s), app("Look", s_p), d_max(d, d1), d_max(d, d1_p)),
        ),
        Rule(
            "StepDemandCong",
            (step_typed(e, e_p, d, d_p),),
            step_typed(app("Demand", e), app("Demand", e_p), d, d_p),
        ),
    ),
)


def _monotone_prop(args: Tuple[Term, ...]) -> Formula:
    _before, _after, d_before, d_after = args
    return le(d_after, d_before)


def prove_soundness_of_bound() -> Theorem:
    subst_thm = prove_substitution_monotonicity()
    cases: Dict[str, object] = {}
    for r in STEP_TYPED.rules:
        if r.name == "StepBeta":
            def _beta_case(prems: Tuple[Theorem, ...], _ihs: Tuple[Theorem, ...], r: Rule = r) -> Theorem:
                sub_bound = mp(inst_all(subst_thm, [b, d_r, d_a, d_sub]), prems[0])
                return lattice_decide(
                    _monotone_prop(r.conclusion.terms[0].args),
                    [sub_bound],
                )
            cases[r.name] = _beta_case
        else:
            cases[r.name] = lambda _prems, ihs, r=r: lattice_decide(
                _monotone_prop(r.conclusion.terms[0].args),
                list(ihs),
            )
    return rule_induction(STEP_TYPED, _monotone_prop, cases)  # type: ignore[arg-type]


if __name__ == "__main__":
    t_amb = prove_ambient_soundness()
    t_sub = prove_substitution_monotonicity()
    t_mon = prove_soundness_of_bound()
    print(f"proved  ambient_soundness          ({len(HAS_TYPE.rules)} typing rules)")
    print(f"proved  substitution_monotonicity  ({len(SUBST_DEPTH.rules)} substitution rules)")
    print(f"proved  soundness_of_bound         ({len(STEP_TYPED.rules)} reduction rules)")
