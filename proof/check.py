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
    assume,
    lattice_decide,
    le,
    pred,
    rule_induction,
    var,
)
import calculus
import orpheus

PROOF_DIR = Path(__file__).resolve().parent

FORBIDDEN_IDENTIFIERS = frozenset(
    {
        "admit",
        "admitted",
        "sorry",
        "axiom",
        "unproved",
        "postulate",
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
            lambda args: le(args[0], var("8")),
            {"Base": lambda _prems, _ihs: lattice_decide(le(var("0"), var("8")))},
        )
    except ProofFault:
        pass
    else:
        raise AssertionError("kernel allowed rule_induction with missing rule case")

    # 4. Rule induction whose case leaves an extra undischarged assumption must fail
    extra = pred("UnprovedExtra", var("d"))
    try:
        rule_induction(
            rel,
            lambda args: extra,
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


def verify_closed_theorem(name: str, thm: Theorem) -> None:
    if not isinstance(thm, Theorem):
        raise ProofFault(f"`{name}` did not return a kernel Theorem")
    if thm.assumptions:
        raise ProofFault(f"`{name}` has undischarged assumptions: {thm.assumptions}")


def main(argv: Sequence[str] = ()) -> int:
    _ = argv
    scanned = verify_no_admissions()
    verify_kernel_soundness_guards()

    t_amb = calculus.prove_ambient_soundness()
    verify_closed_theorem("ambient_soundness", t_amb)

    t_sub = calculus.prove_substitution_monotonicity()
    verify_closed_theorem("substitution_monotonicity", t_sub)

    t_mon = calculus.prove_soundness_of_bound()
    verify_closed_theorem("soundness_of_bound", t_mon)

    orpheus_thms = [
        ("look_sufficiency_ambient", orpheus.prove_look_sufficiency_ambient()),
        ("look_sufficiency_confinement", orpheus.prove_look_sufficiency_confinement()),
        ("look_necessity_confinement", orpheus.prove_look_necessity_confinement()),
        ("look_necessity_numeric", orpheus.prove_look_necessity_numeric()),
        (
            "numeric_vs_membership_counterexample",
            orpheus.prove_numeric_vs_membership_counterexample(),
        ),
        ("binding_taint_violates_ambient", orpheus.prove_binding_taint_violates_ambient()),
        (
            "scope_restain_shifts_obligation_to_caller",
            orpheus.prove_scope_restain_shifts_obligation_to_caller(),
        ),
    ]
    for name, thm in orpheus_thms:
        verify_closed_theorem(name, thm)

    print(f"checked {scanned} proof modules (0 admitted obligations, kernel guards verified)")
    print(f"  [OK] ambient_soundness          ({len(calculus.HAS_TYPE.rules)} typing rules)   |- {t_amb.conclusion}")
    print(f"  [OK] substitution_monotonicity  ({len(calculus.SUBST_DEPTH.rules)} subst rules)    |- {t_sub.conclusion}")
    print(f"  [OK] soundness_of_bound         ({len(calculus.STEP_TYPED.rules)} step rules)     |- {t_mon.conclusion}")
    for name, thm in orpheus_thms:
        print(f"  [OK] {name:42s} |- {thm.conclusion}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
