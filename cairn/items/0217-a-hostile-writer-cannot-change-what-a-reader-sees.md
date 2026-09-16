---
id: 217
title: A hostile writer cannot change what a reader sees
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: l
area: crates/nether-ledger
stratum: '4'
proof: A writer with full write access to a store cannot make any get() return bytes that do not hash to the cairn asked for
---

This is the store's central security claim and it has never been attacked.

`get` verifies, so the obvious attack fails. The interesting ones are around
it: the reverse index is not verified, `resolve` trusts filenames, and an
attacker who can create files can create a short-prefix collision that makes
`resolve` ambiguous — turning a working command into a refusal, which is a
denial of service rather than a lie but is still theirs to cause.

## Delivery plan — 2026-09-15

### Starting point and scope

Tamper rejection and concurrent writers already have tests. Extend coverage to names, prefixes, reverse indexes and races.

### Steps

1. Use 0216 to distinguish integrity failures from absence, ambiguity and denial of service.
2. Race reads against replacements, malformed objects, partial indexes and prefix collisions in disposable stores.
3. Assert every successful get matches the requested cairn; ensure callers do not trust unverified index claims.

### Acceptance and evidence

- [ ] A hostile writer can cause refusal but not successful misnamed content. Retain exploit regressions and document availability/resource limits.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
