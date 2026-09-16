---
id: 261
title: 'Spec: nothing says whether a deposit made twice is recorded twice'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
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

- [ ] §7.3.1 says whether the deposit list may repeat a cairn
- [ ] §8.4 says what `lamp` prints for a repeated deposit
- [ ] A transcript covers the loop above
- [ ] §90.2 records the reading that was not taken
