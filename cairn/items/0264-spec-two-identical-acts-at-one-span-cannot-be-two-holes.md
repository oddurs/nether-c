---
id: 264
title: 'Spec: two identical acts at one span cannot be two holes'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: m
stratum: '0'
area: spec/07-ledger.md
proof: A loop that posts twice leaves a trace recording two sends
---

## What §6.3 cannot say

[§6.3](../spec/06-evaluation.md) now refuses to merge a hole at stratum 4, 6, 7
or 8: two sends are two acts, and a trace that recorded one would record fewer
acts than the program performed.

Two spans is how they stay apart. One span is not:

```c
for (I64 i = 0; i < 2; i += 1) { post("http://h/pay", b"{}"); }
```

The condition waits on nothing, so the loop unrolls, and the two calls have the
same `call` and the same `span`. [§7.3.1](../spec/07-ledger.md#731-node-encoding)
gives a `Hole` exactly those two fields and a stratum, so the two are one node
— named by content, and nothing in the content differs.

So §6.3's rule holds for the case it names and not for the case a loop makes,
and the trace says one payment where the program made two.

## What it would take

A `Hole` needs something that distinguishes two asks the program made
separately. The obvious field is an ordinal: which ask this was, in the order
[§6.2](../spec/06-evaluation.md#62-demand) fixes. That order is deterministic,
so the field is a fact about the program rather than about the implementation.

The cost is the reason this is its own item rather than a line in 0255.
[§7.3.1](../spec/07-ledger.md#731-node-encoding) is **frozen**, and adding a
field to `Hole` changes what the `0x02` kind byte means, which bumps the domain
separator ([§7.2](../spec/07-ledger.md#72-cairns)) and renames every value ever
stored. §7.2 records that the domain has moved three times and that each was
affordable because nothing had been buried; that is still true today and will
not be true forever.

An ordinal also has to be decided carefully. It is a field on a node whose
whole point is to be identical wherever the same question is asked, so it must
be present only where merging is refused — or it defeats the merging half of
§6.3, and two reads of one file become two holes again.

## The alternative

Say a loop that means two sends must write two. That is what the specification
says today by accident, and it is not defensible: `for` is in the grammar, and
a language that cannot send twice from a loop has a loop that does not work at
stratum 6.

## Acceptance criteria

- [ ] §6.3 or §7.3 says what distinguishes two acts the program made separately
- [ ] A loop that posts twice leaves a trace that records two sends
- [ ] Two identical `read` calls, in a loop or not, still leave one hole
- [ ] If the node encoding changes, §7.2 records the bump and why it was affordable
- [ ] §90.2 records the alternative
