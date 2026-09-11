---
id: 4
key: ledger
title: The Ledger
type: milestone
status: buried
created: 2026-09-10
updated: 2026-09-11
priority: p2
due: 2026-11-30
---

## 2026-09-11

Buried. Canonical encoding frozen and complete, cairns under a versioned domain separator, six node kinds, a content-addressed store with a reverse index, and both proofs measured: a million nodes round-trip byte-identical at 3.1M nodes/sec, and a one-source edit over 8,192 sources reuses 99.93% of 40,958 nodes.

Three specification bugs surfaced from writing the implementation: the tag table named types without specifying their bytes, a hole recorded dependents it cannot record, and 7.5 claimed the store holds only nodes when values have cairns too. The fuzzer then found an integer overflow in the decoder's own error path.

None of those would have been found by reading.
