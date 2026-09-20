---
id: 274
title: A comment changes what lamp prints first
type: spec
status: unmarked
created: 2026-09-19
updated: 2026-09-19
priority: p1
effort: m
area: spec/08-rites.md
stratum: '1'
proof: Adding a comment above a program does not change the order `nether lamp` renders its deposits in
---

## What this section must answer

## Constraints it inherits

## Open questions

- [ ]

## Delivery steps and dependencies

1. State the prerequisite decisions and the smallest reviewable specification change.
2. Name the implementation/conformance items that discharge the contract later.

## Acceptance criteria

- [ ] Every normative claim is stated once, in one place
- [ ] Every code sample in it is in `tests/transcripts/`
- [ ] A reader who has not read the rest of the spec can follow it

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
