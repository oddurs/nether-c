---
id: 64
title: nether strata
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
- 47
- 135
created: 2026-09-10
updated: 2026-09-12
priority: p1
effort: m
area: crates/nether-cli
stratum: '1'
proof: Reports the deepest stratum a program reached and the exact line that took it there
---

Blame for depth. The answer to 'why is this thing @5' has to be a command,
not an investigation.

## What it reports

`nether strata <cairn>` takes a trace, walks the graph from its roots, and
collects every `Hole` and every `Witness` under it. §7.3 already gives each of
those a stratum and a span, so the rite reads the ledger rather than the
source, and works on a machine that has never seen the program.

```
depth 3   disk

  3  read("main.nc")           3df81ea7:3:35
  0  everything else

  replayable: yes
```

## Acceptance criteria

- [x] The deepest stratum, by the name §9.1 gives it
- [x] The call that took it there, and where it is, to the column
- [x] `replayable: no` and an unasked-for sentence when the trace reached 8
- [x] `--json`, one object and nothing else

## 2026-09-12

A hole and a witness are not the same depth. Section 7.3 says a witness holds 'the stratum that was reached' and a hole 'the stratum the call would reach', so the rite counts only witnesses toward the headline and reports the holes separately as what exhuming will cost. Without that a trace that has not run yet reads as though it had.

## 2026-09-12

The rite also catches a trace that under-reports its own depth: a witness deeper than the recorded depth means the trace has written down a falsehood about the one thing it is for, so that exits 65. Section 7.3's list of what makes a node malformed does not include it, which is worth deciding on separately.
