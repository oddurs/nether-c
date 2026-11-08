#!/usr/bin/env python3
"""Mechanization of the Orpheus rule (`spec/01-strata.md` §1.6, `spec/02-calculus.md` [LOOK], `spec/90-rationale.md` §90.2).

Proves both halves of `[LOOK]`'s premise `d ∈ δ`:

1. **Sufficiency**:
   - `look_sufficiency_ambient`: `(d' <= max_set(δ ∪ g(s)) ∧ d ∈ δ) → max(d, d') <= max_set(δ ∪ g(s))`
   - `look_sufficiency_confinement`: `(d ∈ δ) → ({d} ⊆ (δ ∪ g(s)))`

2. **Exact Necessity & the Non-Interval Counterexample**:
   - `look_necessity_confinement`: for a closed shade value (`g(s) = ∅_raw`),
     `({d} ⊆ (δ ∪ ∅_raw)) ↔ (d ∈ δ)`. Any local premise on `(d, δ)` that
     guarantees the unveiled stratum `d` is held in `δ ∪ g(s)` for all `s`
     must entail `d ∈ δ`.
   - `look_necessity_numeric`: for a closed shade value (`g(s) = ∅_raw`, `d' = 0`),
     `(max(d, 0) <= max_set(δ ∪ ∅_raw)) ↔ (d <= max_set(δ))`.
   - `numeric_vs_membership_counterexample`: `d <= max_set(δ)` does NOT entail
     `d ∈ δ` when `δ` is a non-interval capability set. Explicit witness:
     `d = 3` (`disk`), `δ = {0, 5}` (`net`), where `3 <= max_set({0, 5})` holds
     (`3 <= 5`) yet `3 ∉ {0, 5}` (`{3} ⊈ {0, 5}`).

3. **§90.2 Rejected Alternatives**:
   - `binding_taint_violates_ambient`: dropping `[LOOK]`'s premise on `(d, δ)`
     while keeping return depth `max(d, d')` violates §2.4 ambient soundness on
     `d = 3, d' = 0, δ = {0}, g(s) = ∅_raw`: `¬(max(3, 0) <= max_set({0} ∪ ∅_raw))`.
   - `scope_restain_shifts_obligation_to_caller`: deferring `d` into the
     enclosing function's latent set `δ_f` (`{d} ⊆ δ_f`) combined with `[APP]`'s
     caller obligation `δ_f ⊆ δ_caller` entails `d ∈ δ_caller`
     (`({d} ⊆ δ_f ∧ δ_f ⊆ δ_caller) → d ∈ δ_caller`), so the capability check
     is never avoided—only displaced from the `look` site to every caller.
"""
from __future__ import annotations

from kernel import (
    Theorem,
    conj,
    elem,
    gen,
    iff,
    imp,
    lattice_decide,
    le,
    neg,
    not_elem,
    not_subset,
    subset,
    var,
)
from calculus import EMPTY, EMPTY_RAW, d, d_max, d_p, df_set, g_of, max_set, s, s_union, singleton, δ, ZERO

D3 = var("3")
D5 = var("5")
δ_caller = var("δ_caller")


def prove_look_sufficiency_ambient() -> Theorem:
    """`∀d d' δ s. (d' <= max_set(δ ∪ g(s)) ∧ d ∈ δ) → max(d, d') <= max_set(δ ∪ g(s))`."""
    hyp = conj(le(d_p, max_set(s_union(δ, g_of(s)))), elem(d, δ))
    goal = imp(hyp, le(d_max(d, d_p), max_set(s_union(δ, g_of(s)))))
    thm = lattice_decide(goal)
    for b in ("s", "δ", "d_p", "d"):
        thm = gen(b, thm)
    return thm


def prove_look_sufficiency_confinement() -> Theorem:
    """`∀d δ s. (d ∈ δ) → ({d} ⊆ (δ ∪ g(s)))`."""
    goal = imp(elem(d, δ), subset(singleton(d), s_union(δ, g_of(s))))
    thm = lattice_decide(goal)
    for b in ("s", "δ", "d"):
        thm = gen(b, thm)
    return thm


def prove_look_necessity_confinement() -> Theorem:
    """For a pure shade value (`g(s) = ∅_raw`), `{d} ⊆ (δ ∪ ∅_raw)` holds iff `d ∈ δ`."""
    goal = iff(subset(singleton(d), s_union(δ, EMPTY_RAW)), elem(d, δ))
    thm = lattice_decide(goal)
    for b in ("δ", "d"):
        thm = gen(b, thm)
    return thm


def prove_look_necessity_numeric() -> Theorem:
    """For a pure shade value (`g(s) = ∅_raw`, `d' = 0`), `max(d, 0) <= max_set(δ ∪ ∅_raw)`
    holds iff `d <= max_set(δ)`."""
    goal = iff(le(d_max(d, ZERO), max_set(s_union(δ, EMPTY_RAW))), le(d, max_set(δ)))
    thm = lattice_decide(goal)
    for b in ("δ", "d"):
        thm = gen(b, thm)
    return thm


def prove_numeric_vs_membership_counterexample() -> Theorem:
    """Concrete counterexample showing `d <= max_set(δ)` is strictly weaker than `d ∈ δ`:
    at `d = 3` (`disk`) and `δ = {0, 5}` (`net`), `3 <= max_set({0, 5})` holds while
    `3 ∉ {0, 5}` and `{3} ⊈ {0, 5}`."""
    delta_05 = s_union(EMPTY, singleton(D5))
    goal = conj(
        le(D3, max_set(delta_05)),
        conj(not_elem(D3, delta_05), not_subset(singleton(D3), delta_05)),
    )
    return lattice_decide(goal)


def prove_binding_taint_violates_ambient() -> Theorem:
    """§90.2 'Binding taint' (unconstrained `look` returning `max(d, d')` without `d ∈ δ`)
    violates §2.4 ambient soundness on `d = 3, d' = 0, δ = {0}, g(s) = ∅_raw`."""
    goal = neg(le(d_max(D3, ZERO), max_set(s_union(EMPTY, EMPTY_RAW))))
    return lattice_decide(goal)


def prove_scope_restain_shifts_obligation_to_caller() -> Theorem:
    """§90.2 'Scope re-stain' (recording `{d} ⊆ δ_f` on the enclosing function instead of
    checking `d ∈ δ` at `look`) combined with `[APP]`'s premise `δ_f ⊆ δ_caller` forces
    `d ∈ δ_caller` at every call site."""
    goal = imp(
        conj(subset(singleton(d), df_set), subset(df_set, δ_caller)),
        elem(d, δ_caller),
    )
    thm = lattice_decide(goal)
    for b in ("δ_caller", "δ_f", "d"):
        thm = gen(b, thm)
    return thm


if __name__ == "__main__":
    for name, fn in (
        ("look_sufficiency_ambient", prove_look_sufficiency_ambient),
        ("look_sufficiency_confinement", prove_look_sufficiency_confinement),
        ("look_necessity_confinement", prove_look_necessity_confinement),
        ("look_necessity_numeric", prove_look_necessity_numeric),
        ("numeric_vs_membership_counterexample", prove_numeric_vs_membership_counterexample),
        ("binding_taint_violates_ambient", prove_binding_taint_violates_ambient),
        ("scope_restain_shifts_obligation_to_caller", prove_scope_restain_shifts_obligation_to_caller),
    ):
        t = fn()
        print(f"proved  {name:42s} |- {t.conclusion}")
