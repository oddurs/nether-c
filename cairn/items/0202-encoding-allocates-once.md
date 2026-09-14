---
id: 202
title: Encoding allocates once
type: chore
status: unmarked
milestone: quickening
created: 2026-09-13
updated: 2026-09-13
priority: p2
effort: m
area: crates/nether-ledger
stratum: '0'
proof: Encoding a node of known size performs one allocation, asserted in a test
---

`encode` builds a `Vec` and grows it. The size is computable before the
first byte is written, and a million-node burial does this a million times.

Small, measurable, and the kind of thing that is easy while the encoder is
still four hundred lines.
