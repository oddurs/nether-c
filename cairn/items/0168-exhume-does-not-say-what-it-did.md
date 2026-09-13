---
id: 168
title: exhume does not say what it did
type: bug
status: buried
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

- [x] `exhume` reports what it spent
- [x] A second cairn is refused rather than chosen between
- [x] Answering nothing says so rather than saying "sealed"
- [x] A hole the ledger does not hold is reported

## 2026-09-13

questions() now returns Result<_, Cairn> rather than filter_map. It also hands back the span it already read, so the loop no longer re-reads each hole node to get one.

## 2026-09-13

The word is chosen from what happened, not printed unconditionally: sealed only when no holes are left, exhumed when some were answered and some remain, unchanged when nothing was answered -- and then there is no arrow, because there is nothing to draw one between. §6.6's transcript is unaffected: it is the sealed case.

## 2026-09-13

Fuel goes in --json only, exactly as bury does. §6.6's sealed line is a spec transcript the harness executes, and adding a field to it would change the specification from the compiler.
