---
id: 170
title: Where a frozen environment is declared
type: spec
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: spec
stratum: '2'
proof: Exhuming with a declared environment is a documented invocation, and nothing in section 07 changed
---

§9.4 says the declaration "lives outside the program, in the burial
invocation", and [§8.3](../spec/08-rites.md) gives `exhume` no way to supply
one. 0068 cannot be built until it does: an `env` provider with nothing to
declare is a provider that refuses everything, and one that reads the host's
environment is the ambient authority §9.1 exists to refuse.

## And one sentence that is not true

§9.4:

> A trace records the declared set and every value read from it.

[§7.3.1](../spec/07-ledger.md) is **frozen** and a `Trace` has no field for a
declared set. One of the two has to give.

The second half is already true: a read produces a `Witness`, a refusal is an
answer and is witnessed like any other, and reading something never declared
collapses and leaves no trace at all. So everything the declaration did to the
program is recorded. What is not recorded is a declaration the program never
read — which changed nothing, and which a cairn therefore should not
distinguish.

## Acceptance criteria

- [x] §8.3 says how a declaration reaches `exhume`
- [x] §9.4 and §7.3.1 agree, with the frozen encoding unchanged
- [x] The rejected alternative is in `spec/90-rationale.md`

## 2026-09-13

The frozen encoding stays. A declared name that was read is already a Witness -- including one declared and unset, because §9.9 makes a refusal an answer -- and one that was never declared collapses, which produces no trace at all. The only thing a declared-set field could record is a name nothing read.
