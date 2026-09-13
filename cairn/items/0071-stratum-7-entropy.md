---
id: 71
title: 'Stratum 7: entropy'
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 67
created: 2026-09-10
updated: 2026-09-13
priority: p1
effort: m
area: crates/nether-world
stratum: '7'
proof: A trace that touched entropy is marked, and replay of it is exact
---

Terry's Oracle, buried. Randomness must be named and seeded, and the seed
becomes part of the program's identity. A trace that reached 7 carries that
fact permanently — it is the one thing a trace can never stop admitting.

## 2026-09-13

The item said randomness must be named and seeded. §9.7 says the opposite about seeding -- draw is the only nondeterminism in the language and re-derivation is what it costs -- so what is named is the draw itself, by cairn, in the witness. The specification comes before the compiler.

## 2026-09-13

/dev/urandom, opened at the grant rather than at the draw: a machine with nothing to draw from cannot grant entropy at all, and finding that out halfway through a burial is worse. Nothing here mixes, stretches or seeds anything, which would be writing cryptography.

## 2026-09-13

Refuse gained a third case. §9.9 has two failures and neither fits a machine that promised a source at the grant and has none at the call: the program did nothing wrong and the world said nothing. It exits 69, not 1.
