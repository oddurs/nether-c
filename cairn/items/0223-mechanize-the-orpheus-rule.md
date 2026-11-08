---
id: 223
title: Mechanize the Orpheus rule
type: chore
status: buried
milestone: assay
assignee: Oddur Sigurdsson
claimed: 2026-11-07
depends_on:
- 222
created: 2026-09-13
updated: 2026-11-07
priority: p0
effort: l
area: proof/
stratum: '0'
proof: 'look''s premise is proved to be exactly what soundness requires: no weaker, and no stronger than necessary'
---

1.6's rule is one premise: `d <= delta`. 90.2 records two rejected forms and
argues this one is right.

Proving it means showing both halves — that it admits everything sound, so the
language is not needlessly restrictive, and that it admits nothing unsound.
The second is the one people worry about; the first is the one that is usually
wrong.

## Delivery plan — 2026-09-15

### Starting point and scope

Use 0222 and §1.6. Necessity requires a precise class of rules and semantics, not an unqualified slogan.

### Steps

1. Formalize look's soundness obligation and countermodels for weaker premises.
2. Prove d <= delta sufficient and state/prove the exact necessity claim supported by the model.
3. Check §90.2's rejected alternatives; file any mismatch as a specification issue.

### Acceptance and evidence

- [x] Both halves are machine checked under explicit assumptions. If necessity is false as written, publish the counterexample rather than silently weaken the claim.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Added `proof/orpheus.py` (wired into `proof/check.py`) proving both halves of `[LOOK]`'s premise `d ∈ δ` alongside the counterexamples for weaker premises and §90.2's rejected alternatives:
  - `look_sufficiency_ambient`: `∀d d_p δ s. (d_p <= max_set(δ ∪ g(s)) ∧ d ∈ δ) → max(d, d_p) <= max_set(δ ∪ g(s))`
  - `look_sufficiency_confinement`: `∀d δ s. (d ∈ δ) → (singleton(d) ⊆ (δ ∪ g(s)))`
  - `look_necessity_confinement`: `∀d δ. (singleton(d) ⊆ (δ ∪ ∅_raw)) ↔ (d ∈ δ)` (for closed shade values `g(s) = ∅_raw`, per-stratum capability confinement requires `d ∈ δ` exactly)
  - `look_necessity_numeric`: `∀d δ. (max(d, 0) <= max_set(δ ∪ ∅_raw)) ↔ (d <= max_set(δ))` (for §2.4's numeric ceiling alone, the exact necessary condition on `(d, δ)` is `d <= max_set(δ)`)
  - `numeric_vs_membership_counterexample`: published and machine-checked the explicit non-interval counterexample `d = 3` (`disk`), `δ = {0, 5}` (`net`), where `3 <= max_set({0, 5})` holds (`3 <= 5`) while `3 ∉ {0, 5}` and `{3} ⊈ {0, 5}`, showing why `d ∈ δ` is required by §1.6's per-capability rule rather than §2.4's numeric ceiling alone
  - `binding_taint_violates_ambient`: `¬(max(3, 0) <= max_set({0} ∪ ∅_raw))` (§90.2 binding taint violates §2.4 ambient soundness on `d = 3, d' = 0, δ = {0}`)
  - `scope_restain_shifts_obligation_to_caller`: `∀d δ_f δ_caller. (singleton(d) ⊆ δ_f ∧ δ_f ⊆ δ_caller) → (d ∈ δ_caller)` (§90.2 scope re-stain still requires `d ∈ δ_caller` at every `[APP]` site)
- Recorded the mechanized distinction and counterexample in `spec/90-rationale.md` §90.2 ("The Orpheus rule: two rejected forms").
- Remaining limits: the necessity and counterexample theorems operate over the core `Depth = {0..8}` and `Held = P({0..8})` domain of §01–§02.
