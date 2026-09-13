---
id: 168
title: exhume does not say what it did
type: bug
status: unmarked
milestone: rites
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: crates/nether-cli
stratum: '1'
proof: Every rite reports what it spent, and refuses a second cairn rather than picking one
---

## Three small ways it is quiet

**It does not say what it spent.** [§8.2](../spec/08-rites.md) says so in as
many words — "A rite reports what it spent, because that is where it is a
fact" — and `exhume` contains the word zero times. `bury --json` has it.

**It takes several cairns and uses the last.**

```console
$ nether exhume deadbeef 6941fe38
sealed   6941fe38 +  → 6941fe38   depth 0   holes 0
```

`deadbeef` was ignored without a word. `lamp` refuses this: "one cairn at a
time".

**It claims to have sealed something when it answered nothing.** The line
above has an empty `+` and a trace that is the one it started with. Nothing was
granted, so nothing was answered, and saying "sealed" for that is saying a
thing happened.

**And a hole whose node is not in the ledger is skipped by a bare `continue`**,
so it silently stays a hole rather than being reported as a trace this ledger
cannot read.

## Acceptance criteria

- [ ] `exhume` reports what it spent
- [ ] A second cairn is refused rather than chosen between
- [ ] Answering nothing says so rather than saying "sealed"
- [ ] A hole the ledger does not hold is reported
