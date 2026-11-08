# Mechanized Proofs (`proof/`)

This directory holds the machine-checked metatheory of NetherC.

## Architecture

- **`kernel.py`** — Small LCF-style proof kernel in pure Python 3.12 (zero external
  dependencies). `Theorem` objects can only be constructed by the kernel's
  primitive inference rules (`assume`, `mp`, `imp_intro`, `conj_intro`,
  `conj_elim_l`, `conj_elim_r`, `disj_intro_l`, `disj_intro_r`, `disj_elim`,
  `iff_intro`, `inst`, `gen`, `refl`, `sym`, `trans`, `cong`, `rewrite`),
  inductive derivation principles (`apply_rule`, `rule_induction`,
  `structural_induction`), and the finite depth-lattice / capability-set
  decision procedure (`lattice_decide` over `Depth = {0..8}` and
  `Held = P({0..8})`).
- **`calculus.py`** — Formalization of `spec/02-calculus.md` (§2.2's eleven
  typing rules in `HAS_TYPE`, substitution depth bounds in `SUBST_DEPTH`, and
  §06's reduction steps in `STEP_TYPED`) and machine-checked proofs of §2.4:
  - `prove_ambient_soundness()`: `Γ ; δ ⊢ e : τ@d → d <= max(δ ∪ g(e))` across
    all 11 typing rules.
  - `prove_substitution_monotonicity()`: `SubstDepth(e, d, d_a, d_sub) → d_sub <= max(d, d_a)`
    across all 12 substitution forms.
  - `prove_soundness_of_bound()`: `StepTyped(e, e', d, d') → d' <= d` across
    all 17 reduction rules.
- **`orpheus.py`** — Formalization of `[LOOK]` (`spec/01-strata.md` §1.6,
  `spec/02-calculus.md` §2.2, `spec/90-rationale.md` §90.2), proving sufficiency
  (`look_sufficiency_ambient`, `look_sufficiency_confinement`), exact necessity
  for capability confinement (`look_necessity_confinement`) and numeric ceiling
  (`look_necessity_numeric`), the non-interval counterexample separating
  `d <= max_set(δ)` from `d ∈ δ` (`numeric_vs_membership_counterexample` on
  `d = 3, δ = {0, 5}`), and the two rejected alternatives from §90.2
  (`binding_taint_violates_ambient`, `scope_restain_shifts_obligation_to_caller`).
- **`staging.py`** — Formalization of the staging law (`spec/06-evaluation.md`
  §6.5 & §6.8), proving residual round-trip identity (`roundtrip_identity` over
  12 constructors), capability-set union and idempotence (`hole_set_union_law`,
  `staging_idempotence_set`), structural staged/direct equivalence (`staging_law`
  over 17 evaluation/residualisation rules under a fixed answer environment and
  sufficient fuel), the non-monotone dependent-hole counterexample
  (`dependent_hole_non_monotone_counterexample` on `A = {0, 3}, B = {0, 5}`),
  and the cumulative dependent-hole theorem (`cumulative_dependent_hole_law`
  under `A ⊆ B`).
- **`check.py`** — Entry point invoked by `scripts/task proofs` and
  `scripts/task test`. Scans all proof modules via `tokenize` to reject any
  admitted obligation (`sorry`, `admit`, `axiom`, `postulate`, `todo`, `fixme`),
  runs negative kernel self-tests (forged theorems, missing induction cases,
  undischarged hypotheses, invalid lattice inequalities), and checks every
  exported theorem for zero undischarged assumptions.

## Running

```sh
scripts/task proofs
```
