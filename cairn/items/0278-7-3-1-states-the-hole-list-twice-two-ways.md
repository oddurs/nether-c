---
id: 278
title: §7.3.1 states the hole list twice, two ways
type: spec
status: buried
created: 2026-10-30
updated: 2026-10-30
priority: p1
effort: s
area: spec/07-ledger.md
proof: §7.3.1 states what each of the Trace's lists holds exactly once, and what it says of the hole list agrees with §6.3
---

## What this section must answer

What a `Trace`'s hole list holds: one entry per hole, or one entry per
question left open.

## Constraints it inherits

§6.3 merges identical holes at the read strata and never at 4, 6, 7 and 8, so
two acts written in one place are two entries naming one node.

## Open questions

- [x] Which of the two statements is right? The first, which #195 wrote. The
      second is what it replaced, left beside it by a merge.

## Delivery steps and dependencies

1. Delete the stale paragraph pair. Nothing else states it.

## Acceptance criteria

- [x] Every normative claim is stated once, in one place
- [x] Every code sample in it is in `tests/transcripts/`
- [x] A reader who has not read the rest of the spec can follow it

## 2026-10-30

§7.3.1 said what a `Trace`'s lists hold twice: the deposit-list paragraph
verbatim both times, and the hole list once as *one entry per question the
burial left open* and once as *names each hole once*. Those disagree at
strata 4, 6, 7 and 8, where §6.3 never merges and two acts from one place are
two entries naming one node. `git log -S` shows #195 wrote the first while
fixing `bury` to match it, and the second is the text it replaced. Found while
looking for where §8.4 should say the deposit list's order (0274). Buried by
deleting the stale pair.
