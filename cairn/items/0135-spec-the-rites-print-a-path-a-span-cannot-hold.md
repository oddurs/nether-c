---
id: 135
title: 'Spec: the rites print a path a span cannot hold'
type: spec
status: buried
milestone: rites
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/08-rites.md
proof: No example in section 08 renders a stored span as a file path
---

## The mismatch

`spec/07-ledger.md` §7.3 fixes a span as the `cairn` of the source, a start
and an end, and says why:

> A path is a fact about one machine at one moment; the trace has to mean the
> same thing on a machine that has never seen that filesystem.

`spec/08-rites.md` then renders a stored span twice as a path:

```
witness  read("main.nc")            stratum 3   build.nc:1:15
```

There is no path in the node, so no implementation can print that line. The
two examples that show `lamp --provenance` and `strata` working show them
doing something the ledger cannot do.

## What this must decide

The position is right and the name is wrong. A rite reports a span against the
cairn of its source, and §8.1 says so once for every rite rather than twice by
example.

§01 and §06 keep their paths. Those are burial diagnostics, printed from a
file the checker is holding open, and a path is the right name for a file that
is right there.

## Acceptance criteria

- [x] No example in §08 renders a stored span as a file path
- [x] The rule is stated once, in §8.1, and links to §7.3

## 2026-09-12

Found while building nether strata: the rite has to print the span of the deepest call, and the span it is handed holds a cairn where the example holds a filename. Fixed by changing the examples, because section 7.3 already gave the reason the node holds no path.
