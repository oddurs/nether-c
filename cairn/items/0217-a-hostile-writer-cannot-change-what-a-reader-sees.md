---
id: 217
title: A hostile writer cannot change what a reader sees
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-13
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
