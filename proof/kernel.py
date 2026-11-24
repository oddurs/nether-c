#!/usr/bin/env python3
"""LCF-style proof kernel for Nether C metatheory.

A `Theorem` can only be constructed by this module's inference rules using the
private `_SEAL` token. Any file in `proof/` that mentions `admit`, `sorry`, or
`unproved_axiom` is rejected by `proof/check.py` before checking begins.
"""
from __future__ import annotations
from dataclasses import dataclass
from typing import Callable, Dict, Iterable, List, Optional, Sequence, Set, Tuple
import weakref

_SEAL = object()


_VALID_FORMULA_KINDS = frozenset(
    {"Pred", "=", "=_d", "!=", "!=_d", "<=", "<", "∈", "∉", "⊆", "⊈", "¬", "∧", "∨", "→", "↔", "∀", "∃"}
)
_BINARY_TERM_KINDS = frozenset({"=", "=_d", "!=_d", "<=", "<", "∈", "∉", "⊆", "⊈"})
_BINARY_SUB_KINDS = frozenset({"∧", "∨", "→", "↔"})


@dataclass(frozen=True)
class Term:
    op: str
    args: Tuple["Term", ...] = ()

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Term cannot be subclassed")

    def __post_init__(self) -> None:
        _require(type(self.op) is str and bool(self.op), f"Term op must be non-empty str, got {self.op!r}")
        _require(
            type(self.args) is tuple and all(type(a) is Term for a in self.args),
            f"Term args must be tuple[Term, ...], got {self.args!r}",
        )

    def __repr__(self) -> str:
        if not self.args:
            return self.op
        return f"{self.op}({', '.join(repr(a) for a in self.args)})"


def _require_term(t: Term, where: str) -> None:
    _require(type(t) is Term, f"{where}: expected Term, got {type(t).__name__}")


def var(name: str) -> Term:
    return Term(name, ())


def app(op: str, *args: Term) -> Term:
    return Term(op, tuple(args))


RESERVED_CONSTANTS = frozenset({"∅", "∅_raw", "True", "False", "U0", "Cairn", "Bool", "VUnit", "Nil"})


def _is_ascii_digits(name: str) -> bool:
    return bool(name) and name.isascii() and name.isdigit()


def _is_constant_name(name: str) -> bool:
    return _is_ascii_digits(name) or name in RESERVED_CONSTANTS


def _require_var_name(name: str, where: str) -> None:
    _require(
        type(name) is str
        and bool(name)
        and not _is_constant_name(name)
        and not any(ch in name for ch in ("(", ")", ",", " ", "\t", "\n")),
        f"{where}: invalid variable name `{name}`",
    )


def _clean_subst(mapping: Dict[str, Term], where: str) -> Dict[str, Term]:
    _require(isinstance(mapping, dict), f"{where}: expected mapping dict")
    clean: Dict[str, Term] = {}
    for k, v in dict.items(mapping):
        _require_var_name(k, where)
        _require_term(v, where)
        clean[k] = v
    return clean


def _subst_term(t: Term, mapping: Dict[str, Term]) -> Term:
    _require_term(t, "subst_term")
    if not t.args and not _is_constant_name(t.op) and t.op in mapping:
        return mapping[t.op]
    return Term(t.op, tuple(_subst_term(a, mapping) for a in t.args))


def subst_term(t: Term, mapping: Dict[str, Term]) -> Term:
    return _subst_term(t, _clean_subst(mapping, "subst_term"))


def free_vars_term(t: Term) -> Set[str]:
    _require_term(t, "free_vars_term")
    if not t.args:
        if _is_constant_name(t.op):
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

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Formula cannot be subclassed")

    def __post_init__(self) -> None:
        _require(type(self.kind) is str, f"Formula kind must be str, got {self.kind!r}")
        _require(
            type(self.terms) is tuple and all(type(t) is Term for t in self.terms),
            f"Formula terms must be tuple[Term, ...], got {self.terms!r}",
        )
        _require(
            type(self.subs) is tuple and all(type(s) is Formula for s in self.subs),
            f"Formula subs must be tuple[Formula, ...], got {self.subs!r}",
        )
        _require(type(self.binder) is str, f"Formula binder must be str, got {self.binder!r}")
        if self.kind == "Pred":
            _require(len(self.terms) == 1 and not self.subs and self.binder == "", f"malformed Pred: {self!r}")
        elif self.kind in _BINARY_TERM_KINDS:
            _require(len(self.terms) == 2 and not self.subs and self.binder == "", f"malformed {self.kind}: {self!r}")
        elif self.kind == "¬":
            _require(not self.terms and len(self.subs) == 1 and self.binder == "", f"malformed ¬: {self!r}")
        elif self.kind in _BINARY_SUB_KINDS:
            _require(not self.terms and len(self.subs) == 2 and self.binder == "", f"malformed {self.kind}: {self!r}")
        elif self.kind in {"∀", "∃"}:
            _require(not self.terms and len(self.subs) == 1, f"malformed quantifier: {self!r}")
            _require_var_name(self.binder, "Formula")
        else:
            raise ProofFault(f"unknown Formula kind `{self.kind}`")

    def __repr__(self) -> str:
        if self.kind == "Pred":
            return repr(self.terms[0])
        if self.kind in _BINARY_TERM_KINDS:
            return f"({self.terms[0]} {self.kind} {self.terms[1]})"
        if self.kind == "¬":
            return f"¬{self.subs[0]}"
        if self.kind in _BINARY_SUB_KINDS:
            return f"({self.subs[0]} {self.kind} {self.subs[1]})"
        if self.kind in {"∀", "∃"}:
            return f"({self.kind}{self.binder}. {self.subs[0]})"
        return f"{self.kind}({self.terms}, {self.subs})"


def _require_formula(f: Formula, where: str) -> None:
    _require(type(f) is Formula, f"{where}: expected Formula, got {type(f).__name__}")


def pred(name: str, *args: Term) -> Formula:
    return Formula("Pred", (Term(name, tuple(args)),))


def eq(a: Term, b: Term) -> Formula:
    return Formula("=", (a, b))


def eq_d(a: Term, b: Term) -> Formula:
    return Formula("=_d", (a, b))


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
    _require(bool(items), "conj_all requires at least one formula")
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
    _require_var_name(binder, "forall")
    return Formula("∀", (), (body,), binder)


def free_vars_formula(f: Formula) -> Set[str]:
    if f.kind in {"∀", "∃"}:
        return free_vars_formula(f.subs[0]) - {f.binder}
    if f.kind == "Pred":
        out: Set[str] = set()
        for a in f.terms[0].args:
            out |= free_vars_term(a)
        return out
    out: Set[str] = set()
    for t in f.terms:
        out |= free_vars_term(t)
    for s in f.subs:
        out |= free_vars_formula(s)
    return out


def _fresh_var(base: str, avoid: Set[str]) -> str:
    candidate = f"{base}_"
    idx = 0
    while candidate in avoid:
        idx += 1
        candidate = f"{base}_{idx}"
    return candidate


def _subst_formula(f: Formula, mapping: Dict[str, Term]) -> Formula:
    _require_formula(f, "subst_formula")
    if f.kind in {"∀", "∃"}:
        inner_map = {k: v for k, v in mapping.items() if k != f.binder}
        if not inner_map:
            return f
        body = f.subs[0]
        body_fv = free_vars_formula(body)
        active = {k: v for k, v in inner_map.items() if k in body_fv}
        if not active:
            return f
        captured: Set[str] = set()
        for t in active.values():
            captured |= free_vars_term(t)
        binder = f.binder
        if binder in captured:
            avoid = body_fv | captured | set(active.keys()) | {binder}
            fresh = _fresh_var(binder, avoid)
            body = _subst_formula(body, {binder: var(fresh)})
            binder = fresh
        return Formula(f.kind, (), (_subst_formula(body, active),), binder)
    if f.kind == "Pred":
        return pred(f.terms[0].op, *(_subst_term(a, mapping) for a in f.terms[0].args))
    return Formula(
        f.kind,
        tuple(_subst_term(t, mapping) for t in f.terms),
        tuple(_subst_formula(s, mapping) for s in f.subs),
        f.binder,
    )


def subst_formula(f: Formula, mapping: Dict[str, Term]) -> Formula:
    return _subst_formula(f, _clean_subst(mapping, "subst_formula"))


class ProofFault(Exception):
    """Raised when an invalid derivation is presented to the kernel."""


def _require(cond: bool, msg: str) -> None:
    if not cond:
        raise ProofFault(msg)


_THEOREM_DATA: Dict[int, Tuple["weakref.ref[Theorem]", frozenset[Formula], Formula]] = {}


def _thm_payload(thm: object) -> Optional[Tuple[frozenset[Formula], Formula]]:
    if type(thm) is not Theorem:
        return None
    entry = _THEOREM_DATA.get(id(thm))
    if entry is None or entry[0]() is not thm:
        return None
    return entry[1], entry[2]


class Theorem:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Theorem cannot be subclassed")

    def __init__(self, _token: object, assumptions: frozenset[Formula], conclusion: Formula) -> None:
        _require(_token is _SEAL, "Theorem constructed outside kernel")
        _require(
            type(assumptions) is frozenset and all(type(a) is Formula for a in assumptions),
            "Theorem assumptions must be frozenset[Formula]",
        )
        _require_formula(conclusion, "Theorem")
        oid = id(self)
        _THEOREM_DATA[oid] = (weakref.ref(self), assumptions, conclusion)
        weakref.finalize(self, _THEOREM_DATA.pop, oid, None)

    @property
    def assumptions(self) -> frozenset[Formula]:
        payload = _thm_payload(self)
        _require(payload is not None, "unsealed Theorem")
        assert payload is not None
        return payload[0]

    @property
    def conclusion(self) -> Formula:
        payload = _thm_payload(self)
        _require(payload is not None, "unsealed Theorem")
        assert payload is not None
        return payload[1]

    def __eq__(self, other: object) -> bool:
        p1 = _thm_payload(self)
        p2 = _thm_payload(other)
        return p1 is not None and p2 is not None and p1 == p2

    def __hash__(self) -> int:
        payload = _thm_payload(self)
        _require(payload is not None, "unsealed Theorem")
        return hash(payload)

    def __repr__(self) -> str:
        hyps = ", ".join(repr(a) for a in sorted(self.assumptions, key=repr))
        return f"{hyps} ⊢ {self.conclusion}"


def _require_thm(thm: Theorem, where: str) -> None:
    _require(_thm_payload(thm) is not None, f"{where}: expected kernel Theorem")


def _thm(assumptions: Iterable[Formula], conclusion: Formula) -> Theorem:
    return Theorem(_SEAL, frozenset(assumptions), conclusion)


# ── Primitive logical inference rules ────────────────────────────────────────


def assume(f: Formula) -> Theorem:
    _require_formula(f, "assume")
    return _thm([f], f)


def refl(t: Term) -> Theorem:
    _require_term(t, "refl")
    return _thm([], eq(t, t))


def sym(thm: Theorem) -> Theorem:
    _require_thm(thm, "sym")
    _require(thm.conclusion.kind == "=", f"sym expects equality, got {thm.conclusion}")
    a, b = thm.conclusion.terms
    return _thm(thm.assumptions, eq(b, a))


def trans(t1: Theorem, t2: Theorem) -> Theorem:
    _require_thm(t1, "trans")
    _require_thm(t2, "trans")
    _require(t1.conclusion.kind == "=" and t2.conclusion.kind == "=", "trans expects =")
    _require(t1.conclusion.terms[1] == t2.conclusion.terms[0], "trans middle mismatch")
    return _thm(
        t1.assumptions | t2.assumptions,
        eq(t1.conclusion.terms[0], t2.conclusion.terms[1]),
    )


def cong(op: str, eq_thms: Sequence[Theorem]) -> Theorem:
    _require(type(op) is str and bool(op), f"cong expects non-empty op, got {op!r}")
    assumptions: Set[Formula] = set()
    lhs_args: List[Term] = []
    rhs_args: List[Term] = []
    for t in eq_thms:
        _require_thm(t, "cong")
        _require(t.conclusion.kind == "=", f"cong expects =, got {t.conclusion}")
        assumptions |= t.assumptions
        lhs_args.append(t.conclusion.terms[0])
        rhs_args.append(t.conclusion.terms[1])
    return _thm(assumptions, eq(Term(op, tuple(lhs_args)), Term(op, tuple(rhs_args))))


def mp(imp_thm: Theorem, ant_thm: Theorem) -> Theorem:
    _require_thm(imp_thm, "mp")
    _require_thm(ant_thm, "mp")
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
    _require_formula(hyp, "discharge")
    _require_thm(thm, "discharge")
    return _thm(thm.assumptions - {hyp}, imp(hyp, thm.conclusion))


def conj_intro(t1: Theorem, t2: Theorem) -> Theorem:
    _require_thm(t1, "conj_intro")
    _require_thm(t2, "conj_intro")
    return _thm(t1.assumptions | t2.assumptions, conj(t1.conclusion, t2.conclusion))


def conj_elim_l(thm: Theorem) -> Theorem:
    _require_thm(thm, "conj_elim_l")
    _require(thm.conclusion.kind == "∧", f"conj_elim_l expects ∧, got {thm.conclusion}")
    return _thm(thm.assumptions, thm.conclusion.subs[0])


def conj_elim_r(thm: Theorem) -> Theorem:
    _require_thm(thm, "conj_elim_r")
    _require(thm.conclusion.kind == "∧", f"conj_elim_r expects ∧, got {thm.conclusion}")
    return _thm(thm.assumptions, thm.conclusion.subs[1])


def iff_intro(ab: Theorem, ba: Theorem) -> Theorem:
    _require_thm(ab, "iff_intro")
    _require_thm(ba, "iff_intro")
    _require(ab.conclusion.kind == "→" and ba.conclusion.kind == "→", "iff_intro expects →")
    a, b = ab.conclusion.subs
    b2, a2 = ba.conclusion.subs
    _require(a == a2 and b == b2, "iff_intro mismatch")
    return _thm(ab.assumptions | ba.assumptions, iff(a, b))


def inst(thm: Theorem, term: Term) -> Theorem:
    _require_thm(thm, "inst")
    _require_term(term, "inst")
    _require(thm.conclusion.kind == "∀", f"inst expects ∀, got {thm.conclusion}")
    body = subst_formula(thm.conclusion.subs[0], {thm.conclusion.binder: term})
    return _thm(thm.assumptions, body)


def inst_all(thm: Theorem, terms: Sequence[Term]) -> Theorem:
    out = thm
    for t in terms:
        out = inst(out, t)
    return out


def gen(binder: str, thm: Theorem) -> Theorem:
    _require_var_name(binder, "gen")
    _require_thm(thm, "gen")
    for a in thm.assumptions:
        _require(
            binder not in free_vars_formula(a),
            f"cannot generalize `{binder}` while free in assumption {a}",
        )
    return _thm(thm.assumptions, forall(binder, thm.conclusion))


def rewrite(eq_thm: Theorem, target: Theorem) -> Theorem:
    _require_thm(eq_thm, "rewrite")
    _require_thm(target, "rewrite")
    _require(eq_thm.conclusion.kind == "=", f"rewrite expects =, got {eq_thm.conclusion}")
    src, dst = eq_thm.conclusion.terms
    src_fv = free_vars_term(src)
    dst_fv = free_vars_term(dst)

    def replace_t(t: Term) -> Term:
        if t == src:
            return dst
        return Term(t.op, tuple(replace_t(a) for a in t.args))

    def replace_f(f: Formula) -> Formula:
        if f.kind in {"∀", "∃"}:
            # If the bound variable appears free in `src`, occurrences inside the
            # binder refer to the bound variable rather than the free one in `eq_thm`.
            if f.binder in src_fv:
                return f
            body = f.subs[0]
            binder = f.binder
            if binder in dst_fv:
                avoid = free_vars_formula(body) | src_fv | dst_fv | {binder}
                fresh = _fresh_var(binder, avoid)
                body = subst_formula(body, {binder: var(fresh)})
                binder = fresh
            return Formula(f.kind, (), (replace_f(body),), binder)
        if f.kind == "Pred":
            return pred(f.terms[0].op, *(replace_t(a) for a in f.terms[0].args))
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

SET_OPS = frozenset({"singleton", "∪"})
SET_CONSTS = frozenset({"∅", "∅_raw"})
DEPTH_OPS = frozenset({"max", "max_set"})


def _eval_depth(t: Term, env_d: Dict[Term, int], env_s: Dict[Term, int]) -> int:
    if not t.args:
        if _is_ascii_digits(t.op):
            v = int(t.op)
            _require(0 <= v <= 8, f"depth literal {v} out of range 0..8")
            return v
        return env_d[t]
    if t.op == "max":
        return max(_eval_depth(a, env_d, env_s) for a in t.args)
    if t.op == "max_set":
        mask = _eval_set(t.args[0], env_d, env_s)
        for bit in range(8, -1, -1):
            if mask & (1 << bit):
                return bit
        return 0
    return env_d[t]


def _eval_set(t: Term, env_d: Dict[Term, int], env_s: Dict[Term, int]) -> int:
    if not t.args:
        if t.op == "∅":
            return 1  # §2.1: stratum 0 is always present in ambient/capability sets
        if t.op == "∅_raw":
            return 0  # raw empty set for g(e)
        return env_s[t]
    if t.op == "singleton":
        d = _eval_depth(t.args[0], env_d, env_s)
        return 1 << d
    if t.op == "∪":
        out = 0
        for a in t.args:
            out |= _eval_set(a, env_d, env_s)
        return out
    return env_s[t]


def _collect_lattice_vars(
    f: Formula,
    d_vars: Set[Term],
    s_vars: Set[Term],
    full_s: Set[Term],
) -> None:
    """Collect depth and set atoms (keyed by Term, not repr), enforcing sort
    separation and recording which set atoms are observed beyond `max_set`."""

    def scan_d(t: Term) -> None:
        if not t.args:
            _require(t.op not in SET_CONSTS, f"set constant `{t}` used in depth position")
            if _is_ascii_digits(t.op):
                v = int(t.op)
                _require(0 <= v <= 8, f"depth literal {v} out of range 0..8")
            else:
                _require(
                    t.op not in RESERVED_CONSTANTS and not t.op.isdigit(),
                    f"non-depth constant `{t}` used in depth position",
                )
                d_vars.add(t)
        elif t.op == "max":
            for a in t.args:
                scan_d(a)
        elif t.op == "max_set":
            _require(len(t.args) == 1, f"max_set expects 1 argument, got {t}")
            scan_s(t.args[0], observed_full=False)
        else:
            _require(t.op not in SET_OPS, f"set term `{t}` used in depth position")
            d_vars.add(t)

    def scan_s(t: Term, *, observed_full: bool) -> None:
        if not t.args:
            _require(
                not _is_ascii_digits(t.op) and not t.op.isdigit(),
                f"depth literal `{t}` used in set position",
            )
            if t.op not in SET_CONSTS:
                _require(
                    t.op not in RESERVED_CONSTANTS,
                    f"non-set constant `{t}` used in set position",
                )
                s_vars.add(t)
                if observed_full:
                    full_s.add(t)
        elif t.op == "singleton":
            _require(len(t.args) == 1, f"singleton expects 1 argument, got {t}")
            scan_d(t.args[0])
        elif t.op == "∪":
            for a in t.args:
                scan_s(a, observed_full=observed_full)
        else:
            _require(t.op not in DEPTH_OPS, f"depth term `{t}` used in set position")
            s_vars.add(t)
            if observed_full:
                full_s.add(t)

    if f.kind in {"<=", "<", "=_d", "!=_d"}:
        scan_d(f.terms[0])
        scan_d(f.terms[1])
    elif f.kind in {"∈", "∉"}:
        scan_d(f.terms[0])
        scan_s(f.terms[1], observed_full=True)
    elif f.kind in {"⊆", "⊈"}:
        scan_s(f.terms[0], observed_full=True)
        scan_s(f.terms[1], observed_full=True)
    elif f.kind in {"¬", "∧", "∨", "→", "↔"}:
        for sub in f.subs:
            _collect_lattice_vars(sub, d_vars, s_vars, full_s)
    else:
        raise ProofFault(f"non-lattice formula `{f}` in lattice_decide")


def _eval_lattice_formula(f: Formula, env_d: Dict[Term, int], env_s: Dict[Term, int]) -> bool:
    def ed(t: Term) -> int:
        if t.args and t.op not in DEPTH_OPS:
            return env_d[t]
        return _eval_depth(t, env_d, env_s)

    def es(t: Term) -> int:
        if t.args and t.op not in SET_OPS:
            return env_s[t]
        return _eval_set(t, env_d, env_s)

    if f.kind == "<=":
        return ed(f.terms[0]) <= ed(f.terms[1])
    if f.kind == "<":
        return ed(f.terms[0]) < ed(f.terms[1])
    if f.kind == "=_d":
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


def _split_conj(f: Formula) -> List[Formula]:
    if f.kind == "∧":
        return _split_conj(f.subs[0]) + _split_conj(f.subs[1])
    return [f]


def _has_set_rel(f: Formula) -> bool:
    if f.kind in {"∈", "∉", "⊆", "⊈"}:
        return True
    return any(_has_set_rel(s) for s in f.subs)


def lattice_decide(goal: Formula, hyps: Sequence[Theorem] = ()) -> Theorem:
    """Soundly verify a universal lattice/capability-set formula over {0..8}.

    Every set variable observed by membership/inclusion (`∈`, `∉`, `⊆`, `⊈`) is
    checked over the **exact full powerset** of `{0..8}` (all 512 bitmasks). A
    set variable observed solely through `max_set` is checked over its exact
    `max_set` quotient (`∅` and intervals `{0..m}`). No implicit name-based
    axioms are assumed on variables.

    When `goal` uses no set relations (`∈`/`∉`/`⊆`/`⊈`), a top-level positive
    antecedent `A ⊆ B` may be soundly weakened to `max_set(A) <= max_set(B)`:
    since `A ⊆ B → max_set(A) <= max_set(B)` holds universally, validity of the
    weakened implication entails validity of the original implication.
    """
    _require_formula(goal, "lattice_decide")
    assumptions: Set[Formula] = set()
    raw_antecedents: List[Formula] = []
    for h in hyps:
        _require_thm(h, "lattice_decide")
        assumptions |= h.assumptions
        raw_antecedents.extend(_split_conj(h.conclusion))

    # Unpack top-level implications in `goal` for earlier antecedent pruning.
    check_goal = goal
    while check_goal.kind == "→":
        raw_antecedents.extend(_split_conj(check_goal.subs[0]))
        check_goal = check_goal.subs[1]

    # Validate original formula sorts before any sound weakening.
    orig_d: Set[Term] = set()
    orig_s: Set[Term] = set()
    orig_full: Set[Term] = set()
    for ant in raw_antecedents:
        _collect_lattice_vars(ant, orig_d, orig_s, orig_full)
    _collect_lattice_vars(check_goal, orig_d, orig_s, orig_full)
    _require(orig_d.isdisjoint(orig_s), f"variable used at both depth and set sort: {orig_d & orig_s}")

    # Sound weakening: if `check_goal` has no set relations, weaken positive
    # top-level `A ⊆ B` antecedents to `max_set(A) <= max_set(B)` so pure
    # depth-bound proofs (`APP`) do not explode over unrelated set masks.
    antecedents: List[Formula] = []
    if not _has_set_rel(check_goal):
        for ant in raw_antecedents:
            if ant.kind == "⊆":
                antecedents.append(le(app("max_set", ant.terms[0]), app("max_set", ant.terms[1])))
            else:
                antecedents.append(ant)
    else:
        antecedents = list(raw_antecedents)

    d_vars: Set[Term] = set()
    s_vars: Set[Term] = set()
    full_s: Set[Term] = set()
    for ant in antecedents:
        _collect_lattice_vars(ant, d_vars, s_vars, full_s)
    _collect_lattice_vars(check_goal, d_vars, s_vars, full_s)

    # Full-domain checking over `{0..8}` for all depths and exact bitmasks for
    # every set variable in `full_s`. Only flat Horn clauses (`∧ atoms → atom`)
    # over positive order with `full_s == empty` and no nonzero constants use
    # the bounded-chain homomorphism (`{0, 1, 2}`).
    def _has_non_horn_f(f: Formula) -> bool:
        def _has_t(t: Term) -> bool:
            if not t.args and _is_ascii_digits(t.op) and int(t.op) > 0:
                return True
            return any(_has_t(a) for a in t.args)

        if f.kind in {"∈", "∉", "⊆", "⊈", "<", "=_d", "!=_d", "¬", "∨", "↔", "→"}:
            return True
        return any(_has_t(t) for t in f.terms) or any(_has_non_horn_f(s) for s in f.subs)

    non_horn = any(_has_non_horn_f(a) for a in antecedents) or _has_non_horn_f(check_goal)
    chain_len = 9 if (full_s or non_horn) else 3

    # Build variable order so antecedents prune as early as possible.
    ant_vars: List[Tuple[Formula, Set[Term], Set[Term]]] = []
    for ant in antecedents:
        dv: Set[Term] = set()
        sv: Set[Term] = set()
        _collect_lattice_vars(ant, dv, sv, set())
        ant_vars.append((ant, dv, sv))

    # If antecedents constrain set-only pairs (e.g. `δ_A ⊆ δ_B`), order sets first.
    s_only_ants = [item for item in ant_vars if not item[1] and item[2]]
    sets_first = bool(s_only_ants and full_s)

    def _term_key(t: Term) -> Tuple[str, int, str]:
        return (t.op, len(t.args), repr(t))

    d_list: List[Term] = []
    for _ant, dv, sv in sorted(ant_vars, key=lambda item: (len(item[2]), len(item[1]))):
        for v in sorted(dv, key=_term_key):
            if v not in d_list:
                d_list.append(v)
    for v in sorted(d_vars, key=_term_key):
        if v not in d_list:
            d_list.append(v)

    s_list: List[Term] = []
    for _ant, _dv, sv in sorted(ant_vars, key=lambda item: (len(item[1]), len(item[2]))):
        for v in sorted(sv, key=_term_key):
            if v not in s_list:
                s_list.append(v)
    for v in sorted(s_vars, key=_term_key):
        if v not in s_list:
            s_list.append(v)

    def _domain_for_set(atom: Term) -> List[int]:
        if atom in full_s:
            return list(range(512))
        return [0] + [(1 << (m + 1)) - 1 for m in range(chain_len)]

    s_doms = [_domain_for_set(v) for v in s_list]
    env_d: Dict[Term, int] = {}
    env_s: Dict[Term, int] = {}

    if sets_first:
        s_bound_at = [set(s_list[: i + 1]) for i in range(len(s_list))]
        ants_at_s: List[List[Formula]] = [[] for _ in range(len(s_list))]
        placed: Set[int] = set()
        for i, bound in enumerate(s_bound_at):
            for a_idx, (ant, dv, sv) in enumerate(ant_vars):
                if a_idx not in placed and not dv and sv <= bound:
                    ants_at_s[i].append(ant)
                    placed.add(a_idx)
        d_bound_at = [set(d_list[: i + 1]) for i in range(len(d_list))]
        ants_at_d: List[List[Formula]] = [[] for _ in range(len(d_list))]
        for i, bound in enumerate(d_bound_at):
            for a_idx, (ant, dv, _sv) in enumerate(ant_vars):
                if a_idx not in placed and dv <= bound:
                    ants_at_d[i].append(ant)
                    placed.add(a_idx)
        ants_before = [ant for a_idx, (ant, _dv, _sv) in enumerate(ant_vars) if a_idx not in placed]

        def check_d(idx: int) -> None:
            if idx == len(d_list):
                if not _eval_lattice_formula(check_goal, env_d, env_s):
                    raise ProofFault(
                        f"lattice_decide counterexample for `{goal}`:\n"
                        f"  depths: {env_d}\n  sets:   {env_s}"
                    )
                return
            name = d_list[idx]
            checks = ants_at_d[idx]
            for val in range(chain_len):
                env_d[name] = val
                if all(_eval_lattice_formula(c, env_d, env_s) for c in checks):
                    check_d(idx + 1)

        def check_s(idx: int) -> None:
            if idx == len(s_list):
                if all(_eval_lattice_formula(c, env_d, env_s) for c in ants_before):
                    check_d(0)
                return
            name = s_list[idx]
            checks = ants_at_s[idx]
            for val in s_doms[idx]:
                env_s[name] = val
                if all(_eval_lattice_formula(c, env_d, env_s) for c in checks):
                    check_s(idx + 1)

        check_s(0)
    else:
        d_bound_at = [set(d_list[: i + 1]) for i in range(len(d_list))]
        ants_at_d = [[] for _ in range(len(d_list))]
        placed = set()
        for i, bound in enumerate(d_bound_at):
            for a_idx, (ant, dv, sv) in enumerate(ant_vars):
                if a_idx not in placed and not sv and dv <= bound:
                    ants_at_d[i].append(ant)
                    placed.add(a_idx)
        s_bound_at = [set(s_list[: i + 1]) for i in range(len(s_list))]
        ants_at_s = [[] for _ in range(len(s_list))]
        for i, bound in enumerate(s_bound_at):
            for a_idx, (ant, _dv, sv) in enumerate(ant_vars):
                if a_idx not in placed and sv <= bound:
                    ants_at_s[i].append(ant)
                    placed.add(a_idx)
        ants_before = [ant for a_idx, (ant, _dv, _sv) in enumerate(ant_vars) if a_idx not in placed]

        def check_s(idx: int) -> None:
            if idx == len(s_list):
                if not _eval_lattice_formula(check_goal, env_d, env_s):
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
                    check_s(idx + 1)

        def check_d(idx: int) -> None:
            if idx == len(d_list):
                if all(_eval_lattice_formula(c, env_d, env_s) for c in ants_before):
                    check_s(0)
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


def _mentions_pred(f: Formula, name: str) -> bool:
    if f.kind == "Pred" and f.terms[0].op == name:
        return True
    return any(_mentions_pred(s, name) for s in f.subs)


def _mentions_any_pred(f: Formula) -> bool:
    if f.kind == "Pred":
        return True
    return any(_mentions_any_pred(s) for s in f.subs)


def _is_strictly_positive_premise(f: Formula) -> bool:
    """Allow predicates only in strictly positive positions (never under `¬`,
    `↔`, or the antecedent of `→`), preventing both direct and mutually negative
    inductive definitions."""
    if f.kind in {"¬", "↔"}:
        return not _mentions_any_pred(f)
    if f.kind == "→":
        return (not _mentions_any_pred(f.subs[0])) and _is_strictly_positive_premise(f.subs[1])
    if f.kind in {"∧", "∨", "∀", "∃"}:
        return all(_is_strictly_positive_premise(s) for s in f.subs)
    return True


@dataclass(frozen=True)
class Rule:
    name: str
    premises: Tuple[Formula, ...]
    conclusion: Formula

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Rule cannot be subclassed")

    def __post_init__(self) -> None:
        _require(type(self.name) is str and bool(self.name), "Rule requires a non-empty name")
        _require(
            type(self.premises) is tuple and all(type(p) is Formula for p in self.premises),
            f"Rule `{self.name}` premises must be tuple[Formula, ...]",
        )
        _require_formula(self.conclusion, f"Rule `{self.name}`")


_RELATIONS: Dict[str, Tuple["InductiveRelation", Tuple[Rule, ...]]] = {}
_REL_DATA: Dict[int, Tuple["weakref.ref[InductiveRelation]", str, Tuple[Rule, ...]]] = {}


def _rel_payload(rel: object) -> Optional[Tuple[str, Tuple[Rule, ...]]]:
    if type(rel) is not InductiveRelation:
        return None
    entry = _REL_DATA.get(id(rel))
    if entry is None or entry[0]() is not rel:
        return None
    return entry[1], entry[2]


class InductiveRelation:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("InductiveRelation cannot be subclassed")

    def __init__(self, name: str, rules: Tuple[Rule, ...]) -> None:
        _require(type(name) is str and bool(name), "InductiveRelation requires a non-empty name")
        _require(
            type(rules) is tuple and bool(rules) and all(type(r) is Rule for r in rules),
            f"InductiveRelation `{name}` requires non-empty tuple[Rule, ...]",
        )
        seen: Set[str] = set()
        arity: Optional[int] = None
        for r in rules:
            _require(r.name not in seen, f"duplicate rule `{r.name}` in `{name}`")
            seen.add(r.name)
            _require(
                r.conclusion.kind == "Pred" and r.conclusion.terms[0].op == name,
                f"rule `{r.name}` conclusion must be `{name}(...)`, got `{r.conclusion}`",
            )
            r_arity = len(r.conclusion.terms[0].args)
            if arity is None:
                arity = r_arity
            else:
                _require(
                    r_arity == arity,
                    f"rule `{r.name}` arity {r_arity} does not match `{name}` arity {arity}",
                )
            for p in r.premises:
                _require(
                    _is_strictly_positive_premise(p),
                    f"non-positive predicate occurrence in premise `{p}` of `{r.name}`",
                )
                if p.kind == "Pred" and p.terms[0].op == name:
                    _require(
                        len(p.terms[0].args) == arity,
                        f"recursive premise `{p}` in `{r.name}` has wrong arity",
                    )
                else:
                    _require(
                        not _mentions_pred(p, name),
                        f"non-positive occurrence of `{name}` in premise `{p}` of `{r.name}`",
                    )
        existing = _RELATIONS.get(name)
        if existing is not None:
            _require(
                existing[1] == rules,
                f"conflicting InductiveRelation definition for `{name}`",
            )
        else:
            _RELATIONS[name] = (self, rules)
        oid = id(self)
        _REL_DATA[oid] = (weakref.ref(self), name, rules)
        weakref.finalize(self, _REL_DATA.pop, oid, None)

    @property
    def name(self) -> str:
        payload = _rel_payload(self)
        _require(payload is not None, "unregistered InductiveRelation")
        assert payload is not None
        return payload[0]

    @property
    def rules(self) -> Tuple[Rule, ...]:
        payload = _rel_payload(self)
        _require(payload is not None, "unregistered InductiveRelation")
        assert payload is not None
        return payload[1]

    @property
    def arity(self) -> int:
        return len(self.rules[0].conclusion.terms[0].args)

    def rule(self, name: str) -> Rule:
        for r in self.rules:
            if r.name == name:
                return r
        raise ProofFault(f"unknown rule `{name}` in `{self.name}`")


def _require_rel(rel: InductiveRelation, where: str) -> Tuple[str, Tuple[Rule, ...]]:
    payload = _rel_payload(rel)
    _require(payload is not None, f"{where}: expected registered InductiveRelation")
    assert payload is not None
    name, rules = payload
    entry = _RELATIONS.get(name)
    _require(entry is not None and entry[1] == rules, f"{where}: expected registered InductiveRelation")
    return name, entry[1]


def apply_rule(
    rel: InductiveRelation,
    rule_name: str,
    subst: Dict[str, Term],
    premise_thms: Sequence[Theorem],
) -> Theorem:
    """Derive a conclusion of `rel` by instantiating `rule_name` and discharging its premises."""
    rel_name, rules = _require_rel(rel, "apply_rule")
    clean = _clean_subst(subst, f"apply_rule `{rule_name}`")
    r: Optional[Rule] = None
    for cand in rules:
        if cand.name == rule_name:
            r = cand
            break
    _require(r is not None, f"unknown rule `{rule_name}` in `{rel_name}`")
    assert r is not None
    _require(
        len(premise_thms) == len(r.premises),
        f"rule `{rule_name}` expects {len(r.premises)} premises, got {len(premise_thms)}",
    )
    assumptions: Set[Formula] = set()
    for expected, given in zip(r.premises, premise_thms):
        _require_thm(given, f"apply_rule `{rule_name}`")
        inst_expected = _subst_formula(expected, clean)
        _require(
            inst_expected == given.conclusion,
            f"rule `{rule_name}` premise mismatch:\n  expected: {inst_expected}\n  got:      {given.conclusion}",
        )
        assumptions |= given.assumptions
    return _thm(assumptions, _subst_formula(r.conclusion, clean))


def rule_induction(
    rel: InductiveRelation,
    params: Tuple[str, ...],
    prop: Formula,
    cases: Dict[str, Callable[[Tuple[Theorem, ...], Tuple[Theorem, ...]], Theorem]],
) -> Theorem:
    """Verify a proof by rule induction over every rule of `rel`.

    The induction property is passed as a syntactic `(params, prop)` template
    rather than an arbitrary Python callable, so the kernel itself substitutes
    rule arguments uniformly across every rule and the universal conclusion.
    """
    rel_name, rules = _require_rel(rel, "rule_induction")
    _require_formula(prop, "rule_induction")
    params_snap = tuple(params)
    rel_arity = len(rules[0].conclusion.terms[0].args)
    _require(
        len(params_snap) == rel_arity and len(set(params_snap)) == rel_arity,
        f"rule_induction on `{rel_name}` expects {rel_arity} distinct parameter names, got {params_snap}",
    )
    for p_name in params_snap:
        _require_var_name(p_name, "rule_induction")
    rule_names = {r.name for r in rules}
    given_names = set(cases.keys())
    _require(
        rule_names == given_names,
        f"rule_induction on `{rel_name}` missing cases {rule_names - given_names} "
        f"or extra cases {given_names - rule_names}",
    )

    def _inst_prop(args: Tuple[Term, ...]) -> Formula:
        return _subst_formula(prop, dict(zip(params_snap, args)))

    avoid = free_vars_formula(prop) - set(params_snap)
    for r in rules:
        # Alpha-rename rule variables away from any free variables in `prop`
        # so schematic rule variables cannot accidentally unify with `prop`.
        rule_fvs = free_vars_formula(r.conclusion)
        for p in r.premises:
            rule_fvs |= free_vars_formula(p)
        clashes = rule_fvs & avoid
        if clashes:
            taken = avoid | rule_fvs | set(params_snap)
            rename: Dict[str, Term] = {}
            for name in sorted(clashes):
                fresh = _fresh_var(name, taken)
                taken.add(fresh)
                rename[name] = var(fresh)
            r_premises = tuple(_subst_formula(p, rename) for p in r.premises)
            r_conclusion = _subst_formula(r.conclusion, rename)
        else:
            r_premises = r.premises
            r_conclusion = r.conclusion

        prem_hyps = tuple(assume(p) for p in r_premises)
        ih_hyps_list: List[Theorem] = []
        for p in r_premises:
            if p.kind == "Pred" and p.terms[0].op == rel_name:
                ih_hyps_list.append(assume(_inst_prop(p.terms[0].args)))
        ih_hyps = tuple(ih_hyps_list)

        case_thm = cases[r.name](prem_hyps, ih_hyps)
        _require_thm(case_thm, f"rule_induction case `{r.name}`")
        expected = _inst_prop(r_conclusion.terms[0].args)
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

    generic_names: List[str] = []
    for i in range(rel_arity):
        name = f"_a{i}"
        while name in avoid or name in generic_names:
            name = f"{name}_"
        generic_names.append(name)
    generic_args = tuple(var(n) for n in generic_names)
    goal = imp(pred(rel_name, *generic_args), _inst_prop(generic_args))
    for v in reversed(generic_args):
        goal = forall(v.op, goal)
    return _thm([], goal)
