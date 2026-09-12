---
id: 156
title: bury accepts --grant and ignores it
type: bug
status: buried
milestone: rites
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: crates/nether-cli
stratum: '0'
proof: Either --grant changes what a burial does, or it says that it does not
---

## The flag that does nothing

`crates/nether-cli/src/bury.rs` parses `--grant`, validates the capability
against §9.1, pushes it onto a vector, and then:

```rust
let _ = &granted;
```

[§8.2](../spec/08-rites.md) opens "Evaluates as far as the granted capabilities
allow", with no caveat. The two runs are byte-identical, down to the cairn:

```console
$ nether bury --grant disk build.nc -> 0380c8ae   depth 3   holes 1
$ nether bury             build.nc -> 0380c8ae   depth 3   holes 1
```

The comment in the code is honest about it. The user gets no signal at all.

## What to do

Granting for real is `exhume`'s job and that rite does not exist yet, so the
honest thing is for `bury` to refuse a grant it cannot honour and name the rite
that will. A flag that validates its argument and discards it is worse than one
that is not there: it looks like it worked.

## Acceptance criteria

- [x] `--grant` either changes the burial or is refused with the reason
- [x] §8.2 says which

## 2026-09-12

Refused rather than honoured, because honouring one is exhume's work and that rite is not built. Section 8.2 now makes it a MUST: an implementation that cannot honour a grant refuses one rather than evaluating as though it were not there. Same reasoning as section 8.0 in a smaller place.
