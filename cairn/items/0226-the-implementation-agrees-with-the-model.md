---
id: 226
title: The implementation agrees with the model
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: xl
area: crates/nether-core
stratum: '0'
proof: A generated program is checked by both the Rust checker and the mechanized model, and they agree, ten million times
---

A proof about a model is worth what the model has to do with the code.

Differential testing between the two is the cheap version of extraction and it
finds the interesting bugs: the ones where the proof is right and the
implementation is not. It also keeps the model honest, because a model that has
drifted from the code stops disagreeing with it.
