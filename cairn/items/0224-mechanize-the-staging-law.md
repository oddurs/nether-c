---
id: 224
title: Mechanize the staging law
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
effort: xl
area: proof/
stratum: '0'
proof: bury(bury(p, A), B) = bury(p, A union B) is proved for the calculus in section 02
---

6.5's staging law is what makes descent incremental instead of all-or-nothing,
and it is what `graft` relies on. It is stated as a law and has never been
proved.

It is also the property most likely to be subtly false — a residue is source
now (0128), so the law has a print and a parse in the middle of it, and those
have to be exactly inverse for it to hold.

## Delivery plan — 2026-09-15

### Starting point and scope

§6.5 states equivalence, not necessarily equal historical trace envelopes. Model printed residue and supplied answers explicitly.

### Steps

1. Define equivalence, capabilities, fixed answer environment and fuel/termination assumptions first.
2. Model reduction, residue and print/parse/lower round-trip; use residue_is_source tests to expose omissions.
3. Prove staged/direct equivalence for the stated calculus; retain counterexamples as regressions/spec questions.

### Acceptance and evidence

- [x] The checked staging law states its assumptions. Do not promise identical histories for independently sampled mutable worlds or limited-fuel failures.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Added `proof/staging.py` (wired into `proof/check.py`) stating the explicit assumptions of the staging law (fixed deterministic answer environment `W`, sufficient fuel without budget exhaustion, `lower(parse(print(r))) = r` round-trip identity on residual expressions, and cumulative capabilities `δ_A ⊆ δ_B` for dependent nested holes) and proving:
  - `roundtrip_identity`: `∀_a0 _a1. RoundTrip(_a0, _a1) → (_a1 = _a0)` by rule induction over all 12 residual expression constructors (`Lit`, `Var`, `HolePure`, `Prim`, `Select`, `Descend`, `Seal`, `Shade`, `Look`, `Opaque`, `Demand`, `App`).
  - `hole_set_union_law`: `∀s_k δ_A δ_B. ((s_k ∈ δ_A) ∨ (s_k ∈ δ_B)) ↔ (s_k ∈ (δ_A ∪ δ_B))`
  - `staging_idempotence_set`: `∀s_k δ_A. ((s_k ∈ δ_A) ∨ (s_k ∈ δ_A)) ↔ (s_k ∈ δ_A)`
  - `staging_law`: `∀_a0.._a4. StagedEval(_a0, _a1, _a2, _a3, _a4) → (_a3 = _a4)` by rule induction over the 17 rules of `STAGED_EVAL` (chaining `roundtrip_identity` at intermediate residual boundaries and covering speculative arm reduction under undecided branches).
  - `dependent_hole_non_monotone_counterexample`: machine-checked counterexample when capabilities are dropped between stages (`A = {0, 3}`, `B = {0, 5}`) on a dependent nested question `read(must(get(url)))` (`5 ∉ A`, `5 ∈ B ∧ 3 ∉ B`, `5 ∈ (A ∪ B) ∧ 3 ∈ (A ∪ B)`).
  - `cumulative_dependent_hole_law`: `∀d_in d_out δ_A δ_B. (δ_A ⊆ δ_B) → (((d_in ∈ δ_A ∧ d_out ∈ δ_A) ∨ (d_in ∈ δ_B ∧ d_out ∈ δ_B)) ↔ (d_in ∈ (δ_A ∪ δ_B) ∧ d_out ∈ (δ_A ∪ δ_B)))`
- Extended `crates/nether-bury/tests/residue_is_source.rs` with `rites_descents_loops_aggregates_and_precedence_survive_round_trip` and `staged_burial_through_printed_residue_matches_direct_burial` verifying that two-stage burial (`env` then `disk`) through printed/re-parsed/re-lowered source matches single-stage burial (`env ∪ disk`).
- Remaining limits: independently sampled mutable worlds (`W_1 != W_2`) and executions that halt on limited-fuel exhaustion are outside the staging equivalence theorem.
