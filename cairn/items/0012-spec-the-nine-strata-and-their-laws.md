---
id: 12
title: 'Spec: the nine strata and their laws'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: spec/01-strata.md
proof: Every stratum has a stated grant, a stated cost, and a stated witness obligation
---

## What this section must answer

What each of the nine strata grants, what guarantee it costs, and what the
runtime is obliged to record when a program reaches it.

## Open questions

- [ ] Is 6 (write the network) genuinely deeper than 5 (read), or are they siblings?
- [ ] Should stratum 1 (Store) exist, or is reading the ledger simply pure?

## Acceptance criteria

- [ ] A table with grant / cost / witness for all nine
- [ ] The monotonicity law stated once, formally
- [ ] `descend` scoping rules, including nesting and early return

## 2026-09-10

Draft landed: spec/01-strata.md. Grant/cost/witness table for all nine, the monotonicity law, descent-as-expression, seal, the Orpheus rule, and the stratum 8 quarantine. Open: whether 6 is genuinely deeper than 5 (argued in 1.8, not settled).
