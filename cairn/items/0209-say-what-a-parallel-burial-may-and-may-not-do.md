---
id: 209
title: Say what a parallel burial may and may not do
type: spec
status: buried
milestone: cortege
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-09-13
updated: 2026-10-30
priority: p0
effort: m
area: spec/06-evaluation.md
stratum: '0'
proof: Section 06 specifies fixed and free orders with serial-equivalence fixtures; 0204–0206 discharge parallel conformance against that contract
---

6.2 says evaluation order is unspecified except that it must be deterministic
for a given source and capability set. That sentence was written when there was
one thread.

It has to say more now: which observable orders are part of the trace — hole
discovery, deposits, witnesses — and which are genuinely free. An
implementation that parallelises without that written down is guessing at what
it is allowed to reorder.

## Delivery plan — 2026-09-15

### Starting point and scope

§6.2 fixes demand source order and hole discovery; §6.4 fixes fuel. This specification must not wait for its dependent implementation.

### Steps

1. Enumerate demand, binding, deposit, hole-dedup/span, witness, collapse and exhaustion order.
2. Separate speculative pure work from committed outcomes and identify effects that may overlap.
3. Specify serial-equivalence fixtures and rejected alternatives before 0204/0206 implementation.

### Acceptance and evidence

- [x] Land the contract and serial fixtures first. Parallel conformance is discharged by 0204–0206, avoiding a circular completion gate.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Buried. §6.2.1 states serial equivalence: a parallel burial MUST leave exactly the trace §6.2's order would and halt with the same diagnostic at the same place. Fixed: hole order and the span a shared hole keeps, deposit order, witness order, which halt comes first (a collapse found ahead of need is not reached until the serial order reaches it), and the steps reported. Free: pure work in any order on any thread, including thrown-away work, under §6.5's three refusals; and the order objects reach the store. Reaching the world in parallel is §6.6's business, not this. Serial fixture: tests/programs/order.nc, lifted verbatim into §6.2.1 with an executed transcript: b declared first, a asked first, so a is hole ① and b ②; deposits one then two in demand order across a demand that starved. §90.2 records comparing traces as sets as the rejected alternative. Parallel conformance against this contract is 0204-0206's, as the item intends.
