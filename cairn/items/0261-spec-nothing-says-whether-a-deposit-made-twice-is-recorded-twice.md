---
id: 261
title: 'Spec: nothing says whether a deposit made twice is recorded twice'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-17
priority: p2
effort: s
stratum: '0'
area: spec/07-ledger.md
proof: Two implementations agree on the cairn of a trace that deposits in a loop
---

## The ambiguity

`spec/07-ledger.md` §7.3.1: a `Deposit` is "`cairn` of the value, `span`", and
a `Trace` holds a "`cairn-list` of deposits" — "`u64` count, then that many
thirty-two byte cairns".

A deposit made a thousand times from one span is one *node*: same value, same
span, same cairn. Whether the trace's list holds that cairn once or a thousand
times is not stated, and both are well-formed under
[§7.1.1](../spec/07-ledger.md#711-what-a-decoder-must-reject).

```c
for (I64 i = 0; i < 1000; i += 1) { "tick\n"; }
```

## Why it is not cosmetic

The list is part of the `Trace`'s encoding, so the two readings give the trace
two different cairns. Two implementations that disagree here disagree about the
name of the artifact — which is the thing §7.1 is frozen to prevent — and
`nether lamp` prints the line once on one of them and a thousand times on the
other.

§8.4 orders deposits "by offset within one source". That is an order over a
set and says nothing about multiplicity.

## The choice

**A list, with repeats.** `lamp` prints what the program deposited, as many
times as it deposited it, which is what anybody reading a program's output
expects. The cost is a trace that grows with a loop's trip count.

**A set, ordered by span.** The deposits are the trace's record of which values
were deposited and where, and a repeat adds nothing a reader did not already
have. Cheap — and it means a program cannot deposit the same value from the
same span twice, which is a real surprise in a language whose only output
channel this is.

Neither is obviously right. Both cost one sentence in §7.3.1 and one in §8.4,
and leaving it costs a reproducibility claim.

## Acceptance criteria

- [x] §7.3.1 says whether the deposit list may repeat a cairn
- [x] §8.4 says what `lamp` prints for a repeated deposit
- [x] A transcript covers the loop above
- [x] §90.2 records the reading that was not taken

## 2026-09-17

Settled as a list, with repeats, and the implementation had already chosen both readings -- which is what decided it. Burial recorded four deposits for the four-turn loop and nether lamp printed one tick, because lamp walked the graph and a Deposit made four times from one span is one node. §7.3.1 now states the rule for both of a trace's lists in one place: the deposit list holds one entry per deposit and may name the same cairn more than once, because the node is what the program said and the list is what it did; the hole list names each hole once, because a question asked twice from one place is one question. §8.4 says lamp renders one line per entry and shows it. §90.2 records the set reading and why it loses -- output that swallows repeats is not output, and it is 0255's argument about sends read again -- and prices the choice at thirty-two bytes per turn in one list, not a node per turn. crates/nether-cli/src/lamp.rs reads the trace's list instead of the graph; tests/programs/tick.nc is the fixture and spec/08-rites.md § 8.4 lamp #3 is an executed transcript over it. Taking only the first entry makes that transcript fail with one tick against four, so it discriminates. Trusted core 8464 -> 8473. scripts/task check passes in full. The hole half of §7.3.1 is true of the implementation for two statements and not for a loop: four posts from one span are still one hole, which is 0264.
