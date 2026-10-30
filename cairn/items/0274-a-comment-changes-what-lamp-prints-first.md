---
id: 274
title: A comment changes what lamp prints first
type: spec
status: buried
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-09-19
updated: 2026-10-30
priority: p1
effort: m
area: spec/08-rites.md
stratum: '1'
proof: Adding a comment above a program does not change the order `nether lamp` renders its deposits in
---

## What this section must answer

## Constraints it inherits

## Open questions

- [x] Where is the order stated? §8.4, which says the list is the order the
      deposits were made in and that `lamp` MUST NOT reorder it.

## Delivery steps and dependencies

1. State the prerequisite decisions and the smallest reviewable specification change.
2. Name the implementation/conformance items that discharge the contract later.

## Acceptance criteria

- [x] Every normative claim is stated once, in one place
- [x] Every code sample in it is in `tests/transcripts/`
- [x] A reader who has not read the rest of the spec can follow it

§8.4 groups deposits by the cairn of the source they came from and says which
group comes first is decided by sorting those cairns, "because a source has no
other order: one trace's two sources were never written down in a sequence".

They were. A sealed trace's deposit list is built by `closing::deposits`, which
is §6.8's order: what the burial deposited, then what burying its residue
deposited. That is a sequence, it is written down, and it is right. `lamp` then
sorts it away:

```rust
found.sort_by_key(|(source, at, _)| (*source, *at));
```

The sort key begins with 32 bytes of hash, so which half of a program's output
comes first is decided by a digest. Add a comment to the top of the file and
the source cairn changes and the output reorders. Two spellings of one program,
same deposits, same residue, different reading:

```console
$ nether lamp f624023d
world
Hello,
$ nether lamp 18ee3b82
Hello, world
```

`18ee3b82` is that program without its comment header. Nothing else differs.

Sorting by offset within a group is the same mistake one level down: a function
called from two places deposits in call order, and its deposits share a span.
`Burial::deposits` is pushed in evaluation order and is already correct.

The fix is to delete the sort and render the list the trace holds, and to
rewrite §8.4's paragraph to say deposits are read in the order they were made —
which is what the section's own first sentence, "which is how a program's
output is read", already claims. Removing it lowers `.decay-ceiling`.

§8.4's rejected alternative — sorting, and why it looked right — goes in
`spec/90-rationale.md` with the change.

Found while writing the manual (0104), which needed a worked example whose
output does not depend on a hash.

## 2026-10-30

Buried. lamp reads the trace's deposit list in the order it holds and no longer sorts it, and no longer walks the graph either: only the root trace's own list ever contributed. §8.4 says the list is the order the deposits were made in and lamp MUST NOT reorder it; §90.2 records sorting by source and offset as the rejected alternative. The mechanism was not quite the one described above: exhume re-parses the residue but tags its spans with the original source's cairn, so the sort compared an offset in the source with an offset in the residue, and a comment long enough moved the first past the second. a_comment_does_not_change_what_lamp_prints_first in crates/nether-cli/tests/rites.rs fails on the old code (worldHello,) and passes now. post.nc lost the paragraph that described this bug as current, so manual chapter 6's cairns moved. closing::deposits cited §6.8 for the order, which says nothing about it, and now cites §8.4. The core is 13 lines smaller.
