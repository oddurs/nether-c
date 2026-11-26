#!/usr/bin/env python3
"""LCF-style proof kernel for Nether C metatheory.

A `Theorem` can only be constructed by this module's inference rules using the
private `_SEAL` token. Any file in `proof/` that mentions `admit`, `sorry`, or
`unproved_axiom` is rejected by `proof/check.py` before checking begins.
"""
from __future__ import annotations
from collections.abc import Callable, Iterable, Sequence
from typing import TypeVar
import weakref

_SEAL = object()
_P = TypeVar("_P")


_BINARY_TERM_KINDS = frozenset({"=", "=_d", "!=_d", "<=", "<", "∈", "∉", "⊆", "⊈"})
_BINARY_SUB_KINDS = frozenset({"∧", "∨", "→", "↔"})
_VALID_SORTS = frozenset({"", "Depth", "Set"})

_TERM_DATA: dict[int, tuple[weakref.ref[Term], str, tuple[Term, ...], int]] = {}


def _require_payload(payload: _P | None, msg: str) -> _P:
    _require(payload is not None, msg)
    assert payload is not None
    return payload


def _term_payload(t: object) -> tuple[str, tuple[Term, ...], int] | None:
    if type(t) is not Term:
        return None
    entry = _TERM_DATA.get(id(t))
    if entry is None or entry[0]() is not t:
        return None
    return entry[1], entry[2], entry[3]


class Term:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Term cannot be subclassed")

    def __new__(cls, op: str, args: tuple[Term, ...] = ()) -> Term:
        _require(cls is Term, "Term cannot be subclassed")
        _require(type(op) is str and bool(op), f"Term op must be non-empty str, got {op!r}")
        _require(
            type(args) is tuple and all(_term_payload(a) is not None for a in args),
            f"Term args must be tuple[Term, ...], got {args!r}",
        )
        self = object.__new__(cls)
        h = hash((op, args))
        oid = id(self)
        _TERM_DATA[oid] = (weakref.ref(self), op, args, h)
        weakref.finalize(self, _TERM_DATA.pop, oid, None)
        return self

    @property
    def op(self) -> str:
        return _require_payload(_term_payload(self), "unsealed Term")[0]

    @property
    def args(self) -> tuple[Term, ...]:
        return _require_payload(_term_payload(self), "unsealed Term")[1]

    def __eq__(self, other: object) -> bool:
        if self is other:
            return True
        p1 = _term_payload(self)
        p2 = _term_payload(other)
        return p1 is not None and p2 is not None and p1[2] == p2[2] and p1[0] == p2[0] and p1[1] == p2[1]

    def __hash__(self) -> int:
        return _require_payload(_term_payload(self), "unsealed Term")[2]

    def __repr__(self) -> str:
        if not self.args:
            return self.op
        return f"{self.op}({', '.join(repr(a) for a in self.args)})"


def _require_term(t: Term, where: str) -> None:
    _require(_term_payload(t) is not None, f"{where}: expected Term, got {type(t).__name__}")


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


def _clean_subst(mapping: dict[str, Term], where: str) -> dict[str, Term]:
    _require(isinstance(mapping, dict), f"{where}: expected mapping dict")
    clean: dict[str, Term] = {}
    for k, v in dict.items(mapping):
        _require_var_name(k, where)
        _require_term(v, where)
        clean[k] = v
    return clean


def _subst_term(t: Term, mapping: dict[str, Term]) -> Term:
    _require_term(t, "subst_term")
    if not t.args and not _is_constant_name(t.op) and t.op in mapping:
        return mapping[t.op]
    return Term(t.op, tuple(_subst_term(a, mapping) for a in t.args))


def subst_term(t: Term, mapping: dict[str, Term]) -> Term:
    return _subst_term(t, _clean_subst(mapping, "subst_term"))


def free_vars_term(t: Term) -> set[str]:
    _require_term(t, "free_vars_term")
    if not t.args:
        if _is_constant_name(t.op):
            return set()
        return {t.op}
    out: set[str] = set()
    for a in t.args:
        out |= free_vars_term(a)
    return out


_FORMULA_DATA: dict[
    int,
    tuple[weakref.ref[Formula], str, tuple[Term, ...], tuple[Formula, ...], str, str, int],
] = {}


def _formula_payload(
    f: object,
) -> tuple[str, tuple[Term, ...], tuple[Formula, ...], str, str, int] | None:
    if type(f) is not Formula:
        return None
    entry = _FORMULA_DATA.get(id(f))
    if entry is None or entry[0]() is not f:
        return None
    return entry[1], entry[2], entry[3], entry[4], entry[5], entry[6]


class Formula:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Formula cannot be subclassed")

    def __new__(
        cls,
        kind: str,
        terms: tuple[Term, ...] = (),
        subs: tuple[Formula, ...] = (),
        binder: str = "",
        binder_sort: str = "",
    ) -> Formula:
        _require(cls is Formula, "Formula cannot be subclassed")
        _require(type(kind) is str, f"Formula kind must be str, got {kind!r}")
        _require(
            type(terms) is tuple and all(_term_payload(t) is not None for t in terms),
            f"Formula terms must be tuple[Term, ...], got {terms!r}",
        )
        _require(
            type(subs) is tuple and all(_formula_payload(s) is not None for s in subs),
            f"Formula subs must be tuple[Formula, ...], got {subs!r}",
        )
        _require(type(binder) is str, f"Formula binder must be str, got {binder!r}")
        _require(
            type(binder_sort) is str and binder_sort in _VALID_SORTS,
            f"Formula binder_sort must be one of {_VALID_SORTS}, got {binder_sort!r}",
        )
        if kind == "Pred":
            _require(len(terms) == 1 and not subs and binder == "" and binder_sort == "", "malformed Pred")
        elif kind in _BINARY_TERM_KINDS:
            _require(len(terms) == 2 and not subs and binder == "" and binder_sort == "", f"malformed {kind}")
        elif kind == "¬":
            _require(not terms and len(subs) == 1 and binder == "" and binder_sort == "", "malformed ¬")
        elif kind in _BINARY_SUB_KINDS:
            _require(not terms and len(subs) == 2 and binder == "" and binder_sort == "", f"malformed {kind}")
        elif kind in {"∀", "∃"}:
            _require(not terms and len(subs) == 1, "malformed quantifier")
            _require_var_name(binder, "Formula")
        else:
            raise ProofFault(f"unknown Formula kind `{kind}`")
        self = object.__new__(cls)
        h = hash((kind, terms, subs, binder, binder_sort))
        oid = id(self)
        _FORMULA_DATA[oid] = (weakref.ref(self), kind, terms, subs, binder, binder_sort, h)
        weakref.finalize(self, _FORMULA_DATA.pop, oid, None)
        return self

    @property
    def kind(self) -> str:
        return _require_payload(_formula_payload(self), "unsealed Formula")[0]

    @property
    def terms(self) -> tuple[Term, ...]:
        return _require_payload(_formula_payload(self), "unsealed Formula")[1]

    @property
    def subs(self) -> tuple[Formula, ...]:
        return _require_payload(_formula_payload(self), "unsealed Formula")[2]

    @property
    def binder(self) -> str:
        return _require_payload(_formula_payload(self), "unsealed Formula")[3]

    @property
    def binder_sort(self) -> str:
        return _require_payload(_formula_payload(self), "unsealed Formula")[4]

    def __eq__(self, other: object) -> bool:
        if self is other:
            return True
        p1 = _formula_payload(self)
        p2 = _formula_payload(other)
        return p1 is not None and p2 is not None and p1[5] == p2[5] and p1[:5] == p2[:5]

    def __hash__(self) -> int:
        return _require_payload(_formula_payload(self), "unsealed Formula")[5]

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
            if self.binder_sort:
                return f"({self.kind}{self.binder}:{self.binder_sort}. {self.subs[0]})"
            return f"({self.kind}{self.binder}. {self.subs[0]})"
        return f"{self.kind}({self.terms}, {self.subs})"


def _require_formula(f: Formula, where: str) -> None:
    _require(_formula_payload(f) is not None, f"{where}: expected Formula, got {type(f).__name__}")


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


def forall(binder: str, body: Formula, binder_sort: str = "") -> Formula:
    _require_var_name(binder, "forall")
    return Formula("∀", (), (body,), binder, binder_sort)


def free_vars_formula(f: Formula) -> set[str]:
    if f.kind in {"∀", "∃"}:
        return free_vars_formula(f.subs[0]) - {f.binder}
    if f.kind == "Pred":
        out: set[str] = set()
        for a in f.terms[0].args:
            out |= free_vars_term(a)
        return out
    out = set()
    for t in f.terms:
        out |= free_vars_term(t)
    for s in f.subs:
        out |= free_vars_formula(s)
    return out


def _fresh_var(base: str, avoid: set[str]) -> str:
    candidate = f"{base}_"
    idx = 0
    while candidate in avoid:
        idx += 1
        candidate = f"{base}_{idx}"
    return candidate


def _subst_formula(f: Formula, mapping: dict[str, Term]) -> Formula:
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
        captured: set[str] = set()
        for t in active.values():
            captured |= free_vars_term(t)
        binder = f.binder
        if binder in captured:
            avoid = body_fv | captured | set(active.keys()) | {binder}
            fresh = _fresh_var(binder, avoid)
            body = _subst_formula(body, {binder: var(fresh)})
            binder = fresh
        return Formula(f.kind, (), (_subst_formula(body, active),), binder, f.binder_sort)
    if f.kind == "Pred":
        return pred(f.terms[0].op, *(_subst_term(a, mapping) for a in f.terms[0].args))
    return Formula(
        f.kind,
        tuple(_subst_term(t, mapping) for t in f.terms),
        tuple(_subst_formula(s, mapping) for s in f.subs),
        f.binder,
        f.binder_sort,
    )


def subst_formula(f: Formula, mapping: dict[str, Term]) -> Formula:
    return _subst_formula(f, _clean_subst(mapping, "subst_formula"))


class ProofFault(Exception):
    """Raised when an invalid derivation is presented to the kernel."""


def _require(cond: bool, msg: str) -> None:
    if not cond:
        raise ProofFault(msg)


_THEOREM_DATA: dict[
    int,
    tuple[weakref.ref[Theorem], frozenset[Formula], Formula, frozenset[tuple[str, str]]],
] = {}


def _thm_payload(
    thm: object,
) -> tuple[frozenset[Formula], Formula, frozenset[tuple[str, str]]] | None:
    if type(thm) is not Theorem:
        return None
    entry = _THEOREM_DATA.get(id(thm))
    if entry is None or entry[0]() is not thm:
        return None
    return entry[1], entry[2], entry[3]


def _merge_sorts(*sort_sets: Iterable[tuple[str, str]]) -> frozenset[tuple[str, str]]:
    merged: dict[str, str] = {}
    for items in sort_sets:
        for name, sort in items:
            _require(sort in ("Depth", "Set"), f"invalid variable sort `{sort}` for `{name}`")
            prev = merged.get(name)
            _require(
                prev is None or prev == sort,
                f"conflicting sorts `{prev}` and `{sort}` for variable `{name}`",
            )
            merged[name] = sort
    return frozenset(merged.items())


# Kernel-registered uninterpreted function symbols whose codomain is fixed to
# `Depth` or `Set` (op -> (arity, result_sort)). Unregistered compound terms
# are rejected by `lattice_decide` and sort checkers so arbitrary constructors
# (`VInt`, `encode`, `f`) cannot be silently axiomatized into `{0..8}` / `P({0..8})`.
_LATTICE_FN_SORTS: dict[str, tuple[int, str]] = {
    "g": (1, "Set"),
    "g_var": (1, "Set"),
    "g_body": (1, "Set"),
}


def _check_depth_term(t: Term, d_vars: set[str], s_vars: set[str], where: str) -> None:
    if not t.args:
        _require(t.op not in SET_CONSTS, f"{where}: set constant `{t}` used in Depth position")
        if _is_ascii_digits(t.op):
            v = int(t.op)
            _require(0 <= v <= 8, f"{where}: depth literal `{t}` out of range 0..8")
        else:
            _require(
                t.op not in RESERVED_CONSTANTS and not t.op.isdigit(),
                f"{where}: non-depth constant `{t}` used in Depth position",
            )
            d_vars.add(t.op)
    elif t.op == "max":
        _require(len(t.args) >= 1, f"{where}: max requires at least one argument")
        for a in t.args:
            _check_depth_term(a, d_vars, s_vars, where)
    elif t.op == "max_set":
        _require(len(t.args) == 1, f"{where}: max_set expects 1 argument, got `{t}`")
        _check_set_term(t.args[0], d_vars, s_vars, where)
    elif t.op in _LATTICE_FN_SORTS and _LATTICE_FN_SORTS[t.op] == (len(t.args), "Depth"):
        pass
    else:
        raise ProofFault(f"{where}: non-depth term `{t}` cannot instantiate Depth quantifier")


def _check_set_term(t: Term, d_vars: set[str], s_vars: set[str], where: str) -> None:
    if not t.args:
        _require(
            not _is_ascii_digits(t.op) and not t.op.isdigit(),
            f"{where}: depth literal `{t}` used in Set position",
        )
        if t.op not in SET_CONSTS:
            _require(
                t.op not in RESERVED_CONSTANTS,
                f"{where}: non-set constant `{t}` used in Set position",
            )
            s_vars.add(t.op)
    elif t.op == "singleton":
        _require(len(t.args) == 1, f"{where}: singleton expects 1 argument, got `{t}`")
        _check_depth_term(t.args[0], d_vars, s_vars, where)
    elif t.op == "∪":
        _require(len(t.args) >= 1, f"{where}: ∪ requires at least one argument")
        for a in t.args:
            _check_set_term(a, d_vars, s_vars, where)
    elif t.op in _LATTICE_FN_SORTS and _LATTICE_FN_SORTS[t.op] == (len(t.args), "Set"):
        pass
    else:
        raise ProofFault(f"{where}: non-set term `{t}` cannot instantiate Set quantifier")


class Theorem:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Theorem cannot be subclassed")

    def __new__(
        cls,
        _token: object,
        assumptions: frozenset[Formula],
        conclusion: Formula,
        sorts: frozenset[tuple[str, str]] = frozenset(),
    ) -> Theorem:
        _require(cls is Theorem, "Theorem cannot be subclassed")
        _require(_token is _SEAL, "Theorem constructed outside kernel")
        _require(
            type(assumptions) is frozenset and all(_formula_payload(a) is not None for a in assumptions),
            "Theorem assumptions must be frozenset[Formula]",
        )
        _require_formula(conclusion, "Theorem")
        _require(type(sorts) is frozenset, "Theorem sorts must be frozenset")
        self = object.__new__(cls)
        oid = id(self)
        _THEOREM_DATA[oid] = (weakref.ref(self), assumptions, conclusion, sorts)
        weakref.finalize(self, _THEOREM_DATA.pop, oid, None)
        return self

    @property
    def assumptions(self) -> frozenset[Formula]:
        return _require_payload(_thm_payload(self), "unsealed Theorem")[0]

    @property
    def conclusion(self) -> Formula:
        return _require_payload(_thm_payload(self), "unsealed Theorem")[1]

    def __eq__(self, other: object) -> bool:
        p1 = _thm_payload(self)
        p2 = _thm_payload(other)
        return p1 is not None and p2 is not None and p1 == p2

    def __hash__(self) -> int:
        return hash(_require_payload(_thm_payload(self), "unsealed Theorem"))

    def __repr__(self) -> str:
        hyps = ", ".join(repr(a) for a in sorted(self.assumptions, key=repr))
        return f"{hyps} ⊢ {self.conclusion}"


def _require_thm(
    thm: Theorem, where: str
) -> tuple[frozenset[Formula], Formula, frozenset[tuple[str, str]]]:
    return _require_payload(_thm_payload(thm), f"{where}: expected kernel Theorem")


def _thm(
    assumptions: Iterable[Formula],
    conclusion: Formula,
    sorts: Iterable[tuple[str, str]] = (),
) -> Theorem:
    return Theorem(_SEAL, frozenset(assumptions), conclusion, _merge_sorts(sorts))


# ── Primitive logical inference rules ────────────────────────────────────────


def assume(f: Formula) -> Theorem:
    _require_formula(f, "assume")
    return _thm([f], f)


def refl(t: Term) -> Theorem:
    _require_term(t, "refl")
    return _thm([], eq(t, t))


def sym(thm: Theorem) -> Theorem:
    assumptions, conclusion, sorts = _require_thm(thm, "sym")
    _require(conclusion.kind == "=", f"sym expects equality, got {conclusion}")
    a, b = conclusion.terms
    return _thm(assumptions, eq(b, a), sorts)


def trans(t1: Theorem, t2: Theorem) -> Theorem:
    a1, c1, s1 = _require_thm(t1, "trans")
    a2, c2, s2 = _require_thm(t2, "trans")
    _require(c1.kind == "=" and c2.kind == "=", "trans expects =")
    _require(c1.terms[1] == c2.terms[0], "trans middle mismatch")
    return _thm(a1 | a2, eq(c1.terms[0], c2.terms[1]), _merge_sorts(s1, s2))


def cong(op: str, eq_thms: Sequence[Theorem]) -> Theorem:
    _require(type(op) is str and bool(op), f"cong expects non-empty op, got {op!r}")
    assumptions: set[Formula] = set()
    all_sorts: list[frozenset[tuple[str, str]]] = []
    lhs_args: list[Term] = []
    rhs_args: list[Term] = []
    for t in eq_thms:
        ta, tc, ts = _require_thm(t, "cong")
        _require(tc.kind == "=", f"cong expects =, got {tc}")
        assumptions |= ta
        all_sorts.append(ts)
        lhs_args.append(tc.terms[0])
        rhs_args.append(tc.terms[1])
    return _thm(assumptions, eq(Term(op, tuple(lhs_args)), Term(op, tuple(rhs_args))), _merge_sorts(*all_sorts))


def mp(imp_thm: Theorem, ant_thm: Theorem) -> Theorem:
    ia, ic, isorts = _require_thm(imp_thm, "mp")
    aa, ac, asorts = _require_thm(ant_thm, "mp")
    _require(
        ic.kind == "→",
        f"mp expects implication, got {ic}",
    )
    lhs, rhs = ic.subs
    _require(
        lhs == ac,
        f"mp antecedent mismatch:\n  expected: {lhs}\n  got:      {ac}",
    )
    return _thm(ia | aa, rhs, _merge_sorts(isorts, asorts))


def discharge(hyp: Formula, thm: Theorem) -> Theorem:
    _require_formula(hyp, "discharge")
    assumptions, conclusion, sorts = _require_thm(thm, "discharge")
    return _thm(assumptions - {hyp}, imp(hyp, conclusion), sorts)


def conj_intro(t1: Theorem, t2: Theorem) -> Theorem:
    a1, c1, s1 = _require_thm(t1, "conj_intro")
    a2, c2, s2 = _require_thm(t2, "conj_intro")
    return _thm(a1 | a2, conj(c1, c2), _merge_sorts(s1, s2))


def conj_elim_l(thm: Theorem) -> Theorem:
    assumptions, conclusion, sorts = _require_thm(thm, "conj_elim_l")
    _require(conclusion.kind == "∧", f"conj_elim_l expects ∧, got {conclusion}")
    return _thm(assumptions, conclusion.subs[0], sorts)


def conj_elim_r(thm: Theorem) -> Theorem:
    assumptions, conclusion, sorts = _require_thm(thm, "conj_elim_r")
    _require(conclusion.kind == "∧", f"conj_elim_r expects ∧, got {conclusion}")
    return _thm(assumptions, conclusion.subs[1], sorts)


def iff_intro(ab: Theorem, ba: Theorem) -> Theorem:
    a1, c1, s1 = _require_thm(ab, "iff_intro")
    a2, c2, s2 = _require_thm(ba, "iff_intro")
    _require(c1.kind == "→" and c2.kind == "→", "iff_intro expects →")
    a, b = c1.subs
    b2, a2_f = c2.subs
    _require(a == a2_f and b == b2, "iff_intro mismatch")
    return _thm(a1 | a2, iff(a, b), _merge_sorts(s1, s2))


def inst(thm: Theorem, term: Term) -> Theorem:
    assumptions, conclusion, sorts = _require_thm(thm, "inst")
    _require_term(term, "inst")
    _require(conclusion.kind == "∀", f"inst expects ∀, got {conclusion}")
    extra: list[tuple[str, str]] = []
    if conclusion.binder_sort == "Depth":
        dv: set[str] = set()
        sv: set[str] = set()
        _check_depth_term(term, dv, sv, "inst")
        extra.extend((v, "Depth") for v in dv)
        extra.extend((v, "Set") for v in sv)
    elif conclusion.binder_sort == "Set":
        dv = set()
        sv = set()
        _check_set_term(term, dv, sv, "inst")
        extra.extend((v, "Depth") for v in dv)
        extra.extend((v, "Set") for v in sv)
    new_sorts = _merge_sorts(sorts, extra)
    body = subst_formula(conclusion.subs[0], {conclusion.binder: term})
    return _thm(assumptions, body, new_sorts)


def inst_all(thm: Theorem, terms: Sequence[Term]) -> Theorem:
    out = thm
    for t in terms:
        out = inst(out, t)
    return out


def gen(binder: str, thm: Theorem) -> Theorem:
    _require_var_name(binder, "gen")
    assumptions, conclusion, sorts = _require_thm(thm, "gen")
    for a in assumptions:
        _require(
            binder not in free_vars_formula(a),
            f"cannot generalize `{binder}` while free in assumption {a}",
        )
    sorts_map = dict(sorts)
    binder_sort = sorts_map.pop(binder, "")
    return _thm(assumptions, forall(binder, conclusion, binder_sort), sorts_map.items())


def gen_all(binders: Sequence[str], thm: Theorem) -> Theorem:
    out = thm
    for b in binders:
        out = gen(b, out)
    return out


def rewrite(eq_thm: Theorem, target: Theorem) -> Theorem:
    ea, ec, esorts = _require_thm(eq_thm, "rewrite")
    ta, tc, tsorts = _require_thm(target, "rewrite")
    _require(ec.kind == "=", f"rewrite expects =, got {ec}")
    src, dst = ec.terms
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
            return Formula(f.kind, (), (replace_f(body),), binder, f.binder_sort)
        if f.kind == "Pred":
            return pred(f.terms[0].op, *(replace_t(a) for a in f.terms[0].args))
        return Formula(
            f.kind,
            tuple(replace_t(t) for t in f.terms),
            tuple(replace_f(s) for s in f.subs),
            f.binder,
            f.binder_sort,
        )

    return _thm(ea | ta, replace_f(tc), _merge_sorts(esorts, tsorts))


# ── Bounded lattice & capability-set decision procedure ──────────────────────
#
# Depth = {0..8} with order <= and join `max`.
# Held  = subsets of {0..8} with 0 always present (§2.1: "Stratum 0 is in δ
#         always"), union `∪`, singleton `{d}`, membership `∈`, subset `⊆`, and
#         upper bound `max_set(δ)`.

SET_OPS = frozenset({"singleton", "∪"})
SET_CONSTS = frozenset({"∅", "∅_raw"})
DEPTH_OPS = frozenset({"max", "max_set"})


def _collect_lattice_vars(
    f: Formula,
    d_vars: set[Term],
    s_vars: set[Term],
    full_s: set[Term],
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
            sig = _LATTICE_FN_SORTS.get(t.op)
            _require(
                sig is not None and sig == (len(t.args), "Depth"),
                f"unregistered non-depth compound term `{t}` in depth position",
            )
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
            sig = _LATTICE_FN_SORTS.get(t.op)
            _require(
                sig is not None and sig == (len(t.args), "Set"),
                f"unregistered non-set compound term `{t}` in set position",
            )
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


def _compile_lattice(
    d_idx: dict[Term, int],
    s_idx: dict[Term, int],
    d_vals: list[int],
    s_vals: list[int],
) -> tuple[Callable[[Term], Callable[[], int]], Callable[[Formula], Callable[[], bool]]]:
    """Compile lattice terms and formulas into flat-array closures over `d_vals`/`s_vals`."""

    def cd(t: Term) -> Callable[[], int]:
        if not t.args:
            if _is_ascii_digits(t.op):
                v = int(t.op)
                _require(0 <= v <= 8, f"depth literal {v} out of range 0..8")
                return lambda v=v: v
            i = d_idx[t]
            return lambda i=i: d_vals[i]
        if t.op == "max":
            fns = tuple(cd(a) for a in t.args)
            if len(fns) == 2:
                f0, f1 = fns

                def _max2(f0=f0, f1=f1) -> int:
                    a = f0()
                    b = f1()
                    return a if a >= b else b

                return _max2
            return lambda fns=fns: max(fn() for fn in fns)
        if t.op == "max_set":
            fs = cs(t.args[0])

            def _max_set(fs=fs) -> int:
                m = fs()
                return m.bit_length() - 1 if m else 0

            return _max_set
        i = d_idx[t]
        return lambda i=i: d_vals[i]

    def cs(t: Term) -> Callable[[], int]:
        if not t.args:
            if t.op == "∅":
                return lambda: 1
            if t.op == "∅_raw":
                return lambda: 0
            i = s_idx[t]
            return lambda i=i: s_vals[i]
        if t.op == "singleton":
            fd = cd(t.args[0])
            return lambda fd=fd: 1 << fd()
        if t.op == "∪":
            fns = tuple(cs(a) for a in t.args)
            acc = fns[0]
            for fn in fns[1:]:
                acc = lambda a=acc, b=fn: a() | b()
            return acc
        i = s_idx[t]
        return lambda i=i: s_vals[i]

    def cf(f: Formula) -> Callable[[], bool]:
        k = f.kind
        if k == "<=":
            a, b = cd(f.terms[0]), cd(f.terms[1])
            return lambda a=a, b=b: a() <= b()
        if k == "<":
            a, b = cd(f.terms[0]), cd(f.terms[1])
            return lambda a=a, b=b: a() < b()
        if k == "=_d":
            a, b = cd(f.terms[0]), cd(f.terms[1])
            return lambda a=a, b=b: a() == b()
        if k == "!=_d":
            a, b = cd(f.terms[0]), cd(f.terms[1])
            return lambda a=a, b=b: a() != b()
        if k == "∈":
            a, b = cd(f.terms[0]), cs(f.terms[1])
            return lambda a=a, b=b: bool(b() & (1 << a()))
        if k == "∉":
            a, b = cd(f.terms[0]), cs(f.terms[1])
            return lambda a=a, b=b: not bool(b() & (1 << a()))
        if k == "⊆":
            a, b = cs(f.terms[0]), cs(f.terms[1])
            return lambda a=a, b=b: (a() & ~b()) == 0
        if k == "⊈":
            a, b = cs(f.terms[0]), cs(f.terms[1])
            return lambda a=a, b=b: (a() & ~b()) != 0
        if k == "¬":
            s0 = cf(f.subs[0])
            return lambda s0=s0: not s0()
        if k == "∧":
            s0, s1 = cf(f.subs[0]), cf(f.subs[1])
            return lambda s0=s0, s1=s1: s0() and s1()
        if k == "∨":
            s0, s1 = cf(f.subs[0]), cf(f.subs[1])
            return lambda s0=s0, s1=s1: s0() or s1()
        if k == "→":
            s0, s1 = cf(f.subs[0]), cf(f.subs[1])
            return lambda s0=s0, s1=s1: (not s0()) or s1()
        if k == "↔":
            s0, s1 = cf(f.subs[0]), cf(f.subs[1])
            return lambda s0=s0, s1=s1: s0() == s1()
        raise ProofFault(f"non-lattice formula `{f}` in lattice_decide")

    return cd, cf


def _split_conj(f: Formula) -> list[Formula]:
    if f.kind == "∧":
        return _split_conj(f.subs[0]) + _split_conj(f.subs[1])
    return [f]


def _has_set_rel(f: Formula) -> bool:
    if f.kind in {"∈", "∉", "⊆", "⊈"}:
        return True
    return any(_has_set_rel(s) for s in f.subs)


def _has_non_horn(f: Formula) -> bool:
    def _has_nonzero(t: Term) -> bool:
        if not t.args and _is_ascii_digits(t.op) and int(t.op) > 0:
            return True
        return any(_has_nonzero(a) for a in t.args)

    if f.kind in {"∈", "∉", "⊆", "⊈", "<", "=_d", "!=_d", "¬", "∨", "↔", "→"}:
        return True
    return any(_has_nonzero(t) for t in f.terms) or any(_has_non_horn(s) for s in f.subs)


def _scan_bit_projection(formulas: Sequence[Formula]) -> tuple[bool, bool, list[Term], int]:
    """Scan lattice formulas for exact bit-projection eligibility.

    When `max_set` does not appear anywhere, every set operation (`∅`, `∅_raw`,
    `singleton`, `∪`) is purely bitwise and every set relation (`∈`, `∉`, `⊆`,
    `⊈`) depends only on the pinned bit positions (bit `0` if `∅` appears, plus
    each depth term inside `∈`/`∉`/`singleton`) and at most one witness bit per
    `⊆`/`⊈` atom from the remaining symmetric bits.
    """
    has_max_set = False
    uses_empty = False
    d_terms: list[Term] = []
    subset_atoms = 0

    def scan_d(t: Term) -> None:
        nonlocal has_max_set
        if not t.args:
            return
        if t.op == "max_set":
            has_max_set = True
            scan_s(t.args[0])
        elif t.op == "max":
            for a in t.args:
                scan_d(a)

    def scan_s(t: Term) -> None:
        nonlocal uses_empty
        if not t.args:
            if t.op == "∅":
                uses_empty = True
            return
        if t.op == "singleton":
            d_terms.append(t.args[0])
            scan_d(t.args[0])
        elif t.op == "∪":
            for a in t.args:
                scan_s(a)

    def scan_f(f: Formula) -> None:
        nonlocal subset_atoms
        if f.kind in {"<=", "<", "=_d", "!=_d"}:
            scan_d(f.terms[0])
            scan_d(f.terms[1])
        elif f.kind in {"∈", "∉"}:
            d_terms.append(f.terms[0])
            scan_d(f.terms[0])
            scan_s(f.terms[1])
        elif f.kind in {"⊆", "⊈"}:
            subset_atoms += 1
            scan_s(f.terms[0])
            scan_s(f.terms[1])
        elif f.kind in {"¬", "∧", "∨", "→", "↔"}:
            for s in f.subs:
                scan_f(s)

    for form in formulas:
        scan_f(form)
    return has_max_set, uses_empty, d_terms, subset_atoms


def _weaken_antecedents(
    raw_antecedents: Sequence[Formula],
    check_goal: Formula,
) -> list[Formula]:
    """Soundly weaken positive `A ⊆ B` / `d ∈ S` antecedents when `check_goal` has no set relations."""
    if _has_set_rel(check_goal):
        return list(raw_antecedents)
    all_pos_atoms = all(a.kind in {"<=", "<", "=_d", "!=_d", "⊆", "∈"} for a in raw_antecedents)
    out: list[Formula] = []
    for ant in raw_antecedents:
        if ant.kind == "⊆":
            out.append(le(app("max_set", ant.terms[0]), app("max_set", ant.terms[1])))
        elif all_pos_atoms and ant.kind == "∈" and ant.terms[1] != Term("∅_raw", ()):
            out.append(le(ant.terms[0], app("max_set", ant.terms[1])))
        else:
            out.append(ant)
    return out


def _schedule_antecedents(
    ant_pairs: Sequence[tuple[Formula, set[Term], set[Term]]],
    outer_list: Sequence[Term],
    inner_list: Sequence[Term],
    compile_f: Callable[[Formula], Callable[[], bool]],
) -> tuple[
    list[tuple[Callable[[], bool], ...]],
    list[tuple[Callable[[], bool], ...]],
    tuple[Callable[[], bool], ...],
]:
    """Attach each antecedent to the earliest variable index where all its atoms are bound."""
    placed: set[int] = set()
    at_outer: list[list[Formula]] = [[] for _ in outer_list]
    for i in range(len(outer_list)):
        bound = set(outer_list[: i + 1])
        for a_idx, (ant, ov, iv) in enumerate(ant_pairs):
            if a_idx not in placed and not iv and ov <= bound:
                at_outer[i].append(ant)
                placed.add(a_idx)
    at_inner: list[list[Formula]] = [[] for _ in inner_list]
    for i in range(len(inner_list)):
        bound = set(inner_list[: i + 1])
        for a_idx, (ant, _ov, iv) in enumerate(ant_pairs):
            if a_idx not in placed and iv <= bound:
                at_inner[i].append(ant)
                placed.add(a_idx)
    before_inner = tuple(
        compile_f(ant) for a_idx, (ant, _ov, _iv) in enumerate(ant_pairs) if a_idx not in placed
    )
    return (
        [tuple(compile_f(c) for c in cs) for cs in at_outer],
        [tuple(compile_f(c) for c in cs) for cs in at_inner],
        before_inner,
    )


def _search_axis(
    vals: list[int],
    doms: Sequence[Sequence[int]],
    checks_at: Sequence[tuple[Callable[[], bool], ...]],
    on_leaf: Callable[[], None],
) -> None:
    """Exhaustively assign `vals[0..n)` from `doms`, pruning at each index with `checks_at`."""
    limit = len(vals)

    def step(idx: int) -> None:
        if idx == limit:
            on_leaf()
            return
        checks = checks_at[idx]
        if not checks:
            for val in doms[idx]:
                vals[idx] = val
                step(idx + 1)
        else:
            for val in doms[idx]:
                vals[idx] = val
                if all(c() for c in checks):
                    step(idx + 1)

    step(0)


def _term_key(t: Term) -> tuple[str, int, str]:
    return (t.op, len(t.args), repr(t))


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
    assumptions: set[Formula] = set()
    hyp_sorts: list[frozenset[tuple[str, str]]] = []
    raw_antecedents: list[Formula] = []
    for h in hyps:
        ha, hc, hs = _require_thm(h, "lattice_decide")
        assumptions |= ha
        hyp_sorts.append(hs)
        raw_antecedents.extend(_split_conj(hc))

    # Unpack top-level implications in `goal` for earlier antecedent pruning.
    check_goal = goal
    while check_goal.kind == "→":
        raw_antecedents.extend(_split_conj(check_goal.subs[0]))
        check_goal = check_goal.subs[1]

    # Validate original formula sorts before any sound weakening.
    orig_d: set[Term] = set()
    orig_s: set[Term] = set()
    orig_full: set[Term] = set()
    for ant in raw_antecedents:
        _collect_lattice_vars(ant, orig_d, orig_s, orig_full)
    _collect_lattice_vars(check_goal, orig_d, orig_s, orig_full)
    _require(orig_d.isdisjoint(orig_s), f"variable used at both depth and set sort: {orig_d & orig_s}")

    antecedents = _weaken_antecedents(raw_antecedents, check_goal)

    d_vars: set[Term] = set()
    s_vars: set[Term] = set()
    full_s: set[Term] = set()
    for ant in antecedents:
        _collect_lattice_vars(ant, d_vars, s_vars, full_s)
    _collect_lattice_vars(check_goal, d_vars, s_vars, full_s)

    # Full-domain checking over `{0..8}` for all depths and exact bitmasks for
    # every set variable in `full_s`. Only flat Horn clauses (`∧ atoms → atom`)
    # over positive order with `full_s == empty` and no nonzero constants use
    # the bounded-chain homomorphism (`{0, 1, 2}`).
    non_horn = any(_has_non_horn(a) for a in antecedents) or _has_non_horn(check_goal)
    chain_len = 9 if (full_s or non_horn) else 3

    # Build variable order so antecedents prune as early as possible.
    ant_vars: list[tuple[Formula, set[Term], set[Term]]] = []
    for ant in antecedents:
        dv: set[Term] = set()
        sv: set[Term] = set()
        _collect_lattice_vars(ant, dv, sv, set())
        ant_vars.append((ant, dv, sv))

    # Exact bit-projection for `max_set`-free formulas: once depth variables are
    # fixed, only the pinned bit positions and at most one symmetric witness bit
    # per `⊆`/`⊈` atom can affect truth values. Any counterexample over 2^{0..8}
    # projects to a counterexample over the active bit subset, and vice versa.
    has_max_set, uses_empty, d_terms, subset_atoms = _scan_bit_projection([*antecedents, check_goal])
    use_bit_proj = bool(full_s and not has_max_set)

    # If antecedents constrain set-only pairs (e.g. `δ_A ⊆ δ_B`), order sets first
    # unless bit-projection needs depth variables bound first.
    s_only_ants = [item for item in ant_vars if not item[1] and item[2]]
    sets_first = bool(s_only_ants and full_s and not use_bit_proj)

    d_list: list[Term] = []
    for _ant, dv, sv in sorted(ant_vars, key=lambda item: (len(item[2]), len(item[1]))):
        for v in sorted(dv, key=_term_key):
            if v not in d_list:
                d_list.append(v)
    for v in sorted(d_vars, key=_term_key):
        if v not in d_list:
            d_list.append(v)

    s_list: list[Term] = []
    for _ant, _dv, sv in sorted(ant_vars, key=lambda item: (len(item[1]), len(item[2]))):
        for v in sorted(sv, key=_term_key):
            if v not in s_list:
                s_list.append(v)
    for v in sorted(s_vars, key=_term_key):
        if v not in s_list:
            s_list.append(v)

    # A set variable observed solely through `max_set` and `∪` is completely
    # determined by its maximum bit (`max_set(A ∪ B) = max(max_set(A), max_set(B))`).
    max_set_dom = [0] + [1 << m for m in range(1, chain_len)]
    full_dom = list(range(512))
    s_doms = [full_dom if v in full_s else max_set_dom for v in s_list]
    d_doms = [range(chain_len)] * len(d_list)

    d_idx = {v: i for i, v in enumerate(d_list)}
    s_idx = {v: i for i, v in enumerate(s_list)}
    d_vals = [0] * len(d_list)
    s_vals = [0] * len(s_list)
    compile_d, compile_f = _compile_lattice(d_idx, s_idx, d_vals, s_vals)
    goal_fn = compile_f(check_goal)

    def _check_goal() -> None:
        if not goal_fn():
            env_d = {v: d_vals[i] for i, v in enumerate(d_list)}
            env_s = {v: s_vals[i] for i, v in enumerate(s_list)}
            raise ProofFault(
                f"lattice_decide counterexample for `{goal}`:\n"
                f"  depths: {env_d}\n  sets:   {env_s}"
            )

    if sets_first:
        at_s, at_d, before_d = _schedule_antecedents(
            [(ant, sv, dv) for ant, dv, sv in ant_vars], s_list, d_list, compile_f
        )

        def _enter_d() -> None:
            if all(c() for c in before_d):
                _search_axis(d_vals, d_doms, at_d, _check_goal)

        _search_axis(s_vals, s_doms, at_s, _enter_d)
    else:
        at_d, at_s, before_s = _schedule_antecedents(ant_vars, d_list, s_list, compile_f)
        d_term_fns = tuple(compile_d(dt) for dt in d_terms) if use_bit_proj else ()

        def _enter_s() -> None:
            if not all(c() for c in before_s):
                return
            if use_bit_proj:
                pinned: set[int] = {0} if uses_empty else set()
                for dt_fn in d_term_fns:
                    pinned.add(dt_fn())
                free_bits = [b for b in range(9) if b not in pinned]
                active_bits = sorted(pinned) + free_bits[:subset_atoms]
                proj_masks = [0]
                for bit in active_bits:
                    m = 1 << bit
                    proj_masks.extend([x | m for x in proj_masks])
                _search_axis(s_vals, [proj_masks] * len(s_list), at_s, _check_goal)
            else:
                _search_axis(s_vals, s_doms, at_s, _check_goal)

        _search_axis(d_vals, d_doms, at_d, _enter_s)

    out_sorts = _merge_sorts(
        *hyp_sorts,
        [(t.op, "Depth") for t in orig_d if not t.args],
        [(t.op, "Set") for t in orig_s if not t.args],
    )
    return _thm(assumptions, goal, out_sorts)


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


def _infer_rule_var_sorts(
    premises: tuple[Formula, ...],
    conclusion: Formula,
    explicit: dict[str, str] | None = None,
) -> frozenset[tuple[str, str]]:
    """Infer Depth/Set sorts for schematic variables appearing in lattice
    operations or lattice relations in a `Rule`, merged with any explicit sorts."""
    d_vars: set[str] = set()
    s_vars: set[str] = set()

    def scan_term_any(t: Term) -> None:
        if t.op == "max":
            for a in t.args:
                _check_depth_term(a, d_vars, s_vars, "Rule")
        elif t.op == "max_set" and len(t.args) == 1:
            _check_set_term(t.args[0], d_vars, s_vars, "Rule")
        elif t.op == "singleton" and len(t.args) == 1:
            _check_depth_term(t.args[0], d_vars, s_vars, "Rule")
        elif t.op == "∪":
            for a in t.args:
                _check_set_term(a, d_vars, s_vars, "Rule")
        else:
            for a in t.args:
                scan_term_any(a)

    def scan_formula(f: Formula) -> None:
        if f.kind in {"<=", "<", "=_d", "!=_d"}:
            _check_depth_term(f.terms[0], d_vars, s_vars, "Rule")
            _check_depth_term(f.terms[1], d_vars, s_vars, "Rule")
        elif f.kind in {"∈", "∉"}:
            _check_depth_term(f.terms[0], d_vars, s_vars, "Rule")
            _check_set_term(f.terms[1], d_vars, s_vars, "Rule")
        elif f.kind in {"⊆", "⊈"}:
            _check_set_term(f.terms[0], d_vars, s_vars, "Rule")
            _check_set_term(f.terms[1], d_vars, s_vars, "Rule")
        else:
            for t in f.terms:
                scan_term_any(t)
            for s in f.subs:
                scan_formula(s)

    for p in premises:
        scan_formula(p)
    scan_formula(conclusion)
    extra: list[tuple[str, str]] = []
    if explicit is not None:
        _require(isinstance(explicit, dict), "Rule var_sorts must be a dict")
        for k, v in dict.items(explicit):
            _require_var_name(k, "Rule var_sorts")
            _require(v in ("Depth", "Set"), f"Rule var_sorts: invalid sort `{v}` for `{k}`")
            extra.append((k, v))
    return _merge_sorts([(v, "Depth") for v in d_vars], [(v, "Set") for v in s_vars], extra)


_RULE_DATA: dict[
    int,
    tuple[weakref.ref[Rule], str, tuple[Formula, ...], Formula, frozenset[tuple[str, str]], int],
] = {}


def _rule_payload(
    r: object,
) -> tuple[str, tuple[Formula, ...], Formula, frozenset[tuple[str, str]], int] | None:
    if type(r) is not Rule:
        return None
    entry = _RULE_DATA.get(id(r))
    if entry is None or entry[0]() is not r:
        return None
    return entry[1], entry[2], entry[3], entry[4], entry[5]


class Rule:
    __slots__ = ("__weakref__",)

    def __init_subclass__(cls, **kwargs: object) -> None:
        raise ProofFault("Rule cannot be subclassed")

    def __new__(
        cls,
        name: str,
        premises: tuple[Formula, ...],
        conclusion: Formula,
        var_sorts: dict[str, str] | None = None,
    ) -> Rule:
        _require(cls is Rule, "Rule cannot be subclassed")
        _require(type(name) is str and bool(name), "Rule requires a non-empty name")
        _require(
            type(premises) is tuple and all(_formula_payload(p) is not None for p in premises),
            f"Rule `{name}` premises must be tuple[Formula, ...]",
        )
        _require_formula(conclusion, f"Rule `{name}`")
        sorts = _infer_rule_var_sorts(premises, conclusion, var_sorts)
        self = object.__new__(cls)
        h = hash((name, premises, conclusion, sorts))
        oid = id(self)
        _RULE_DATA[oid] = (weakref.ref(self), name, premises, conclusion, sorts, h)
        weakref.finalize(self, _RULE_DATA.pop, oid, None)
        return self

    @property
    def name(self) -> str:
        return _require_payload(_rule_payload(self), "unsealed Rule")[0]

    @property
    def premises(self) -> tuple[Formula, ...]:
        return _require_payload(_rule_payload(self), "unsealed Rule")[1]

    @property
    def conclusion(self) -> Formula:
        return _require_payload(_rule_payload(self), "unsealed Rule")[2]

    @property
    def var_sorts(self) -> frozenset[tuple[str, str]]:
        return _require_payload(_rule_payload(self), "unsealed Rule")[3]

    def __eq__(self, other: object) -> bool:
        if self is other:
            return True
        p1 = _rule_payload(self)
        p2 = _rule_payload(other)
        return p1 is not None and p2 is not None and p1[4] == p2[4] and p1[:4] == p2[:4]

    def __hash__(self) -> int:
        return _require_payload(_rule_payload(self), "unsealed Rule")[4]

    def __repr__(self) -> str:
        return f"Rule({self.name!r}, {self.premises!r}, {self.conclusion!r})"


_RELATIONS: dict[str, tuple[InductiveRelation, tuple[Rule, ...]]] = {}
_REL_DATA: dict[int, tuple[weakref.ref[InductiveRelation], str, tuple[Rule, ...]]] = {}


def _rel_payload(rel: object) -> tuple[str, tuple[Rule, ...]] | None:
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

    def __new__(cls, name: str, rules: tuple[Rule, ...]) -> InductiveRelation:
        _require(cls is InductiveRelation, "InductiveRelation cannot be subclassed")
        _require(type(name) is str and bool(name), "InductiveRelation requires a non-empty name")
        _require(
            type(rules) is tuple and bool(rules) and all(_rule_payload(r) is not None for r in rules),
            f"InductiveRelation `{name}` requires non-empty tuple[Rule, ...]",
        )
        seen: set[str] = set()
        arity: int | None = None
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
        self = object.__new__(cls)
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
        return self

    @property
    def name(self) -> str:
        return _require_payload(_rel_payload(self), "unregistered InductiveRelation")[0]

    @property
    def rules(self) -> tuple[Rule, ...]:
        return _require_payload(_rel_payload(self), "unregistered InductiveRelation")[1]

    @property
    def arity(self) -> int:
        return len(self.rules[0].conclusion.terms[0].args)

    def rule(self, name: str) -> Rule:
        for r in self.rules:
            if r.name == name:
                return r
        raise ProofFault(f"unknown rule `{name}` in `{self.name}`")


def _require_rel(rel: InductiveRelation, where: str) -> tuple[str, tuple[Rule, ...]]:
    name, rules = _require_payload(_rel_payload(rel), f"{where}: expected registered InductiveRelation")
    entry = _RELATIONS.get(name)
    _require(entry is not None and entry[1] == rules, f"{where}: expected registered InductiveRelation")
    assert entry is not None
    return name, entry[1]


def apply_rule(
    rel: InductiveRelation,
    rule_name: str,
    subst: dict[str, Term],
    premise_thms: Sequence[Theorem],
) -> Theorem:
    """Derive a conclusion of `rel` by instantiating `rule_name` and discharging its premises."""
    rel_name, rules = _require_rel(rel, "apply_rule")
    clean = _clean_subst(subst, f"apply_rule `{rule_name}`")
    r: Rule | None = None
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
    assumptions: set[Formula] = set()
    all_sorts: list[frozenset[tuple[str, str]]] = []
    # Enforce declared/inferred Depth and Set sorts on every schematic variable
    # instantiated by `subst` so unsorted terms cannot enter a sorted rule.
    inst_d: set[str] = set()
    inst_s: set[str] = set()
    for vname, vsort in r.var_sorts:
        term = clean.get(vname, var(vname))
        if vsort == "Depth":
            _check_depth_term(term, inst_d, inst_s, f"apply_rule `{rule_name}`")
        else:
            _check_set_term(term, inst_d, inst_s, f"apply_rule `{rule_name}`")
    if inst_d or inst_s:
        all_sorts.append(_merge_sorts([(v, "Depth") for v in inst_d], [(v, "Set") for v in inst_s]))
    for expected, given in zip(r.premises, premise_thms):
        ga, gc, gs = _require_thm(given, f"apply_rule `{rule_name}`")
        inst_expected = _subst_formula(expected, clean)
        _require(
            inst_expected == gc,
            f"rule `{rule_name}` premise mismatch:\n  expected: {inst_expected}\n  got:      {gc}",
        )
        assumptions |= ga
        all_sorts.append(gs)
    return _thm(assumptions, _subst_formula(r.conclusion, clean), _merge_sorts(*all_sorts))


def _alpha_rename_rule(
    r: Rule,
    avoid: set[str],
    params: tuple[str, ...],
) -> tuple[tuple[Formula, ...], Formula, dict[str, str]]:
    """Alpha-rename rule variables away from any free variables in `prop`
    so schematic rule variables cannot accidentally unify with `prop`."""
    rule_fvs = free_vars_formula(r.conclusion)
    for p in r.premises:
        rule_fvs |= free_vars_formula(p)
    clashes = rule_fvs & avoid
    r_var_sorts = dict(r.var_sorts)
    if not clashes:
        return r.premises, r.conclusion, r_var_sorts
    taken = avoid | rule_fvs | set(params)
    rename: dict[str, Term] = {}
    for name in sorted(clashes):
        fresh = _fresh_var(name, taken)
        taken.add(fresh)
        rename[name] = var(fresh)
    r_premises = tuple(_subst_formula(p, rename) for p in r.premises)
    r_conclusion = _subst_formula(r.conclusion, rename)
    r_var_sorts = {
        (rename[k].op if k in rename else k): v for k, v in r_var_sorts.items()
    }
    return r_premises, r_conclusion, r_var_sorts


def _verify_case_sorts(
    rel_name: str,
    rule_name: str,
    concl_args: tuple[Term, ...],
    case_sorts: dict[str, str],
    r_var_sorts: dict[str, str],
    avoid: set[str],
    param_sorts: dict[int, str],
    out_sorts_list: list[tuple[str, str]],
) -> None:
    """Validate and propagate sort assumptions introduced inside a case proof.

    Any sort assumption introduced inside a case proof (e.g. via `lattice_decide`)
    must either be on a free var of `prop` (propagated outward), on a bare
    conclusion argument (propagated onto the quantified parameter binder), or
    already declared/enforced on `r.var_sorts` (enforced by `apply_rule`).
    Silently dropping a sort on an unsorted premise-only rule variable is unsound.
    """
    bare_concl_vars = {
        arg.op for arg in concl_args if not arg.args and not _is_constant_name(arg.op)
    }
    for var_name, sort in case_sorts.items():
        if var_name in avoid:
            out_sorts_list.append((var_name, sort))
        elif var_name in bare_concl_vars:
            pass
        else:
            declared = r_var_sorts.get(var_name)
            _require(
                declared == sort,
                f"rule_induction case `{rule_name}` assumed sort `{sort}` on unsorted "
                f"rule-local variable `{var_name}` (declared: `{declared}`)",
            )
    for i, arg in enumerate(concl_args):
        inferred: str | None = None
        if not arg.args and arg.op in case_sorts:
            inferred = case_sorts[arg.op]
        elif arg.args and arg.op in DEPTH_OPS and (free_vars_term(arg) & case_sorts.keys()):
            inferred = "Depth"
        elif arg.args and arg.op in SET_OPS and (free_vars_term(arg) & case_sorts.keys()):
            inferred = "Set"
        if inferred is not None:
            prev = param_sorts.get(i)
            _require(
                prev is None or prev == inferred,
                f"rule_induction on `{rel_name}` parameter {i} has conflicting sorts `{prev}` and `{inferred}`",
            )
            param_sorts[i] = inferred


def _prop_param_sorts(rel_name: str, params_snap: tuple[str, ...], prop: Formula) -> dict[int, str]:
    p_idx = {name: i for i, name in enumerate(params_snap)}
    found: dict[int, str] = {}

    def mark(i: int, sort: str) -> None:
        prev = found.get(i)
        _require(
            prev is None or prev == sort,
            f"rule_induction on `{rel_name}` parameter `{params_snap[i]}` used as both `{prev}` and `{sort}`",
        )
        found[i] = sort

    def scan_t(t: Term, sort: str) -> None:
        if not t.args:
            if t.op in p_idx:
                mark(p_idx[t.op], sort)
        elif t.op == "max":
            for a in t.args:
                scan_t(a, "Depth")
        elif t.op == "max_set" and len(t.args) == 1:
            scan_t(t.args[0], "Set")
        elif t.op == "singleton" and len(t.args) == 1:
            scan_t(t.args[0], "Depth")
        elif t.op == "∪":
            for a in t.args:
                scan_t(a, "Set")

    def scan_f(f: Formula) -> None:
        if f.kind in {"<=", "<", "=_d", "!=_d"}:
            scan_t(f.terms[0], "Depth")
            scan_t(f.terms[1], "Depth")
        elif f.kind in {"∈", "∉"}:
            scan_t(f.terms[0], "Depth")
            scan_t(f.terms[1], "Set")
        elif f.kind in {"⊆", "⊈"}:
            scan_t(f.terms[0], "Set")
            scan_t(f.terms[1], "Set")
        else:
            for s in f.subs:
                scan_f(s)

    scan_f(prop)
    return found


def rule_induction(
    rel: InductiveRelation,
    params: tuple[str, ...],
    prop: Formula,
    cases: dict[str, Callable[[tuple[Theorem, ...], tuple[Theorem, ...]], Theorem]],
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

    def _inst_prop(args: tuple[Term, ...]) -> Formula:
        return _subst_formula(prop, dict(zip(params_snap, args)))

    avoid = free_vars_formula(prop) - set(params_snap)
    param_sorts = _prop_param_sorts(rel_name, params_snap, prop)
    out_sorts_list: list[tuple[str, str]] = []
    for r in rules:
        r_premises, r_conclusion, r_var_sorts = _alpha_rename_rule(r, avoid, params_snap)
        prem_hyps = tuple(assume(p) for p in r_premises)
        ih_hyps = tuple(
            assume(_inst_prop(p.terms[0].args))
            for p in r_premises
            if p.kind == "Pred" and p.terms[0].op == rel_name
        )

        case_thm = cases[r.name](prem_hyps, ih_hyps)
        ca, cc, cs = _require_thm(case_thm, f"rule_induction case `{r.name}`")
        expected = _inst_prop(r_conclusion.terms[0].args)
        _require(
            cc == expected,
            f"rule_induction case `{r.name}` proved wrong conclusion:\n"
            f"  expected: {expected}\n  got:      {cc}",
        )
        allowed = {h.conclusion for h in prem_hyps} | {h.conclusion for h in ih_hyps}
        extra = ca - allowed
        _require(
            not extra,
            f"rule_induction case `{r.name}` left undischarged assumptions: {extra}",
        )
        _verify_case_sorts(
            rel_name,
            r.name,
            r_conclusion.terms[0].args,
            dict(cs),
            r_var_sorts,
            avoid,
            param_sorts,
            out_sorts_list,
        )

    generic_names: list[str] = []
    for i in range(rel_arity):
        name = f"_a{i}"
        while name in avoid or name in generic_names:
            name = f"{name}_"
        generic_names.append(name)
    generic_args = tuple(var(n) for n in generic_names)
    goal = imp(pred(rel_name, *generic_args), _inst_prop(generic_args))
    for i in range(rel_arity - 1, -1, -1):
        goal = forall(generic_args[i].op, goal, param_sorts.get(i, ""))
    return _thm([], goal, _merge_sorts(out_sorts_list))
