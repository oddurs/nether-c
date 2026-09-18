---
id: 264
title: 'bury: an act repeated from one span is one hole'
type: bug
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-17
updated: 2026-09-17
priority: p1
effort: m
stratum: 0
area: crates/nether-bury
proof: A loop that posts four times leaves four holes
part_of:
- 255
---

## What happens

```c
U0 send() @6 { post("http://h/x", b"y"); }
demand descend net! { send() };
demand descend net! { send() };
```

```
buried   twice.nc → 1bdd329d   depth 0   holes 1   5 nodes
  hole ①  post("http://h/x", b"y")       stratum 6  net!
```

Two messages, one hole. 0255 removed the merge that did this across two spans
and `spec/06-evaluation.md` §6.3 now says holes at 4, 6, 7 and 8 are never
merged — but a function body is one span, so two calls to it ask from the same
place. Same call, same stratum, same span, so one `Hole` node, and `dig` pushed
its cairn once.

> A loop was the case this item was first written against and it does not
> reproduce: `for (…) { post(…); }` starves on the first turn, the unrolling
> is abandoned and the loop residualises whole, so the burial asks once and one
> hole is right. Two call sites into one body is the case that reaches `dig`
> twice.

## What should happen

Two entries. §6.3's reason does not care whether the second ask was written
where the first one was: two `post`s are two messages, and a trace that
describes fewer acts than the program performed is a trace that lies about
what was done.

0261 settled exactly this for deposits and went the other way round — the
`Deposit` node stays one node and the trace's list names it four times, which
is what `spec/07-ledger.md` §7.3.1 now says. The same shape should work here:
one `Hole` node, four entries in the hole list.

## Reproduction

1. The program above, `nether bury`. One hole before, two after.

## Cairn of the offending trace

`1bdd329d`, from the local run above. Not in any shared ledger.

## Fix boundary and regression proof

- §7.3.1 says the hole list names each hole once. That sentence is 0261's and
  it is the one this contradicts, so it changes here or this item is wrong.
- `Burial::dig` pushes one entry per ask at an act stratum, and the `fresh`
  check that 0255 added stays for reads.
- Exhumation asks the world once per entry, and §6.3's ordered answers already
  serve the nth ask the nth answer — so this may be the only place that moves.
- A test beside `every_act_is_its_own_hole_and_every_read_is_shared` in
  `crates/nether-bury/tests/holes.rs`, over one body reached from two calls.

## 2026-09-17

Fixed, and the item's premise was wrong. The loop it was written against does not reproduce: a body that starves on the first turn has its unrolling abandoned and the loop residualises whole, so the burial asks once and one hole is right. The case that reaches dig twice is one function body reached from two call sites -- the span is inside the body, so both asks carry the same call, stratum and span and therefore one Hole node. On main that trace says one hole; it now says two, naming that one node twice. dig pushes one entry per ask and the read path is unchanged, because the interning above it returns the first hole before the push is reached; the fresh guard 0255 added is gone with it, since it only ever protected the act case. spec/07-ledger.md §7.3.1's hole sentence is rewritten to match and now reads as one rule with the deposit sentence beside it: one entry per question left open, two acts written in one place being two entries naming one node. crates/nether-bury/tests/holes.rs pins both halves -- two calls into one sending body giving two entries and one cairn, and two calls into one reading body giving one. scripts/task check passes in full; the ceiling did not move.
