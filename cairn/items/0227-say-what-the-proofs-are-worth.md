---
id: 227
title: Say what the proofs are worth
type: docs
status: unmarked
milestone: assay
depends_on:
- 223
- 224
- 225
- 226
created: 2026-09-13
updated: 2026-09-15
priority: p2
effort: s
area: spec/90-rationale.md
stratum: '0'
proof: Section 90 states what is proved, what is tested, and what is merely believed, with no overlap between the three
---

A project that mechanizes half its metatheory and tests the other half owes a
reader a clear account of which is which.

The temptation once there are proofs is to let the word cover everything
nearby. The honest version names the three tiers and puts each claim in exactly
one.

## Delivery plan — 2026-09-15

### Starting point and scope

Build the evidence ledger after 0222–0226, keeping failed/deferred work visible and model claims distinct from implementation claims.

### Steps

1. Inventory normative claims with theorem identifiers, test paths or explicit unproved status.
2. Record strongest evidence, assumptions, supported subset and reproduction command, allowing supporting lower-tier tests.
3. Update spec/90-rationale.md without duplicating rules or calling differential testing a proof.

### Acceptance and evidence

- [ ] Each claim has one primary evidence class and traceable support. No unfinished theorem, observation or long measurement is described as complete.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
