---
id: 161
title: as_unit drops a demand rather than failing
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p3
effort: s
area: crates/nether-bury
stratum: '0'
proof: A residue with a different number of demands than the unit it came from fails rather than truncating
---

## The zip

`Residue::as_unit` rebuilds a unit from the residue and the one that was
buried:

```rust
demands: self.demands.iter().zip(&buried.demands).map(...)
```

`zip` stops at the shorter. Burial produces one residual per demand, so the
lengths match — and if they ever did not, the residue would silently lose a
demand and the staging law would break with no diagnostic.

It is the cheap kind of wrong: correct today, quiet if it stops being.

## Acceptance criteria

- [ ] A length mismatch is a panic with a reason, not a truncation
