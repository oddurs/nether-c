---
id: 162
title: A ledger was committed to the repository
type: chore
status: buried
milestone: rites
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: .gitignore
stratum: '0'
proof: Burying a program in the checkout leaves the working tree clean
---

## The litter

`nether bury` writes to `$NETHER_STORE`, or to `.nether` beside the work when
nobody says otherwise. `.nether` is not in `.gitignore`, so running a rite in
the checkout leaves a store in the working tree, and the next `git add -A`
sweeps it in.

Fifteen files are committed. They arrived in two goes — #90, when the rite
first ran, and #105, when a review ran it again — and they hold a trace of
`hello.nc` and one of a throwaway probe. Nothing reads them: every test uses a
scratch directory or removes `NETHER_STORE` from the environment, and the
transcript harness makes its own.

It is only litter, but it is the kind that grows: every burial anybody runs
from the checkout adds to it, and content-addressed objects never collide, so
nothing ever overwrites and nothing ever shrinks. §7.6 is explicit that a store
has no garbage collection.

## Acceptance criteria

- [x] Burying a program in the checkout leaves the working tree clean
- [x] The committed store is gone

## 2026-09-12

Fifteen files, arrived in two goes: PR 90 when the rite first ran, and PR 105 when a review ran it again. They held a trace of hello.nc and one of a throwaway probe, and nothing read them -- every test uses a scratch directory or removes NETHER_STORE.
