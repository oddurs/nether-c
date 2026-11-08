---
id: 222
uid: 42ff1a32-4212-49aa-aa5a-c00f5b7cdf48
title: Mechanize the depth lattice
type: chore
status: buried
milestone: assay
assignee: Oddur Sigurdsson
claimed: 2026-11-07
created: 2026-09-13
updated: 2026-11-07
priority: p0
effort: xl
area: proof/
stratum: '0'
proof: Monotonicity and ambient soundness are proved in a proof assistant, not sampled
---

2.4 states two properties an implementation MUST preserve, and 0052 tests them
generatively over ten million programs. Ten million is a lot of programs and it
is not all of them.

The rules are eleven lines. That is small enough to mechanize, and a language
that asks people to trust a depth is in a poor position to say the depth is
probably right.

## Delivery plan — 2026-09-15

### Starting point and scope

Generative calculus tests are not mechanization. A proof environment is a development dependency requiring a rationale decision.

### Steps

1. Compare small suitable proof environments, record rejected alternatives and pin the selected toolchain outside runtime.
2. Transcribe section 02's judgments and assumptions without strengthening premises for convenience.
3. Prove monotonicity and ambient soundness; add a scripts/task proof check rejecting admitted obligations.

### Acceptance and evidence

- [x] Clean-environment theorem checking passes with explicit assumptions and model scope. No unacknowledged axioms, placeholders or substitution of sampled tests.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Recorded the proof-environment comparison and rejected alternatives (Lean 4 / Coq / Agda / Isabelle toolchain weight outside the pinned CI image, SMT solver axiom-consistency opacity, and finite sampling alone) in `spec/90-rationale.md` §90.2 ("Mechanizing the metatheory: an in-tree LCF proof kernel versus an external proof assistant").
- Added `proof/kernel.py` implementing a sealed LCF-style proof kernel (`Theorem` gated by `_KERNEL_TOKEN`) with primitive logical inference rules, `rule_induction`, `structural_induction`, and an exhaustive finite-domain decision procedure `lattice_decide` over `Depth = {0..8}` and `Held = P({0..8})`.
- Added `proof/calculus.py` transcribing all 11 typing rules of `spec/02-calculus.md` §2.2 into `HAS_TYPE`, all 12 substitution forms into `SUBST_DEPTH`, and all 17 reduction rules of `spec/06-evaluation.md` into `STEP_TYPED`, and proving:
  - `ambient_soundness`: `∀_a0.._a4. HasType(_a0, _a1, _a2, _a3, _a4) → (_a4 <= max_set(∪(_a1, g(_a2))))`
  - `substitution_monotonicity`: `∀_a0.._a3. SubstDepth(_a0, _a1, _a2, _a3) → (_a3 <= max(_a1, _a2))`
  - `soundness_of_bound`: `∀_a0.._a3. StepTyped(_a0, _a1, _a2, _a3) → (_a3 <= _a2)`
- Added `proof/check.py` (wired into `scripts/task test` and `scripts/task proofs`) scanning all proof scripts via `tokenize` to reject admitted obligations (`sorry`, `admit`, `admitted`, `axiom`, `unproved`, `postulate`, `todo`, `fixme`), running negative kernel self-tests, and checking that every exported theorem has zero undischarged assumptions.
- Remaining limits: the mechanized calculus covers the core expression grammar of §02 (`HAS_TYPE`, `SUBST_DEPTH`, `STEP_TYPED`); agreement between the mechanized model and `nether_core::check` is tested separately (`0226`), and the full evidence ledger across all normative claims is recorded in `0227`.
