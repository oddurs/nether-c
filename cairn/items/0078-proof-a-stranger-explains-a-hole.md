---
id: 78
title: 'Proof: a stranger explains a hole'
type: chore
status: unmarked
milestone: necropolis
depends_on:
- 76
- 77
created: 2026-09-10
updated: 2026-09-15
priority: p0
effort: s
area: web/necropolis
proof: Five people who have not read the spec, five minutes each, three of them explain a hole correctly
---

The stage 5 proof, and the only one in the roadmap measured on humans rather
than on machines. If the idea cannot survive five minutes with a stranger, the
problem is the idea and not the explanation.

## Delivery plan — 2026-09-15

### Starting point and scope

Run the five-person comprehension study after 0076 and 0077. This is not the one-person navigation observation in 0075.

### Steps

1. Prepare one fixed example, a neutral task and a rubric: a hole is a world-question still owed an answer, not a crash.
2. Recruit five people who have not read the spec; allow five minutes each without coaching and obtain consent for anonymized notes.
3. Record the commit, task, each explanation and pass/fail; turn repeated confusion into linked items.

### Acceptance and evidence

- [ ] At least three of five explain a hole correctly under the recorded rubric. Participants, elapsed time and observations are required; automated browser tests cannot close this.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
