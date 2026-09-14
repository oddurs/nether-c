---
id: 203
title: A fuel budget that means something
type: spec
status: unmarked
milestone: quickening
depends_on:
- 199
created: 2026-09-13
updated: 2026-09-13
priority: p2
effort: s
area: spec/06-evaluation.md
stratum: '0'
proof: The default budget is justified by a measurement rather than chosen
---

`DEFAULT_FUEL` is 1,000,000 because it looked about right. 6.4 requires the
budget to be deterministic and finite and says nothing about what it should be.

Once burial is profiled, pick it from evidence: large enough that no reasonable
program hits it, small enough that a runaway stops in a second rather than a
minute. Then say which it is in 6.4.
