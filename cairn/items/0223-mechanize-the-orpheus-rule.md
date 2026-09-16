---
id: 223
title: Mechanize the Orpheus rule
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-15
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

- [ ] Both halves are machine checked under explicit assumptions. If necessity is false as written, publish the counterexample rather than silently weaken the claim.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
