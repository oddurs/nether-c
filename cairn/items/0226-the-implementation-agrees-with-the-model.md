---
id: 226
title: The implementation agrees with the model
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-15
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

## Delivery plan — 2026-09-15

### Starting point and scope

The model needs comparable outputs before differential checking means anything. Reuse 0222 and seeded calculus generators.

### Steps

1. Define a shared input subset and normalized success/depth/error results, including invalid programs.
2. Drive both checkers on identical seeded inputs with bounds and mismatch minimization.
3. Run ten million cases, recording versions/seeds/counts; retain a smaller deterministic CI corpus.

### Acceptance and evidence

- [ ] Both checkers agree on the ten-million-case corpus and regressions. This tests correspondence, not Rust correctness or syntax outside the shared model.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
