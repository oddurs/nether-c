---
id: 220
title: A trace's claims are checkable by somebody who does not trust it
type: feature
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: m
area: crates/nether-cli
stratum: '1'
proof: nether strata on a trace that under-reports its depth says so, and says it about a trace nobody trusts
---

A trace records its own depth and its own stratum-8 mark. A hostile one can
record whatever it likes.

`strata` already refuses a trace that under-reports — that landed in 0064 —
and the general principle should be stated and tested: every claim a trace
makes about itself is either derivable from its nodes or worthless. Which ones
are which belongs in section 08.
