---
id: 16
title: 'Decide: is stratum 8 admissible at all?'
type: spec
status: buried
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: s
area: spec/01-strata.md
stratum: '8'
proof: Either a written justification or its removal from the lattice
---

## The question

Stratum 8, the Unrecorded, is foreign code whose provenance the ledger cannot
keep. It is the one stratum that breaks the language's central promise rather
than merely deepening it.

Two honest positions: admit it and quarantine it loudly, or refuse it and
accept that Nether C cannot call C. Refusing is purer and probably fatal to
adoption. Admitting it means every downstream claim needs the qualifier
"unless the trace touched stratum 8".

## Acceptance criteria

- [x] A decision, written down
- [x] If admitted: what a trace that touched 8 is permitted to claim

## 2026-09-10

PROPOSED, in spec/01-strata.md 1.7: admit it, quarantine it loudly. A stratum 8 trace is marked permanently and transitively and may never claim replayability. spec/90-rationale.md 90.2 records that refusing foreign code entirely is purer and calls this the weakest part of the design.

## 2026-09-10

SETTLED. Stratum 8 exists and is quarantined: a trace that reaches it is marked permanently and transitively and may never claim replayability. Refusing foreign code entirely is purer and is recorded in 90.2 as rejected.
