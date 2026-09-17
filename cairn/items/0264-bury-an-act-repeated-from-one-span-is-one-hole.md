---
id: 264
title: 'bury: an act repeated from one span is one hole'
type: bug
status: unmarked
milestone: codex
created: 2026-09-17
updated: 2026-09-17
priority: p1
effort: m
stratum: 0
area: crates/nether-bury
proof: A loop that posts four times leaves four holes
part_of: [255]
---

## What happens

```c
U0 send() @6 { for (I64 i = 0; i < 4; i += 1) { post("http://h/x", b"y"); } }
demand descend net! { send() };
```

```
buried   acts.nc → 0f03be86   depth 0   holes 1   5 nodes
  hole ①  post("http://h/x", b"y")       stratum 6  net!
```

Four messages, one hole. 0255 removed the merge that did this across two
spans and `spec/06-evaluation.md` §6.3 now says holes at 4, 6, 7 and 8 are
never merged — but a `Hole` node is named by its content, and four asks from
one span have one call, one stratum and one span, so they are one node.
`dig` pushes the cairn once and the interning it stopped doing is not what is
merging them.

## What should happen

Four entries. §6.3's reason does not care whether the second ask was written
on the same line as the first: two `post`s are two messages, and a trace that
describes fewer acts than the program performed is a trace that lies about
what was done.

0261 settled exactly this for deposits and went the other way round — the
`Deposit` node stays one node and the trace's list names it four times, which
is what `spec/07-ledger.md` §7.3.1 now says. The same shape should work here:
one `Hole` node, four entries in the hole list.

## Reproduction

1. The program above, `nether bury`.

## Cairn of the offending trace

`0f03be86`, from the local run above. Not in any shared ledger.

## Fix boundary and regression proof

- §7.3.1 says the hole list names each hole once. That sentence is 0261's and
  it is the one this contradicts, so it changes here or this item is wrong.
- `Burial::dig` pushes one entry per ask at an act stratum, and the `fresh`
  check that 0255 added stays for reads.
- Exhumation asks the world once per entry, and §6.3's ordered answers already
  serve the nth ask the nth answer — so this may be the only place that moves.
- A test beside `every_act_is_its_own_hole_and_every_read_is_shared` in
  `crates/nether-bury/tests/holes.rs`, over a loop rather than two statements.
