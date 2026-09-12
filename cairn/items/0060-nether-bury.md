---
id: 60
title: nether bury
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
- 49
- 128
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-cli
stratum: '4'
proof: Reports depth, hole count and node count on every burial, and writes exactly one trace
---

The primary verb. Everything a burial learns goes into the ledger; the
terminal output is a summary of what was written, never the thing itself.

## 2026-09-12

Blocked on 0128 rather than on anything here. Every part of the pipeline exists — lex, parse, lower, check, bury, and a store — and what is missing is what a trace's roots are. Section 7.3 says a Trace holds what was demanded, and a demand that did not reduce has no node to point at, because section 6.5 calls the residue nodes and section 7.3 has no node that can hold one. Section 6.6's own summary line counts 903 nodes for a program with one hole, which is the residue being counted, so the two readings cannot both be right and the number in the specification assumes the one section 7.3 cannot express. Inventing a trace identity here would be implementing something the specification does not describe.

## 2026-09-12

Done. The pipeline runs end to end: read, parse, lower, check, bury, write, report.

    $ nether bury tests/programs/build.nc
    buried   build.nc → 0380c8ae   depth 3   holes 1   4 nodes
      hole ①  read("main.nc")                stratum 3  disk

which is 8.2's own transcript against 8.2's own program. Seven CLI tests, including that two burials of one source name the same trace.

Three things worth recording.

The spec said 903 nodes for that program and the real number is 4. 903 was the residue being counted as nodes, which is the reading 0128 rejected — a trace NAMES its residue and does not contain it. Both examples updated to the real output, with a note in 6.5 saying why the number moved.

8.2 claims a trace buried under a different budget is a different trace. It cannot be: running out of fuel produces a Halt and no trace at all, so a budget either lets a burial finish or there is nothing to compare. The test that asserted 8.2 directly failed, and the contradiction is filed rather than worked around.

deposits is an empty list, honestly. Burial does not produce Node::Deposit yet, which means hello.nc buries but lamp shows nothing — the front page's own example does not work end to end. Filed as a bug; it is the last thing between the implementation and the first program in the specification.
