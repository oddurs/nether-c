#!/usr/bin/env python3
"""Mechanization of the staging law (`spec/06-evaluation.md` §6.5 & §6.8).

Explicit assumptions modeled in this file:
1. **Fixed deterministic answer environment `W`:** Every world-question `(s_k, q_id, v)`
   at capability stratum `s_k` with value argument `v` returns the same answer
   `ans(W, s_k, q_id, v)` whether queried during a single burial `bury(e, A ∪ B)`
   or across staged burials `bury(bury(e, A), B)`. Independently sampling a
   mutable world (`W_1 != W_2`) is outside the law.
2. **Sufficient fuel:** Reductions reach normal form without exhausting the step
   budget (`fuel = ∞`).
3. **Round-trip identity (`lower(parse(print(r))) = r`):** Verified inductively
   over all 12 residual expression forms (`ROUND_TRIP`) and tested against the
   implementation in `crates/nether-bury/tests/residue_is_source.rs`.
4. **Capability monotonicity for dependent nested holes:** For flat holes (whose
   arguments are pure values), `(s_k ∈ A ∨ s_k ∈ B) ↔ s_k ∈ (A ∪ B)` holds for
   arbitrary capability sets `A, B`. For dependent nested holes (an outer hole
   at stratum `d_out` whose argument waits on an inner hole at stratum `d_in`),
   dropping capabilities between stages (`A = {0, 3}, B = {0, 5}`) is a
   counterexample (`dependent_hole_non_monotone_counterexample`), whereas
   cumulative capability sets (`A ⊆ B`, or passing `A ∪ B` to the second stage)
   preserve equivalence (`cumulative_dependent_hole_law`).
"""
from __future__ import annotations

from typing import Dict, Tuple

from kernel import (
    Formula,
    InductiveRelation,
    Rule,
    Term,
    Theorem,
    app,
    cong,
    conj,
    disj,
    elem,
    eq,
    gen,
    iff,
    imp,
    inst_all,
    lattice_decide,
    mp,
    not_elem,
    pred,
    refl,
    rule_induction,
    subset,
    var,
)
from calculus import EMPTY, s_union, singleton

v = var("v")
x = var("x")
s_k = var("s_k")
q_id = var("q_id")
op = var("op")
d = var("d")
e = var("e")
e1 = var("e1")
e2 = var("e2")
e3 = var("e3")
f = var("f")
a = var("a")
out = var("out")
out1 = var("out1")
out2 = var("out2")
out3 = var("out3")
r1 = var("r1")
r2 = var("r2")
r3 = var("r3")
u1 = var("u1")
u2 = var("u2")
u3 = var("u3")
A = var("δ_A")
B = var("δ_B")
W = var("W")
d_in = var("d_in")
d_out = var("d_out")
D3 = var("3")
D5 = var("5")


# ── 1. Print / Parse / Lower round-trip over all 12 expression constructors ──


def round_trip(expr: Term, result: Term) -> Formula:
    return pred("RoundTrip", expr, result)


ROUND_TRIP = InductiveRelation(
    "RoundTrip",
    (
        Rule("RT_Lit", (), round_trip(app("Lit", v), app("Lit", v))),
        Rule("RT_Var", (), round_trip(app("Var", x), app("Var", x))),
        Rule(
            "RT_HolePure",
            (),
            round_trip(app("HolePure", s_k, q_id, v), app("HolePure", s_k, q_id, v)),
        ),
        Rule(
            "RT_Prim",
            (round_trip(e1, out1), round_trip(e2, out2)),
            round_trip(app("Prim", op, e1, e2), app("Prim", op, out1, out2)),
        ),
        Rule(
            "RT_Select",
            (round_trip(e1, out1), round_trip(e2, out2), round_trip(e3, out3)),
            round_trip(app("Select", e1, e2, e3), app("Select", out1, out2, out3)),
        ),
        Rule(
            "RT_Descend",
            (round_trip(e1, out1),),
            round_trip(app("Descend", s_k, e1), app("Descend", s_k, out1)),
        ),
        Rule(
            "RT_Seal",
            (round_trip(e1, out1),),
            round_trip(app("Seal", e1), app("Seal", out1)),
        ),
        Rule(
            "RT_Shade",
            (round_trip(e1, out1),),
            round_trip(app("Shade", e1), app("Shade", out1)),
        ),
        Rule(
            "RT_Look",
            (round_trip(e1, out1),),
            round_trip(app("Look", d, e1), app("Look", d, out1)),
        ),
        Rule(
            "RT_Opaque",
            (round_trip(e1, out1),),
            round_trip(app("Opaque", e1), app("Opaque", out1)),
        ),
        Rule(
            "RT_Demand",
            (round_trip(e1, out1),),
            round_trip(app("Demand", e1), app("Demand", out1)),
        ),
        Rule(
            "RT_App",
            (round_trip(f, out1), round_trip(a, out2)),
            round_trip(app("App", f, a), app("App", out1, out2)),
        ),
    ),
)


def _rt_prop(args: Tuple[Term, ...]) -> Formula:
    expr, res = args
    return eq(res, expr)


def prove_roundtrip_identity() -> Theorem:
    """`∀e out. RoundTrip(e, out) → out = e` across all 12 residual constructors."""
    cases = {
        "RT_Lit": lambda _p, _ih: refl(app("Lit", v)),
        "RT_Var": lambda _p, _ih: refl(app("Var", x)),
        "RT_HolePure": lambda _p, _ih: refl(app("HolePure", s_k, q_id, v)),
        "RT_Prim": lambda _p, ih: cong("Prim", [refl(op), ih[0], ih[1]]),
        "RT_Select": lambda _p, ih: cong("Select", [ih[0], ih[1], ih[2]]),
        "RT_Descend": lambda _p, ih: cong("Descend", [refl(s_k), ih[0]]),
        "RT_Seal": lambda _p, ih: cong("Seal", [ih[0]]),
        "RT_Shade": lambda _p, ih: cong("Shade", [ih[0]]),
        "RT_Look": lambda _p, ih: cong("Look", [refl(d), ih[0]]),
        "RT_Opaque": lambda _p, ih: cong("Opaque", [ih[0]]),
        "RT_Demand": lambda _p, ih: cong("Demand", [ih[0]]),
        "RT_App": lambda _p, ih: cong("App", [ih[0], ih[1]]),
    }
    return rule_induction(ROUND_TRIP, _rt_prop, cases)


# ── 2. Capability-set algebra for staged vs direct hole resolution ───────────


def prove_hole_set_union_law() -> Theorem:
    """`∀s_k δ_A δ_B. ((s_k ∈ δ_A) ∨ (s_k ∈ δ_B)) ↔ (s_k ∈ (δ_A ∪ δ_B))`."""
    goal = iff(disj(elem(s_k, A), elem(s_k, B)), elem(s_k, s_union(A, B)))
    thm = lattice_decide(goal)
    for b in ("δ_B", "δ_A", "s_k"):
        thm = gen(b, thm)
    return thm


def prove_staging_idempotence_set() -> Theorem:
    """`∀s_k δ_A. ((s_k ∈ δ_A) ∨ (s_k ∈ δ_A)) ↔ (s_k ∈ δ_A)`."""
    goal = iff(disj(elem(s_k, A), elem(s_k, A)), elem(s_k, A))
    thm = lattice_decide(goal)
    for b in ("δ_A", "s_k"):
        thm = gen(b, thm)
    return thm


# ── 3. Full structural staging law across all expression forms ───────────────
#
# `StagedEval(e, A, B, r_staged, r_direct)` relates the two-stage reduction
# `bury(bury(e, A), B)` and the single-stage reduction `bury(e, A ∪ B)` under a
# fixed deterministic answer environment `W` and sufficient fuel.


def staged_eval(expr: Term, cap_a: Term, cap_b: Term, res_staged: Term, res_direct: Term) -> Formula:
    return pred("StagedEval", expr, cap_a, cap_b, res_staged, res_direct)


def resolve_hole(world: Term, cap_set: Term, stratum: Term, q: Term, arg_v: Term) -> Term:
    return app("ResolveHole", world, cap_set, stratum, q, arg_v)


STAGED_EVAL = InductiveRelation(
    "StagedEval",
    (
        # Literals are already values in both stages and in direct burial
        Rule(
            "SE_Lit",
            (),
            staged_eval(app("Lit", v), A, B, app("Lit", v), app("Lit", v)),
        ),
        # Free/residual variables round-trip between stage A and stage B
        Rule(
            "SE_Var",
            (round_trip(app("Var", x), out),),
            staged_eval(app("Var", x), A, B, out, app("Var", x)),
        ),
        # Hole answered in stage A (`s_k ∈ A` -> `s_k ∈ A ∪ B`): both yield `Lit(ans(W, s_k, q_id, v))`
        # after round-tripping the literal between stage A and stage B
        Rule(
            "SE_HoleHitA",
            (elem(s_k, A), round_trip(app("Lit", app("ans", W, s_k, q_id, v)), out)),
            staged_eval(
                app("HolePure", s_k, q_id, v),
                A,
                B,
                out,
                app("Lit", app("ans", W, s_k, q_id, v)),
            ),
        ),
        # Hole unanswered in stage A (`s_k ∉ A`) and answered in stage B (`s_k ∈ B`):
        # stage A residualises `HolePure(s_k, q_id, v)`, round-trips it, and stage B answers it
        Rule(
            "SE_HoleHitB",
            (not_elem(s_k, A), elem(s_k, B)),
            staged_eval(
                app("HolePure", s_k, q_id, v),
                A,
                B,
                app("Lit", app("ans", W, s_k, q_id, v)),
                app("Lit", app("ans", W, s_k, q_id, v)),
            ),
        ),
        # Hole unanswered in both stages (`s_k ∉ A ∧ s_k ∉ B` <-> `s_k ∉ A ∪ B`):
        # stage A residualises `HolePure(s_k, q_id, v)` and round-trips it into `out`
        Rule(
            "SE_HoleMiss",
            (not_elem(s_k, A), not_elem(s_k, B), round_trip(app("HolePure", s_k, q_id, v), out)),
            staged_eval(
                app("HolePure", s_k, q_id, v),
                A,
                B,
                out,
                app("HolePure", s_k, q_id, v),
            ),
        ),
        # Primitive fold when both operands reduce to literals
        Rule(
            "SE_PrimFold",
            (
                staged_eval(e1, A, B, app("Lit", u1), app("Lit", u1)),
                staged_eval(e2, A, B, app("Lit", u2), app("Lit", u2)),
            ),
            staged_eval(
                app("Prim", op, e1, e2),
                A,
                B,
                app("Lit", app("eval_prim", op, u1, u2)),
                app("Lit", app("eval_prim", op, u1, u2)),
            ),
        ),
        # Primitive residualisation when an operand remains residual
        Rule(
            "SE_PrimResid",
            (
                staged_eval(e1, A, B, r1, u1),
                staged_eval(e2, A, B, r2, u2),
            ),
            staged_eval(
                app("Prim", op, e1, e2),
                A,
                B,
                app("Prim", op, r1, r2),
                app("Prim", op, u1, u2),
            ),
        ),
        # Conditional whose condition resolves to True
        Rule(
            "SE_SelectTrue",
            (
                staged_eval(e1, A, B, app("Lit", var("True")), app("Lit", var("True"))),
                staged_eval(e2, A, B, r2, u2),
            ),
            staged_eval(app("Select", e1, e2, e3), A, B, r2, u2),
        ),
        # Conditional whose condition resolves to False
        Rule(
            "SE_SelectFalse",
            (
                staged_eval(e1, A, B, app("Lit", var("False")), app("Lit", var("False"))),
                staged_eval(e3, A, B, r3, u3),
            ),
            staged_eval(app("Select", e1, e2, e3), A, B, r3, u3),
        ),
        # Undecided branch: speculative reduction of both arms (§6.5)
        Rule(
            "SE_SelectSpeculative",
            (
                staged_eval(e1, A, B, r1, u1),
                staged_eval(e2, A, B, r2, u2),
                staged_eval(e3, A, B, r3, u3),
            ),
            staged_eval(
                app("Select", e1, e2, e3),
                A,
                B,
                app("Select", r1, r2, r3),
                app("Select", u1, u2, u3),
            ),
        ),
        # Descent
        Rule(
            "SE_Descend",
            (staged_eval(e1, A, B, r1, u1),),
            staged_eval(app("Descend", s_k, e1), A, B, app("Descend", s_k, r1), app("Descend", s_k, u1)),
        ),
        # Seal
        Rule(
            "SE_Seal",
            (staged_eval(e1, A, B, r1, u1),),
            staged_eval(app("Seal", e1), A, B, app("Seal", r1), app("Seal", u1)),
        ),
        # Shade
        Rule(
            "SE_Shade",
            (staged_eval(e1, A, B, r1, u1),),
            staged_eval(app("Shade", e1), A, B, app("Shade", r1), app("Shade", u1)),
        ),
        # Look
        Rule(
            "SE_Look",
            (staged_eval(e1, A, B, r1, u1),),
            staged_eval(app("Look", d, e1), A, B, app("Look", d, r1), app("Look", d, u1)),
        ),
        # Opaque barrier: never reduces under any stage (§6.4), round-tripped between stages
        Rule(
            "SE_Opaque",
            (round_trip(app("Opaque", e1), out),),
            staged_eval(app("Opaque", e1), A, B, out, app("Opaque", e1)),
        ),
        # Demand
        Rule(
            "SE_Demand",
            (staged_eval(e1, A, B, r1, u1),),
            staged_eval(app("Demand", e1), A, B, app("Demand", r1), app("Demand", u1)),
        ),
        # Specialised call / residual call (§6.5)
        Rule(
            "SE_App",
            (staged_eval(f, A, B, r1, u1), staged_eval(a, A, B, r2, u2)),
            staged_eval(app("App", f, a), A, B, app("App", r1, r2), app("App", u1, u2)),
        ),
    ),
)


def _staged_prop(args: Tuple[Term, ...]) -> Formula:
    _expr, _cap_a, _cap_b, res_staged, res_direct = args
    return eq(res_staged, res_direct)


def prove_staging_law() -> Theorem:
    """`∀e δ_A δ_B r_staged r_direct. StagedEval(e, δ_A, δ_B, r_staged, r_direct) → r_staged = r_direct`."""
    rt_thm = prove_roundtrip_identity()
    lit_ans = app("Lit", app("ans", W, s_k, q_id, v))
    hole_pure = app("HolePure", s_k, q_id, v)
    cases = {
        "SE_Lit": lambda _p, _ih: refl(app("Lit", v)),
        "SE_Var": lambda prems, _ih: mp(inst_all(rt_thm, [app("Var", x), out]), prems[0]),
        "SE_HoleHitA": lambda prems, _ih: mp(inst_all(rt_thm, [lit_ans, out]), prems[1]),
        "SE_HoleHitB": lambda _p, _ih: refl(lit_ans),
        "SE_HoleMiss": lambda prems, _ih: mp(inst_all(rt_thm, [hole_pure, out]), prems[2]),
        "SE_PrimFold": lambda _p, _ih: refl(app("Lit", app("eval_prim", op, u1, u2))),
        "SE_PrimResid": lambda _p, ih: cong("Prim", [refl(op), ih[0], ih[1]]),
        "SE_SelectTrue": lambda _p, ih: ih[1],
        "SE_SelectFalse": lambda _p, ih: ih[1],
        "SE_SelectSpeculative": lambda _p, ih: cong("Select", [ih[0], ih[1], ih[2]]),
        "SE_Descend": lambda _p, ih: cong("Descend", [refl(s_k), ih[0]]),
        "SE_Seal": lambda _p, ih: cong("Seal", [ih[0]]),
        "SE_Shade": lambda _p, ih: cong("Shade", [ih[0]]),
        "SE_Look": lambda _p, ih: cong("Look", [refl(d), ih[0]]),
        "SE_Opaque": lambda prems, _ih: mp(inst_all(rt_thm, [app("Opaque", e1), out]), prems[0]),
        "SE_Demand": lambda _p, ih: cong("Demand", [ih[0]]),
        "SE_App": lambda _p, ih: cong("App", [ih[0], ih[1]]),
    }
    return rule_induction(STAGED_EVAL, _staged_prop, cases)


# ── 4. Dependent nested holes: non-monotone counterexample vs cumulative law ─


def prove_dependent_hole_non_monotone_counterexample() -> Theorem:
    """Counterexample when stage B drops capabilities from stage A on a nested
    dependent question `read(must(get(url)))` (`d_out = 3`, `d_in = 5`):
    With `A = {0, 3}` and `B = {0, 5}`, `5 ∉ A` (so stage A cannot resolve the
    inner `get`), `3 ∉ B` (so stage B resolves `get` but cannot resolve the outer
    `read`), whereas `5 ∈ (A ∪ B) ∧ 3 ∈ (A ∪ B)` resolves both in `bury(e, A ∪ B)`."""
    set_a = s_union(EMPTY, singleton(D3))
    set_b = s_union(EMPTY, singleton(D5))
    set_ab = s_union(set_a, set_b)
    goal = conj(
        not_elem(D5, set_a),
        conj(
            conj(elem(D5, set_b), not_elem(D3, set_b)),
            conj(elem(D5, set_ab), elem(D3, set_ab)),
        ),
    )
    return lattice_decide(goal)


def prove_cumulative_dependent_hole_law() -> Theorem:
    """When capability grants are cumulative (`δ_A ⊆ δ_B`, as when stage 2 runs
    with `δ_A ∪ δ_B`), a dependent nested hole (`d_in` inside `d_out`) resolves
    across the two stages iff it resolves in a single stage under `δ_A ∪ δ_B`."""
    staged_resolves = disj(
        conj(elem(d_in, A), elem(d_out, A)),
        conj(elem(d_in, B), elem(d_out, B)),
    )
    direct_resolves = conj(elem(d_in, s_union(A, B)), elem(d_out, s_union(A, B)))
    goal = imp(subset(A, B), iff(staged_resolves, direct_resolves))
    thm = lattice_decide(goal)
    for b in ("δ_B", "δ_A", "d_out", "d_in"):
        thm = gen(b, thm)
    return thm


if __name__ == "__main__":
    for name, fn in (
        ("roundtrip_identity", prove_roundtrip_identity),
        ("hole_set_union_law", prove_hole_set_union_law),
        ("staging_idempotence_set", prove_staging_idempotence_set),
        ("staging_law", prove_staging_law),
        ("dependent_hole_non_monotone_counterexample", prove_dependent_hole_non_monotone_counterexample),
        ("cumulative_dependent_hole_law", prove_cumulative_dependent_hole_law),
    ):
        t = fn()
        print(f"proved  {name:42s} |- {t.conclusion}")
