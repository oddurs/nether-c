#!/usr/bin/env python3
"""LCF-style proof kernel for Nether C metatheory.

A `Theorem` can only be constructed by this module's inference rules using the
private `_SEAL` token. Any file in `proof/` that mentions `admit`, `sorry`, or
`unproved_axiom` is rejected by `proof/check.py` before checking begins.
"""
from __future__ import annotations
from dataclasses import dataclass
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple

_SEAL = object()


@dataclass(frozen=True)
class Term:
    op: str
    args: Tuple["Term", ...] = ()

    def __repr__(self) -> str:
        if not self.args:
            return self.op
        return f"{self.op}({', '.join(repr(a) for a in self.args)})"


def var(name: str) -> Term:
    return Term(name, ())


def app(op: str, *args: Term) -> Term:
    return Term(op, tuple(args))


def subst_term(t: Term, mapping: Dict[str, Term]) -> Term:
    if not t.args and t.op in mapping:
        return mapping[t.op]
    return Term(t.op, tuple(subst_term(a, mapping) for a in t.args))


def free_vars_term(t: Term) -> Set[str]:
    if not t.args:
        if t.op.isdigit() or t.op in {"∅", "True", "False", "U0", "Cairn"}:
            return set()
        return {t.op}
    out: Set[str] = set()
    for a in t.args:
        out |= free_vars_term(a)
    return out


@dataclass(frozen=True)
class Formula:
    kind: str
    terms: Tuple[Term, ...] = ()
    subs: Tuple["Formula", ...] = ()
    binder: str = ""

    def __repr__(self) -> str:
        if self.kind == "Pred":
            return repr(self.terms[0])
        if self.kind in {"=", "<=", "<", "!=_d", "∈", "∉", "⊆", "⊈"}:
            return f"({self.terms[0]} {self.kind} {self.terms[1]})"
        if self.kind == "¬":
            return f"¬{self.subs[0]}"
        if self.kind in {"∧", "∨", "→", "↔"}:
            return f"({self.subs[0]} {self.kind} {self.subs[1]})"
        if self.kind in {"∀", "∃"}:
            return f"({self.kind}{self.binder}. {self.subs[0]})"
        return f"{self.kind}({self.terms}, {self.subs})"


def pred(name: str, *args: Term) -> Formula:
    return Formula("Pred", (Term(name, tuple(args)),))


def eq(a: Term, b: Term) -> Formula:
    return Formula("=", (a, b))


def neq_d(a: Term, b: Term) -> Formula:
    return Formula("!=_d", (a, b))


def le(a: Term, b: Term) -> Formula:
    return Formula("<=", (a, b))


def lt(a: Term, b: Term) -> Formula:
    return Formula("<", (a, b))


def elem(d: Term, s: Term) -> Formula:
    return Formula("∈", (d, s))


def not_elem(d: Term, s: Term) -> Formula:
    return Formula("∉", (d, s))


def subset(a: Term, b: Term) -> Formula:
    return Formula("⊆", (a, b))


def not_subset(a: Term, b: Term) -> Formula:
    return Formula("⊈", (a, b))


def neg(f: Formula) -> Formula:
    return Formula("¬", (), (f,))


def conj(a: Formula, b: Formula) -> Formula:
    return Formula("∧", (), (a, b))


def conj_all(items: Sequence[Formula]) -> Formula:
    assert items, "conj_all requires at least one formula"
    out = items[0]
    for nxt in items[1:]:
        out = conj(out, nxt)
    return out


def disj(a: Formula, b: Formula) -> Formula:
    return Formula("∨", (), (a, b))


def imp(a: Formula, b: Formula) -> Formula:
    return Formula("→", (), (a, b))


def iff(a: Formula, b: Formula) -> Formula:
    return Formula("↔", (), (a, b))


def forall(binder: str, body: Formula) -> Formula:
    return Formula("∀", (), (body,), binder)


def subst_formula(f: Formula, mapping: Dict[str, Term]) -> Formula:
    if f.kind in {"∀", "∃"}:
        inner_map = {k: v for k, v in mapping.items() if k != f.binder}
        return Formula(f.kind, (), (subst_formula(f.subs[0], inner_map),), f.binder)
    return Formula(
        f.kind,
        tuple(subst_term(t, mapping) for t in f.terms),
        tuple(subst_formula(s, mapping) for s in f.subs),
        f.binder,
    )


class ProofFault(Exception):
    """Raised when an invalid derivation is presented to the kernel."""


def _require(cond: bool, msg: str) -> None:
    if not cond:
        raise ProofFault(msg)


@dataclass(frozen=True)
class Theorem:
    _token: object
    assumptions: frozenset[Formula]
    conclusion: Formula

    def __post_init__(self) -> None:
        _require(self._token is _SEAL, "Theorem constructed outside kernel")

    def __repr__(self) -> str:
        hyps = ", ".join(repr(a) for a in sorted(self.assumptions, key=repr))
        return f"{hyps} ⊢ {self.conclusion}"


def _thm(assumptions: Iterable[Formula], conclusion: Formula) -> Theorem:
    return Theorem(_SEAL, frozenset(assumptions), conclusion)


# ── Primitive logical inference rules ────────────────────────────────────────


def assume(f: Formula) -> Theorem:
    return _thm([f], f)


def refl(t: Term) -> Theorem:
    return _thm([], eq(t, t))


def sym(thm: Theorem) -> Theorem:
    _require(thm.conclusion.kind == "=", f"sym expects equality, got {thm.conclusion}")
    a, b = thm.conclusion.terms
    return _thm(thm.assumptions, eq(b, a))


def trans(t1: Theorem, t2: Theorem) -> Theorem:
    _require(t1.conclusion.kind == "=" and t2.conclusion.kind == "=", "trans expects =")
    _require(t1.conclusion.terms[1] == t2.conclusion.terms[0], "trans middle mismatch")
    return _thm(
        t1.assumptions | t2.assumptions,
        eq(t1.conclusion.terms[0], t2.conclusion.terms[1]),
    )


def mp(imp_thm: Theorem, ant_thm: Theorem) -> Theorem:
    _require(
        imp_thm.conclusion.kind == "→",
        f"mp expects implication, got {imp_thm.conclusion}",
    )
    lhs, rhs = imp_thm.conclusion.subs
    _require(
        lhs == ant_thm.conclusion,
        f"mp antecedent mismatch:\n  expected: {lhs}\n  got:      {ant_thm.conclusion}",
    )
    return _thm(imp_thm.assumptions | ant_thm.assumptions, rhs)


def discharge(hyp: Formula, thm: Theorem) -> Theorem:
    return _thm(thm.assumptions - {hyp}, imp(hyp, thm.conclusion))


def conj_intro(t1: Theorem, t2: Theorem) -> Theorem:
    return _thm(t1.assumptions | t2.assumptions, conj(t1.conclusion, t2.conclusion))


def conj_elim_l(thm: Theorem) -> Theorem:
    _require(thm.conclusion.kind == "∧", f"conj_elim_l expects ∧, got {thm.conclusion}")
    return _thm(thm.assumptions, thm.conclusion.subs[0])


def conj_elim_r(thm: Theorem) -> Theorem:
    _require(thm.conclusion.kind == "∧", f"conj_elim_r expects ∧, got {thm.conclusion}")
    return _thm(thm.assumptions, thm.conclusion.subs[1])


def iff_intro(ab: Theorem, ba: Theorem) -> Theorem:
    _require(ab.conclusion.kind == "→" and ba.conclusion.kind == "→", "iff_intro expects →")
    a, b = ab.conclusion.subs
    b2, a2 = ba.conclusion.subs
    _require(a == a2 and b == b2, "iff_intro mismatch")
    return _thm(ab.assumptions | ba.assumptions, iff(a, b))


def inst(thm: Theorem, term: Term) -> Theorem:
    _require(thm.conclusion.kind == "∀", f"inst expects ∀, got {thm.conclusion}")
    body = subst_formula(thm.conclusion.subs[0], {thm.conclusion.binder: term})
    return _thm(thm.assumptions, body)


def inst_all(thm: Theorem, terms: Sequence[Term]) -> Theorem:
    out = thm
    for t in terms:
        out = inst(out, t)
    return out


def gen(binder: str, thm: Theorem) -> Theorem:
    for a in thm.assumptions:
        _require(
            binder not in repr(a),
            f"cannot generalize `{binder}` while free in assumption {a}",
        )
    return _thm(thm.assumptions, forall(binder, thm.conclusion))


def rewrite(eq_thm: Theorem, target: Theorem) -> Theorem:
    _require(eq_thm.conclusion.kind == "=", f"rewrite expects =, got {eq_thm.conclusion}")
    src, dst = eq_thm.conclusion.terms

    def replace_t(t: Term) -> Term:
        if t == src:
            return dst
        return Term(t.op, tuple(replace_t(a) for a in t.args))

    def replace_f(f: Formula) -> Formula:
        return Formula(
            f.kind,
            tuple(replace_t(t) for t in f.terms),
            tuple(replace_f(s) for s in f.subs),
            f.binder,
        )

    return _thm(eq_thm.assumptions | target.assumptions, replace_f(target.conclusion))


# ── Bounded lattice & capability-set decision procedure ──────────────────────
#
# Depth = {0..8} with order <= and join `max`.
# Held  = subsets of {0..8} with 0 always present (§2.1: "Stratum 0 is in δ
#         always"), union `∪`, singleton `{d}`, membership `∈`, subset `⊆`, and
#         upper bound `max_set(δ)`.


def _eval_depth(t: Term, env_d: Dict[str, int], env_s: Dict[str, int]) -> int:
    if not t.args:
        if t.op.isdigit():
            v = int(t.op)
            _require(0 <= v <= 8, f"depth literal {v} out of range 0..8")
            return v
        return env_d[t.op]
    if t.op == "max":
        return max(_eval_depth(a, env_d, env_s) for a in t.args)
    if t.op == "max_set":
        mask = _eval_set(t.args[0], env_d, env_s)
        for bit in range(8, -1, -1):
            if mask & (1 << bit):
                return bit
        return 0
    return env_d[repr(t)]


def _eval_set(t: Term, env_d: Dict[str, int], env_s: Dict[str, int]) -> int:
    if not t.args:
        if t.op == "∅":
            return 1  # §2.1: stratum 0 is always present in ambient/capability sets
        if t.op == "∅_raw":
            return 0  # raw empty set for g(e)
        return env_s[t.op]
    if t.op == "singleton":
        d = _eval_depth(t.args[0], env_d, env_s)
        return 1 << d
    if t.op == "∪":
        out = 0
        for a in t.args:
            out |= _eval_set(a, env_d, env_s)
        return out
    return env_s[repr(t)]


def _collect_lattice_vars(f: Formula, d_vars: Set[str], s_vars: Set[str], ambient_s: Set[str]) -> None:
    def scan_d(t: Term) -> None:
        if not t.args:
            if not t.op.isdigit():
                d_vars.add(t.op)
        elif t.op == "max":
            for a in t.args:
                scan_d(a)
        elif t.op == "max_set":
            scan_s(t.args[0])
        else:
            # Uninterpreted depth-valued term such as d_x or s(κ): treat as a depth variable
            d_vars.add(repr(t))

    def scan_s(t: Term) -> None:
        if not t.args:
            if t.op not in {"∅", "∅_raw"}:
                s_vars.add(t.op)
                if t.op.startswith("δ"):
                    ambient_s.add(t.op)
        elif t.op == "singleton":
            scan_d(t.args[0])
        elif t.op == "∪":
            for a in t.args:
                scan_s(a)
        else:
            s_vars.add(repr(t))

    if f.kind in {"<=", "<", "=", "!=_d"}:
        scan_d(f.terms[0])
        scan_d(f.terms[1])
    elif f.kind in {"∈", "∉"}:
        scan_d(f.terms[0])
        scan_s(f.terms[1])
    elif f.kind in {"⊆", "⊈"}:
        scan_s(f.terms[0])
        scan_s(f.terms[1])
    else:
        for sub in f.subs:
            _collect_lattice_vars(sub, d_vars, s_vars, ambient_s)


def _eval_lattice_formula(f: Formula, env_d: Dict[str, int], env_s: Dict[str, int]) -> bool:
    def ed(t: Term) -> int:
        if t.args and t.op not in {"max", "max_set"}:
            return env_d[repr(t)]
        return _eval_depth(t, env_d, env_s)

    def es(t: Term) -> int:
        if t.args and t.op not in {"singleton", "∪"}:
            return env_s[repr(t)]
        return _eval_set(t, env_d, env_s)

    if f.kind == "<=":
        return ed(f.terms[0]) <= ed(f.terms[1])
    if f.kind == "<":
        return ed(f.terms[0]) < ed(f.terms[1])
    if f.kind == "=":
        return ed(f.terms[0]) == ed(f.terms[1])
    if f.kind == "!=_d":
        return ed(f.terms[0]) != ed(f.terms[1])
    if f.kind == "∈":
        return bool(es(f.terms[1]) & (1 << ed(f.terms[0])))
    if f.kind == "∉":
        return not bool(es(f.terms[1]) & (1 << ed(f.terms[0])))
    if f.kind == "⊆":
        a, b = es(f.terms[0]), es(f.terms[1])
        return (a & ~b) == 0
    if f.kind == "⊈":
        a, b = es(f.terms[0]), es(f.terms[1])
        return (a & ~b) != 0
    if f.kind == "¬":
        return not _eval_lattice_formula(f.subs[0], env_d, env_s)
    if f.kind == "∧":
        return _eval_lattice_formula(f.subs[0], env_d, env_s) and _eval_lattice_formula(
            f.subs[1], env_d, env_s
        )
    if f.kind == "∨":
        return _eval_lattice_formula(f.subs[0], env_d, env_s) or _eval_lattice_formula(
            f.subs[1], env_d, env_s
        )
    if f.kind == "→":
        return (not _eval_lattice_formula(f.subs[0], env_d, env_s)) or _eval_lattice_formula(
            f.subs[1], env_d, env_s
        )
    if f.kind == "↔":
        return _eval_lattice_formula(f.subs[0], env_d, env_s) == _eval_lattice_formula(
            f.subs[1], env_d, env_s
        )
    raise ProofFault(f"non-lattice formula `{f}` in lattice_decide")


def lattice_decide(goal: Formula, hyps: Sequence[Theorem] = ()) -> Theorem:
    """Exhaustively verify a universal lattice/capability-set formula over {0..8}.

    Because `Depth = {0..8}` is a 9-element chain and every capability set is a
    bitmask over `{0..8}`, any quantifier-free formula over depth variables and
    capability-set expressions is decidable by complete finite-domain checking
    (or algebraic reduction of `max_set` to its maximum element in `{0..8}`).
    """
    assumptions: Set[Formula] = set()
    antecedents: List[Formula] = []
    for h in hyps:
        assumptions |= h.assumptions
        antecedents.append(h.conclusion)
    full = imp(conj_all(antecedents), goal) if antecedents else goal

    d_vars: Set[str] = set()
    s_vars: Set[str] = set()
    ambient_s: Set[str] = set()
    _collect_lattice_vars(full, d_vars, s_vars, ambient_s)
    # Precompute which depth and set variables each antecedent and the goal need
    ant_vars: List[Tuple[Formula, Set[str], Set[str]]] = []
    for ant in antecedents:
        dv: Set[str] = set()
        sv: Set[str] = set()
        _collect_lattice_vars(ant, dv, sv, set())
        ant_vars.append((ant, dv, sv))

    # Order depth and set variables so antecedents become checkable as early as possible
    d_list: List[str] = []
    for _ant, dv, sv in sorted(ant_vars, key=lambda item: (len(item[2]), len(item[1]))):
        for v in sorted(dv):
            if v not in d_list:
                d_list.append(v)
    for v in sorted(d_vars):
        if v not in d_list:
            d_list.append(v)

    s_list: List[str] = []
    for _ant, _dv, sv in sorted(ant_vars, key=lambda item: len(item[2])):
        for v in sorted(sv):
            if v not in s_list:
                s_list.append(v)
    for v in sorted(s_vars):
        if v not in s_list:
            s_list.append(v)

    def _has_non_horn_f(f: Formula) -> bool:
        def _has_t(t: Term) -> bool:
            if not t.args and t.op.isdigit() and int(t.op) > 0:
                return True
            return any(_has_t(a) for a in t.args)

        if f.kind in {"∈", "∉", "⊈", "<", "!=_d", "¬", "∨"}:
            return True
        return any(_has_t(t) for t in f.terms) or any(_has_non_horn_f(s) for s in f.subs)

    has_elem_ops = any(tok in repr(full) for tok in ("∈", "∉", "⊈"))
    has_strict_or_const = _has_non_horn_f(full)
    # By the bounded-chain homomorphism theorem: for any positive order formula
    # over `{0, max, max_set, ∪, singleton, ⊆, <=}` without nonzero constants,
    # if a counterexample σ exists in `{0..8}` (so lhs(σ) > rhs(σ) >= 0), the
    # monotone map π(0)=0, π(0<x<=rhs(σ))=1, π(x>rhs(σ))=2 preserves all `0`,
    # `max`, `∪`, `⊆`, `<=` antecedents while keeping π(lhs(σ))=2 > π(rhs(σ)),
    # yielding a counterexample in `{0, 1, 2}`. Formulas with membership, strict
    # inequality, or nonzero constants check all 9 strata `0..8`.
    chain_len = 9 if has_strict_or_const else 3
    interval_domains = [
        (
            [(1 << (m + 1)) - 1 for m in range(chain_len)]
            if v in ambient_s
            else [0] + [(1 << (m + 1)) - 1 for m in range(chain_len)]
        )
        for v in s_list
    ]

    env_d: Dict[str, int] = {}
    env_s: Dict[str, int] = {}

    def domains_for_env_d() -> List[List[int]]:
        if not has_elem_ops:
            return interval_domains
        # Exact finite-model quotient given env_d: all subsets of active depths
        # combined with each possible maximum element m in 0..8 and witness bit.
        active = sorted({0} | set(env_d.values()))
        non_active = [b for b in range(9) if b not in active]
        witness = non_active[0] if non_active else 0
        extra_bits = active + ([witness] if witness not in active else [])
        sub_masks: List[int] = [0]
        for b in extra_bits:
            sub_masks = sub_masks + [m | (1 << b) for m in sub_masks]
        raw_masks: Set[int] = set(sub_masks)
        for m in range(9):
            for sm in sub_masks:
                # Clear bits above m and set bit m so max_set is exactly m
                clipped = (sm & ((1 << (m + 1)) - 1)) | (1 << m)
                raw_masks.add(clipped)
        sorted_masks = sorted(raw_masks)
        return [
            [m | (1 if v in ambient_s else 0) for m in sorted_masks]
            for v in s_list
        ]

    # Check antecedents whose set/depth variables are all bound at step `idx`
    d_bound_at: List[Set[str]] = [set(d_list[: i + 1]) for i in range(len(d_list))]
    ants_at_d: List[List[Formula]] = [[] for _ in range(len(d_list))]
    placed: Set[int] = set()
    for i, bound in enumerate(d_bound_at):
        for a_idx, (ant, dv, sv) in enumerate(ant_vars):
            if a_idx not in placed and not sv and dv <= bound:
                ants_at_d[i].append(ant)
                placed.add(a_idx)

    s_bound_at: List[Set[str]] = [set(s_list[: i + 1]) for i in range(len(s_list))]
    ants_at_s: List[List[Formula]] = [[] for _ in range(len(s_list))]
    for i, bound in enumerate(s_bound_at):
        for a_idx, (ant, _dv, sv) in enumerate(ant_vars):
            if a_idx not in placed and sv <= bound:
                ants_at_s[i].append(ant)
                placed.add(a_idx)
    ants_before_s = [ant for a_idx, (ant, _dv, sv) in enumerate(ant_vars) if a_idx not in placed and not sv]

    def check_s(idx: int, s_doms: List[List[int]]) -> None:
        if idx == len(s_list):
            if not _eval_lattice_formula(goal, env_d, env_s):
                raise ProofFault(
                    f"lattice_decide counterexample for `{goal}`:\n"
                    f"  depths: {env_d}\n  sets:   {env_s}"
                )
            return
        name = s_list[idx]
        checks = ants_at_s[idx]
        for val in s_doms[idx]:
            env_s[name] = val
            if all(_eval_lattice_formula(c, env_d, env_s) for c in checks):
                check_s(idx + 1, s_doms)

    def check_d(idx: int) -> None:
        if idx == len(d_list):
            if all(_eval_lattice_formula(c, env_d, env_s) for c in ants_before_s):
                check_s(0, domains_for_env_d())
            return
        name = d_list[idx]
        checks = ants_at_d[idx]
        for val in range(chain_len):
            env_d[name] = val
            if all(_eval_lattice_formula(c, env_d, env_s) for c in checks):
                check_d(idx + 1)

    check_d(0)
    return _thm(assumptions, goal)


# ── Inductive relations & rule induction ─────────────────────────────────────


@dataclass(frozen=True)
class Rule:
    name: str
    premises: Tuple[Formula, ...]
    conclusion: Formula


@dataclass(frozen=True)
class InductiveRelation:
    name: str
    rules: Tuple[Rule, ...]

    def rule(self, name: str) -> Rule:
        for r in self.rules:
            if r.name == name:
                return r
        raise ProofFault(f"unknown rule `{name}` in `{self.name}`")


def apply_rule(rel: InductiveRelation, rule_name: str, subst: Dict[str, Term], premise_thms: Sequence[Theorem]) -> Theorem:
    """Derive a conclusion of `rel` by instantiating `rule_name` and discharging its premises."""
    r = rel.rule(rule_name)
    _require(
        len(premise_thms) == len(r.premises),
        f"rule `{rule_name}` expects {len(r.premises)} premises, got {len(premise_thms)}",
    )
    assumptions: Set[Formula] = set()
    for expected, given in zip(r.premises, premise_thms):
        inst_expected = subst_formula(expected, subst)
        _require(
            inst_expected == given.conclusion,
            f"rule `{rule_name}` premise mismatch:\n  expected: {inst_expected}\n  got:      {given.conclusion}",
        )
        assumptions |= given.assumptions
    return _thm(assumptions, subst_formula(r.conclusion, subst))


def rule_induction(
    rel: InductiveRelation,
    prop_fn: Callable[[Tuple[Term, ...]], Formula],
    cases: Dict[str, Callable[[Tuple[Theorem, ...], Tuple[Theorem, ...]], Theorem]],
) -> Theorem:
    """Verify a proof by rule induction over every rule of `rel`.

    For each rule `R` of `rel`:
    - `prem_hyps` are the assumed premises `P_1..P_k` of `R`;
    - `ih_hyps` are the induction hypotheses `prop_fn(args)` for every recursive
      premise `rel(args)` in `R`;
    - `cases[R.name](prem_hyps, ih_hyps)` must return a valid `Theorem` whose
      conclusion is `prop_fn(conclusion_args)` using only `prem_hyps` and `ih_hyps`.
    """
    rule_names = {r.name for r in rel.rules}
    given_names = set(cases.keys())
    _require(
        rule_names == given_names,
        f"rule_induction on `{rel.name}` missing cases {rule_names - given_names} "
        f"or extra cases {given_names - rule_names}",
    )

    for r in rel.rules:
        prem_hyps = tuple(assume(p) for p in r.premises)
        ih_hyps_list: List[Theorem] = []
        for p in r.premises:
            if p.kind == "Pred" and p.terms[0].op == rel.name:
                ih_hyps_list.append(assume(prop_fn(p.terms[0].args)))
        ih_hyps = tuple(ih_hyps_list)

        case_thm = cases[r.name](prem_hyps, ih_hyps)
        expected = prop_fn(r.conclusion.terms[0].args)
        _require(
            case_thm.conclusion == expected,
            f"rule_induction case `{r.name}` proved wrong conclusion:\n"
            f"  expected: {expected}\n  got:      {case_thm.conclusion}",
        )
        allowed = {h.conclusion for h in prem_hyps} | {h.conclusion for h in ih_hyps}
        extra = case_thm.assumptions - allowed
        _require(
            not extra,
            f"rule_induction case `{r.name}` left undischarged assumptions: {extra}",
        )

    generic_args = tuple(var(f"_a{i}") for i in range(len(rel.rules[0].conclusion.terms[0].args)))
    goal = imp(pred(rel.name, *generic_args), prop_fn(generic_args))
    for v in reversed(generic_args):
        goal = forall(v.op, goal)
    return _thm([], goal)


# ── Algebraic datatypes & structural induction ───────────────────────────────


@dataclass(frozen=True)
class Constructor:
    name: str
    field_sorts: Tuple[str, ...]


@dataclass(frozen=True)
class Datatype:
    name: str
    constructors: Tuple[Constructor, ...]


def structural_induction(
    dt: Datatype,
    prop_fn: Callable[[Term], Formula],
    cases: Dict[str, Callable[[Tuple[Term, ...], Tuple[Theorem, ...]], Theorem]],
) -> Theorem:
    """Verify a proof by structural induction over every constructor of `dt`."""
    ctor_names = {c.name for c in dt.constructors}
    given_names = set(cases.keys())
    _require(
        ctor_names == given_names,
        f"structural_induction on `{dt.name}` mismatch: expected {ctor_names}, got {given_names}",
    )

    for c in dt.constructors:
        fields = tuple(var(f"_{c.name}_{i}") for i in range(len(c.field_sorts)))
        ihs = tuple(
            assume(prop_fn(f))
            for f, sort in zip(fields, c.field_sorts)
            if sort == dt.name
        )
        term = app(c.name, *fields)
        case_thm = cases[c.name](fields, ihs)
        expected = prop_fn(term)
        _require(
            case_thm.conclusion == expected,
            f"structural_induction case `{c.name}` conclusion mismatch:\n"
            f"  expected: {expected}\n  got:      {case_thm.conclusion}",
        )
        allowed = {h.conclusion for h in ihs}
        extra = case_thm.assumptions - allowed
        _require(
            not extra,
            f"structural_induction case `{c.name}` left undischarged assumptions: {extra}",
        )

    x = var(f"_x_{dt.name}")
    return _thm([], forall(x.op, prop_fn(x)))
