---
id: 193
key: quickening
title: The Quickening
type: milestone
status: marked
created: 2026-09-13
updated: 2026-09-15
priority: p2
due: 2027-12-01
proof: Performance gates detect the stated regression, cache/allocation claims are directly measured, and changed budgets are justified. Every optimization retains canonical-byte and diagnostic regressions.
---

## Outcome and current boundary

Evidence-led performance work with canonical results and fuel semantics preserved. Existing interpreter step budgets are a starting point, not a whole-system benchmark.

## Sequence

0103 → 0199 → 0198 and 0203 is the pilot measurement chain. Preliminary instrumentation can happen now. 0201 read caching and 0202 encoding allocation are bounded independent measurements. 0200 requires a fuel/cache semantics decision before code.

## Exit gate

Performance gates detect the stated regression, cache/allocation claims are directly measured, and changed budgets are justified. Every optimization retains canonical-byte and diagnostic regressions.

The due date is a planning target, not a commitment or a replacement for evidence.
