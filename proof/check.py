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
    Theorem,
    app,
    assume,
    conj,
    elem,
    eq,
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
KERNEL_PRIVATE = frozenset({"_SEAL", "_thm", "_token"})


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

    # 8. Incomparable sets with equal maxima exist: `|- ¬((δ1 ⊈ δ2 ∧ δ2 ⊈ δ1) ∧ max_set(δ1)=max_set(δ2))` must fail
    try:
        lattice_decide(
            neg(
                conj(
                    conj(not_subset(d1_s, d2_s), not_subset(d2_s, d1_s)),
                    eq(app("max_set", d1_s), app("max_set", d2_s)),
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
        lattice_decide(eq(app("f", var("x")), var("f(x)")))
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


def verify_closed_theorem(name: str, thm: Theorem, expected_head: str = "") -> None:
    if not isinstance(thm, Theorem):
        raise ProofFault(f"`{name}` did not return a kernel Theorem")
    if thm.assumptions:
        raise ProofFault(f"`{name}` has undischarged assumptions: {thm.assumptions}")
    if expected_head and expected_head not in repr(thm.conclusion):
        raise ProofFault(f"`{name}` conclusion {thm.conclusion} does not match expected `{expected_head}`")


def main(argv: Sequence[str] = ()) -> int:
    _ = argv
    scanned = verify_no_admissions()
    verify_kernel_soundness_guards()

    t_amb = calculus.prove_ambient_soundness()
    verify_closed_theorem("ambient_soundness", t_amb, "HasType(_a0, _a1, _a2, _a3, _a4, _a5) → (_a4 <= max_set(∪(_a1, _a5)))")

    t_sub = calculus.prove_substitution_monotonicity()
    verify_closed_theorem("substitution_monotonicity", t_sub, "SubstDepth(_a0, _a1, _a2, _a3) → (_a3 <= max(_a1, _a2))")

    t_mon = calculus.prove_soundness_of_bound()
    verify_closed_theorem("soundness_of_bound", t_mon, "StepTyped(_a0, _a1, _a2, _a3) → (_a3 <= _a2)")

    orpheus_thms = [
        ("look_sufficiency_ambient", orpheus.prove_look_sufficiency_ambient(), "max(d, d_p) <= max_set(∪(δ, g(s)))"),
        ("look_sufficiency_confinement", orpheus.prove_look_sufficiency_confinement(), "singleton(d) ⊆ ∪(δ, g(s))"),
        ("look_necessity_confinement", orpheus.prove_look_necessity_confinement(), "(singleton(d) ⊆ ∪(δ, ∅_raw)) ↔ (d ∈ δ)"),
        ("look_necessity_numeric", orpheus.prove_look_necessity_numeric(), "(max(d, 0) <= max_set(∪(δ, ∅_raw))) ↔ (d <= max_set(δ))"),
        ("numeric_vs_membership_counterexample", orpheus.prove_numeric_vs_membership_counterexample(), "singleton(3) ⊈ ∪(∅, singleton(5))"),
        ("binding_taint_violates_ambient", orpheus.prove_binding_taint_violates_ambient(), "¬(max(3, 0) <= max_set(∪(∅, ∅_raw)))"),
        ("scope_restain_shifts_obligation_to_caller", orpheus.prove_scope_restain_shifts_obligation_to_caller(), "d ∈ δ_caller"),
    ]
    for name, thm, head in orpheus_thms:
        verify_closed_theorem(name, thm, head)

    staging_thms = [
        ("roundtrip_identity", staging.prove_roundtrip_identity(), "RoundTrip(_a0, _a1) → (_a1 = _a0)"),
        ("hole_set_union_law", staging.prove_hole_set_union_law(), "s_k ∈ ∪(δ_A, δ_B)"),
        ("staging_idempotence_set", staging.prove_staging_idempotence_set(), "s_k ∈ δ_A"),
        ("staging_law", staging.prove_staging_law(), "StagedEval(_a0, _a1, _a2, _a3, _a4) → (_a3 = _a4)"),
        ("dependent_hole_non_monotone_counterexample", staging.prove_dependent_hole_non_monotone_counterexample(), "3 ∉ ∪(∅, singleton(5))"),
        ("cumulative_dependent_hole_law", staging.prove_cumulative_dependent_hole_law(), "δ_A ⊆ δ_B"),
    ]
    for name, thm, head in staging_thms:
        verify_closed_theorem(name, thm, head)

    n_fixtures = encoding.verify_codec_fixtures_and_exclusions()
    encoding_thms = [
        ("value_decode_encode", encoding.prove_value_decode_encode(), "ValRoundTrip(_a0, _a1, _a2, _a3) → ((_a2 = _a0) ∧ (_a3 = _a1))"),
        ("node_decode_encode", encoding.prove_node_decode_encode(), "NodeRoundTrip(_a0, _a1, _a2) → (_a2 = Pair(_a0, _a1))"),
        ("injectivity_from_left_inverse", encoding.prove_injectivity_from_left_inverse(), "(encode(x) = encode(y)) → (x = y)"),
        ("cairn_preimage_injectivity", encoding.prove_cairn_preimage_injectivity(), "(domain_prefix(encode(x)) = domain_prefix(encode(y))) → (x = y)"),
    ]
    for name, thm, head in encoding_thms:
        verify_closed_theorem(name, thm, head)

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
