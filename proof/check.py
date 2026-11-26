#!/usr/bin/env python3
"""Verify all machine-checked proofs in `proof/` and reject admitted obligations."""
from __future__ import annotations

import io
from pathlib import Path
import sys
import tokenize
from typing import Sequence

from kernel import (
    Formula,
    InductiveRelation,
    ProofFault,
    Rule,
    Term,
    Theorem,
    app,
    apply_rule,
    assume,
    conj,
    disj,
    elem,
    eq,
    eq_d,
    forall,
    gen,
    imp,
    inst,
    lattice_decide,
    le,
    neg,
    not_subset,
    pred,
    refl,
    rewrite,
    rule_induction,
    subset,
    subst_term,
    sym,
    var,
)
import calculus
import encoding
import orpheus
import staging

PROOF_DIR = Path(__file__).resolve().parent

FORBIDDEN_IDENTIFIERS = frozenset(
    {
        "admit",
        "admitted",
        "sorry",
        "axiom",
        "unproved",
        "unproved_axiom",
        "postulate",
    }
)

# The seal is a Python name, so any proof script could import it and mint a
# theorem without an inference rule. Only the kernel may name it. A string
# passed to `getattr` still gets past this scan, which a reviewer must catch.
KERNEL_PRIVATE = frozenset(
    {
        "_SEAL",
        "_thm",
        "_token",
        "_TERM_DATA",
        "_FORMULA_DATA",
        "_RULE_DATA",
        "_THEOREM_DATA",
        "_REL_DATA",
        "_RELATIONS",
        "_LATTICE_FN_SORTS",
        "_term_payload",
        "_formula_payload",
        "_rule_payload",
        "_thm_payload",
        "_rel_payload",
    }
)


def scan_source_for_admissions(path: Path, text: str) -> None:
    """Reject any proof script containing forbidden admission tokens or placeholders."""
    reader = io.StringIO(text).readline
    for tok in tokenize.generate_tokens(reader):
        if tok.type == tokenize.NAME and tok.string.lower() in FORBIDDEN_IDENTIFIERS:
            raise ProofFault(
                f"{path.name}:{tok.start[0]}:{tok.start[1]}: forbidden admission identifier `{tok.string}`"
            )
        if tok.type == tokenize.NAME and tok.string in KERNEL_PRIVATE and path.name != "kernel.py":
            raise ProofFault(
                f"{path.name}:{tok.start[0]}:{tok.start[1]}: `{tok.string}` belongs to the kernel"
            )
        if tok.type == tokenize.COMMENT:
            lower = tok.string.lower()
            for marker in ("todo", "fixme", "admit", "sorry"):
                if marker in lower:
                    raise ProofFault(
                        f"{path.name}:{tok.start[0]}:{tok.start[1]}: forbidden placeholder `{marker}` in comment"
                    )


def verify_no_admissions(directory: Path = PROOF_DIR) -> int:
    count = 0
    for path in sorted(directory.glob("*.py")):
        if path.name == "check.py":
            continue
        scan_source_for_admissions(path, path.read_text(encoding="utf-8"))
        count += 1
    return count


def verify_kernel_soundness_guards() -> None:
    """Self-test the LCF kernel: forged theorems, missing cases, undischarged hypotheses,
    false lattice claims, and admitted obligations must all be rejected."""
    p = pred("P", var("x"))

    # 1. Direct Theorem construction without the kernel token must fail
    try:
        Theorem(object(), frozenset(), p)
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed forged Theorem construction")

    # 2. False lattice inequality (`d1 <= d2` with no hypotheses) must fail
    try:
        lattice_decide(le(var("d1"), var("d2")))
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel accepted invalid lattice inequality")

    # 3. Rule induction with a missing rule case must fail
    rel = InductiveRelation(
        "DummyRel",
        (
            Rule("Base", (), pred("DummyRel", var("0"))),
            Rule("Step", (pred("DummyRel", var("d")),), pred("DummyRel", var("d"))),
        ),
    )
    try:
        rule_induction(
            rel,
            ("_d",),
            le(var("_d"), var("8")),
            {"Base": lambda _prems, _ihs: lattice_decide(le(var("0"), var("8")))},
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed rule_induction with missing rule case")

    # 4. Rule induction whose case leaves an extra undischarged assumption must fail
    extra = pred("UnprovedExtra", var("_d"))
    try:
        rule_induction(
            rel,
            ("_d",),
            extra,
            {
                "Base": lambda _prems, _ihs: assume(extra),
                "Step": lambda _prems, ihs: ihs[0],
            },
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed rule_induction with undischarged assumption")

    # 5. Admission scanner must reject `sorry` and `admit`
    try:
        scan_source_for_admissions(Path("synthetic.py"), "thm = sorry()\n")
    except ProofFault:
        pass
    else:
        raise AssertionError("scanner allowed `sorry` identifier")

    # 6. A proof script outside the kernel must not name the seal
    try:
        scan_source_for_admissions(Path("synthetic.py"), "from kernel import _SEAL\n")
    except ProofFault:
        pass
    else:
        raise AssertionError("scanner allowed a proof script to import the kernel seal")

    # 7. False set-inclusion claim `(max_set(δ1) <= max_set(δ2)) |- (δ1 ⊆ δ2)` must fail
    d1_s, d2_s = var("δ1"), var("δ2")
    try:
        lattice_decide(
            subset(d1_s, d2_s),
            [assume(le(app("max_set", d1_s), app("max_set", d2_s)))],
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide accepted false max_set -> subset claim")

    # 8. Incomparable sets with equal maxima exist: `|- ¬((δ1 ⊈ δ2 ∧ δ2 ⊈ δ1) ∧ max_set(δ1)=_d max_set(δ2))` must fail
    try:
        lattice_decide(
            neg(
                conj(
                    conj(not_subset(d1_s, d2_s), not_subset(d2_s, d1_s)),
                    eq_d(app("max_set", d1_s), app("max_set", d2_s)),
                )
            )
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide missed incomparable sets sharing a maximum")

    # 9. InductiveRelation must reject non-predicate / non-matching conclusions (`0 = 1`)
    try:
        InductiveRelation("Forge", (Rule("Bad", (), eq(var("0"), var("1"))),))
    except ProofFault:
        pass
    else:
        raise AssertionError("InductiveRelation allowed non-predicate conclusion")

    # 10. InductiveRelation must reject empty rules and non-positive recursion
    try:
        InductiveRelation("Empty", ())
    except ProofFault:
        pass
    else:
        raise AssertionError("InductiveRelation allowed empty rules")
    try:
        InductiveRelation("Neg", (Rule("Bad", (neg(pred("Neg", var("x"))),), pred("Neg", var("x"))),))
    except ProofFault:
        pass
    else:
        raise AssertionError("InductiveRelation allowed negative recursion")

    # 11. Constants cannot be generalized or bound
    try:
        gen("0", assume(eq(var("0"), var("x"))))
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed generalizing constant `0`")

    # 12. lattice_decide must not assume `0 ∈ δ` implicitly from variable name
    try:
        lattice_decide(elem(var("0"), var("δ")))
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide assumed implicit `0 ∈ δ` axiom")

    # 13. lattice_decide must distinguish opaque applications from variables
    try:
        lattice_decide(eq_d(app("f", var("x")), var("f(x)")))
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide conflated app `f(x)` with var `f(x)`")

    # 14. Nested implications must not use 3-element chain shortcut
    x0, x1, x2, x3 = var("x0"), var("x1"), var("x2"), var("x3")
    pigeon = imp(
        conj(conj(le(x0, x1), le(x1, x2)), le(x2, x3)),
        imp(
            imp(le(x1, x0), le(x3, x0)),
            imp(imp(le(x2, x1), le(x3, x0)), imp(imp(le(x3, x2), le(x3, x0)), le(x3, x0))),
        ),
    )
    try:
        lattice_decide(pigeon)
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide accepted 3-chain false nested implication")

    # 15. rule_induction must freshen rule variables colliding with prop free vars
    any_rel = InductiveRelation("AnyRel", (Rule("R1", (), pred("AnyRel", var("x"))),))
    try:
        rule_induction(
            any_rel,
            ("_a",),
            eq(var("_a"), var("x")),
            {"R1": lambda _p, _ih: refl(var("x"))},
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("rule_induction unified rule variable with prop free var")

    # 16. Conflicting InductiveRelation definitions for the same name must fail
    try:
        InductiveRelation("AnyRel", (Rule("R2", (), pred("AnyRel", var("1"))),))
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed conflicting InductiveRelation definitions")

    # 17. rewrite must not replace predicate head symbols
    rw = rewrite(assume(eq(app("P", var("x")), app("Q", var("x")))), assume(pred("P", var("x"))))
    if rw.conclusion != pred("P", var("x")):
        raise AssertionError(f"rewrite altered predicate head symbol: {rw.conclusion}")

    # 18. inst must avoid variable capture under quantifiers
    f_all = forall("x", forall("y", pred("R", var("x"), var("y"))))
    t_inst = inst(assume(f_all), var("y"))
    if t_inst.conclusion.binder == "y":
        raise AssertionError(f"inst captured bound variable `y`: {t_inst.conclusion}")

    # 19. Duck-typed and subclassed Theorem / InductiveRelation objects must be rejected
    class FakeThm:
        assumptions = frozenset()
        conclusion = eq(var("1"), var("0"))

    try:
        sym(FakeThm())  # type: ignore[arg-type]
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel accepted duck-typed Theorem in sym")

    try:
        type("EvilThm", (Theorem,), {})
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed subclassing Theorem")

    class FakeRel:
        name = "HasType"
        arity = 1
        rules = ()

    try:
        rule_induction(FakeRel(), ("_a",), eq(var("0"), var("1")), {})  # type: ignore[arg-type]
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel accepted duck-typed InductiveRelation in rule_induction")

    # 20. Polymorphic FOL `=` must not be decided by finite-domain lattice_decide
    try:
        lattice_decide(eq(var("0"), var("0")))
    except ProofFault:
        pass
    else:
        raise AssertionError("lattice_decide accepted polymorphic FOL `=`")

    # 21. Mutually negative InductiveRelation premises must be rejected
    try:
        InductiveRelation(
            "MutNegA",
            (Rule("NegStep", (neg(pred("MutNegB", var("x"))),), pred("MutNegA", var("x"))),),
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("InductiveRelation allowed negative predicate premise")

    # 22. Malformed Formula construction must be rejected
    try:
        Formula("=", (var("0"), var("0")), (pred("False"),), "0")
    except ProofFault:
        pass
    else:
        raise AssertionError("Formula accepted extraneous subs/binder")

    # 23. rule_induction must snapshot `params` against stateful iterables
    pair_rel = InductiveRelation("Pair01", (Rule("Mk", (), pred("Pair01", var("0"), var("1"))),))

    class EvilParams:
        def __init__(self) -> None:
            self.calls = 0

        def __len__(self) -> int:
            return 2

        def __iter__(self):
            self.calls += 1
            return iter(("_a", "_a") if self.calls == 4 else ("_a", "_b"))

    try:
        rule_induction(
            pair_rel,
            EvilParams(),  # type: ignore[arg-type]
            eq(var("_a"), var("1")),
            {"Mk": lambda _p, _ih: refl(var("1"))},
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("rule_induction accepted stateful params iterable")

    # 24. apply_rule must snapshot `subst` against stateful dict subclasses
    eq_zero = InductiveRelation(
        "EqZero",
        (Rule("R", (eq(var("x"), var("0")),), pred("EqZero", var("x"))),),
    )

    class EvilMap(dict):
        def __init__(self) -> None:
            super().__init__({"x": var("0")})
            self.n = 0

        def __getitem__(self, k: str) -> Term:
            self.n += 1
            return var("0") if self.n == 1 else var("1")

    res_map = apply_rule(eq_zero, "R", EvilMap(), [refl(var("0"))])
    if res_map.conclusion != pred("EqZero", var("0")):
        raise AssertionError(f"apply_rule accepted stateful dict substitution: {res_map.conclusion}")

    # 25. Theorem and InductiveRelation must not expose __dict__ or allow attribute mutation
    t0 = refl(var("0"))
    try:
        vars(t0)
    except TypeError:
        pass
    else:
        raise AssertionError("Theorem exposed __dict__ via vars()")
    try:
        object.__setattr__(t0, "conclusion", eq(var("0"), var("1")))
    except AttributeError:
        pass
    else:
        raise AssertionError("Theorem allowed object.__setattr__ mutation")
    try:
        object.__setattr__(pair_rel, "rules", ())
    except AttributeError:
        pass
    else:
        raise AssertionError("InductiveRelation allowed object.__setattr__ mutation")

    # 26. Reserved 0-ary constructors (Bool, VUnit, Nil) cannot be substituted as variables
    for cname in ("Bool", "VUnit", "Nil"):
        try:
            subst_term(var(cname), {cname: var("I64")})
        except ProofFault:
            pass
        else:
            raise AssertionError(f"subst_term treated reserved constant `{cname}` as a variable")

    # 27. lattice_decide must reject short-circuited out-of-range depths, reserved constants, and Unicode digits
    for bad_goal in (
        disj(le(var("0"), var("0")), le(var("999"), var("0"))),
        le(var("True"), var("8")),
        le(var("²"), var("8")),
    ):
        try:
            lattice_decide(bad_goal)
        except ProofFault:
            pass
        else:
            raise AssertionError(f"lattice_decide accepted invalid goal `{bad_goal}`")

    # 28. Term, Formula, and Rule must not expose __dict__ or allow object.__setattr__ mutation
    term_0 = var("0")
    form_0 = eq(term_0, term_0)
    rule_0 = Rule("R0", (), pred("P0", term_0))
    for obj, attr, bad_val in (
        (term_0, "op", "1"),
        (form_0, "terms", (var("0"), var("1"))),
        (rule_0, "conclusion", eq(var("0"), var("1"))),
    ):
        try:
            vars(obj)
        except TypeError:
            pass
        else:
            raise AssertionError(f"{type(obj).__name__} exposed __dict__ via vars()")
        try:
            object.__setattr__(obj, attr, bad_val)
        except AttributeError:
            pass
        else:
            raise AssertionError(f"{type(obj).__name__} allowed object.__setattr__ mutation")

    # 29. Quantifiers over lattice_decide variables must preserve Depth/Set sorts across gen/inst
    d_le_8 = gen("d", lattice_decide(le(var("d"), var("8"))))
    for bad_term in (var("9"), var("∅"), app("VUnit")):
        try:
            inst(d_le_8, bad_term)
        except ProofFault:
            pass
        else:
            raise AssertionError(f"inst allowed non-Depth term `{bad_term}` for Depth quantifier")
    x_le_8 = gen("x", inst(d_le_8, var("x")))
    try:
        inst(x_le_8, var("9"))
    except ProofFault:
        pass
    else:
        raise AssertionError("inst lost Depth sort after re-generalization")
    bad_ind = InductiveRelation("BadDepthRel", (Rule("R", (), pred("BadDepthRel", var("d"))),))
    bad_ind_thm = rule_induction(
        bad_ind,
        ("_d",),
        le(var("_d"), var("8")),
        {"R": lambda _p, _ih: lattice_decide(le(var("d"), var("8")))},
    )
    try:
        inst(bad_ind_thm, var("9"))
    except ProofFault:
        pass
    else:
        raise AssertionError("rule_induction lost Depth sort on inductive parameter")

    # 30. Re-calling __init__ must never mutate sealed kernel objects in place (C1)
    sealed_thm = refl(var("0"))
    try:
        sealed_thm.conclusion.__init__("=", (var("0"), var("1")))  # type: ignore[misc]
    except (TypeError, ProofFault):
        pass
    if sealed_thm.conclusion != eq(var("0"), var("0")):
        raise AssertionError(f"re-calling __init__ mutated sealed Theorem: {sealed_thm.conclusion}")

    # 31. rule_induction must reject assumed sorts on unsorted premise-only rule vars (C2)
    boom = InductiveRelation(
        "BoomUnsorted",
        (Rule("Mk", (eq(var("d"), var("9")),), pred("BoomUnsorted", var("0"))),),
    )
    try:
        rule_induction(
            boom,
            ("_a",),
            le(var("9"), var("8")),
            {"Mk": lambda prems, _ihs: rewrite(prems[0], lattice_decide(le(var("d"), var("8"))))},
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("rule_induction erased Depth sort on unsorted premise-only variable")

    # 32. lattice_decide must reject unregistered compound terms (C3)
    for bad_compound in (
        le(app("VInt", var("999")), var("8")),
        le(app("f", var("x")), var("8")),
        subset(app("foo", var("0")), app("foo", var("0"))),
    ):
        try:
            lattice_decide(bad_compound)
        except ProofFault:
            pass
        else:
            raise AssertionError(f"lattice_decide accepted unregistered compound term `{bad_compound}`")

    # 33. Bit-projection and membership weakening must still reject false set/depth claims
    for bad_opt in (
        neg(conj(not_subset(d1_s, d2_s), not_subset(d2_s, d1_s))),
        imp(le(var("d"), app("max_set", d1_s)), elem(var("d"), d1_s)),
        imp(elem(var("d"), d1_s), le(var("d"), var("0"))),
    ):
        try:
            lattice_decide(bad_opt)
        except ProofFault:
            pass
        else:
            raise AssertionError(f"lattice_decide accepted false set/depth claim `{bad_opt}`")


def verify_closed_theorem(name: str, thm: Theorem, expected_conclusion: str) -> None:
    if not isinstance(thm, Theorem):
        raise ProofFault(f"`{name}` did not return a kernel Theorem")
    if thm.assumptions:
        raise ProofFault(f"`{name}` has undischarged assumptions: {thm.assumptions}")
    if repr(thm.conclusion) != expected_conclusion:
        raise ProofFault(
            f"`{name}` conclusion mismatch:\n  expected: {expected_conclusion}\n  got:      {thm.conclusion}"
        )


def main(argv: Sequence[str] = ()) -> int:
    _ = argv
    scanned = verify_no_admissions()
    verify_kernel_soundness_guards()

    t_amb = calculus.prove_ambient_soundness()
    verify_closed_theorem(
        "ambient_soundness",
        t_amb,
        "(∀_a0. (∀_a1:Set. (∀_a2. (∀_a3. (∀_a4:Depth. (∀_a5:Set. (HasType(_a0, _a1, _a2, _a3, _a4, _a5) → (_a4 <= max_set(∪(_a1, _a5))))))))))",
    )

    t_sub = calculus.prove_substitution_monotonicity()
    verify_closed_theorem(
        "substitution_monotonicity",
        t_sub,
        "(∀_a0. (∀_a1:Depth. (∀_a2:Depth. (∀_a3:Depth. (SubstDepth(_a0, _a1, _a2, _a3) → (_a3 <= max(_a1, _a2)))))))",
    )

    t_mon = calculus.prove_soundness_of_bound()
    verify_closed_theorem(
        "soundness_of_bound",
        t_mon,
        "(∀_a0. (∀_a1. (∀_a2:Depth. (∀_a3:Depth. (StepTyped(_a0, _a1, _a2, _a3) → (_a3 <= _a2))))))",
    )

    orpheus_thms = [
        (
            "look_sufficiency_ambient",
            orpheus.prove_look_sufficiency_ambient(),
            "(∀d:Depth. (∀d_p:Depth. (∀δ:Set. (∀s. (((d_p <= max_set(∪(δ, g(s)))) ∧ (d ∈ δ)) → (max(d, d_p) <= max_set(∪(δ, g(s)))))))))",
        ),
        (
            "look_sufficiency_confinement",
            orpheus.prove_look_sufficiency_confinement(),
            "(∀d:Depth. (∀δ:Set. (∀s. ((d ∈ δ) → (singleton(d) ⊆ ∪(δ, g(s)))))))",
        ),
        (
            "look_necessity_confinement",
            orpheus.prove_look_necessity_confinement(),
            "(∀d:Depth. (∀δ:Set. ((singleton(d) ⊆ ∪(δ, ∅_raw)) ↔ (d ∈ δ))))",
        ),
        (
            "look_necessity_numeric",
            orpheus.prove_look_necessity_numeric(),
            "(∀d:Depth. (∀δ:Set. ((max(d, 0) <= max_set(∪(δ, ∅_raw))) ↔ (d <= max_set(δ)))))",
        ),
        (
            "numeric_vs_membership_counterexample",
            orpheus.prove_numeric_vs_membership_counterexample(),
            "((3 <= max_set(∪(∅, singleton(5)))) ∧ ((3 ∉ ∪(∅, singleton(5))) ∧ (singleton(3) ⊈ ∪(∅, singleton(5)))))",
        ),
        (
            "binding_taint_violates_ambient",
            orpheus.prove_binding_taint_violates_ambient(),
            "¬(max(3, 0) <= max_set(∪(∅, ∅_raw)))",
        ),
        (
            "scope_restain_shifts_obligation_to_caller",
            orpheus.prove_scope_restain_shifts_obligation_to_caller(),
            "(∀d:Depth. (∀δ_f:Set. (∀δ_caller:Set. (((singleton(d) ⊆ δ_f) ∧ (δ_f ⊆ δ_caller)) → (d ∈ δ_caller)))))",
        ),
    ]
    for name, thm, expected in orpheus_thms:
        verify_closed_theorem(name, thm, expected)

    staging_thms = [
        (
            "roundtrip_identity",
            staging.prove_roundtrip_identity(),
            "(∀_a0. (∀_a1. (RoundTrip(_a0, _a1) → (_a1 = _a0))))",
        ),
        (
            "hole_set_union_law",
            staging.prove_hole_set_union_law(),
            "(∀s_k:Depth. (∀δ_A:Set. (∀δ_B:Set. (((s_k ∈ δ_A) ∨ (s_k ∈ δ_B)) ↔ (s_k ∈ ∪(δ_A, δ_B))))))",
        ),
        (
            "staging_idempotence_set",
            staging.prove_staging_idempotence_set(),
            "(∀s_k:Depth. (∀δ_A:Set. (((s_k ∈ δ_A) ∨ (s_k ∈ δ_A)) ↔ (s_k ∈ δ_A))))",
        ),
        (
            "staging_law",
            staging.prove_staging_law(),
            "(∀_a0. (∀_a1. (∀_a2. (∀_a3. (∀_a4. (StagedEval(_a0, _a1, _a2, _a3, _a4) → (_a3 = _a4)))))))",
        ),
        (
            "dependent_hole_non_monotone_counterexample",
            staging.prove_dependent_hole_non_monotone_counterexample(),
            "((5 ∉ ∪(∅, singleton(3))) ∧ (((5 ∈ ∪(∅, singleton(5))) ∧ (3 ∉ ∪(∅, singleton(5)))) ∧ ((5 ∈ ∪(∪(∅, singleton(3)), ∪(∅, singleton(5)))) ∧ (3 ∈ ∪(∪(∅, singleton(3)), ∪(∅, singleton(5)))))))",
        ),
        (
            "cumulative_dependent_hole_law",
            staging.prove_cumulative_dependent_hole_law(),
            "(∀d_in:Depth. (∀d_out:Depth. (∀δ_A:Set. (∀δ_B:Set. ((δ_A ⊆ δ_B) → ((((d_in ∈ δ_A) ∧ (d_out ∈ δ_A)) ∨ ((d_in ∈ δ_B) ∧ (d_out ∈ δ_B))) ↔ ((d_in ∈ ∪(δ_A, δ_B)) ∧ (d_out ∈ ∪(δ_A, δ_B)))))))))",
        ),
    ]
    for name, thm, expected in staging_thms:
        verify_closed_theorem(name, thm, expected)

    n_fixtures = encoding.verify_codec_fixtures_and_exclusions()
    encoding_thms = [
        (
            "value_decode_encode",
            encoding.prove_value_decode_encode(),
            "(∀_a0. (∀_a1. (∀_a2. (∀_a3. (ValRoundTrip(_a0, _a1, _a2, _a3) → ((_a2 = _a0) ∧ (_a3 = _a1)))))))",
        ),
        (
            "node_decode_encode",
            encoding.prove_node_decode_encode(),
            "(∀_a0. (∀_a1. (∀_a2. (NodeRoundTrip(_a0, _a1, _a2) → (_a2 = Pair(_a0, _a1))))))",
        ),
        (
            "injectivity_from_left_inverse",
            encoding.prove_injectivity_from_left_inverse(),
            "(∀x. (∀y. ((decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((encode(x) = encode(y)) → (x = y))))))",
        ),
        (
            "cairn_preimage_injectivity",
            encoding.prove_cairn_preimage_injectivity(),
            "(∀x. (∀y. ((strip_domain(domain_prefix(encode(x))) = encode(x)) → ((strip_domain(domain_prefix(encode(y))) = encode(y)) → ((decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((domain_prefix(encode(x)) = domain_prefix(encode(y))) → (x = y))))))))",
        ),
    ]
    for name, thm, expected in encoding_thms:
        verify_closed_theorem(name, thm, expected)

    print(
        f"checked {scanned} proof modules (0 admitted obligations, kernel guards verified, {n_fixtures} codec fixtures verified)"
    )
    print(f"  [OK] ambient_soundness          ({len(calculus.HAS_TYPE.rules)} typing rules)   |- {t_amb.conclusion}")
    print(f"  [OK] substitution_monotonicity  ({len(calculus.SUBST_DEPTH.rules)} subst rules)    |- {t_sub.conclusion}")
    print(f"  [OK] soundness_of_bound         ({len(calculus.STEP_TYPED.rules)} step rules)     |- {t_mon.conclusion}")
    for name, thm, _ in orpheus_thms:
        print(f"  [OK] {name:42s} |- {thm.conclusion}")
    for name, thm, _ in staging_thms:
        print(f"  [OK] {name:42s} |- {thm.conclusion}")
    for name, thm, _ in encoding_thms:
        print(f"  [OK] {name:42s} |- {thm.conclusion}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
