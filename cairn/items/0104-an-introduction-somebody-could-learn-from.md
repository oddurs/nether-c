---
id: 104
title: An introduction somebody could learn from
type: docs
status: descending
milestone: after
assignee: Oddur Sigurdsson
claimed: 2026-09-19
depends_on:
- 66
created: 2026-09-11
updated: 2026-09-19
priority: p1
effort: l
area: docs/
proof: Somebody who has never read the specification writes and buries a working program after one sitting
---

The specification is not a tutorial and should not become one. It answers
'what is the rule', and a newcomer needs 'what do I type'.

Separate document, different voice: worked examples in order, each introducing
exactly one idea, with the reader writing and burying real programs from the
first page. Depth arrives when it is needed, not in the first paragraph.

The proof is a person, not a page count.

## Delivery plan — 2026-09-15

### Starting point and scope

The spec answers rules; this item teaches a first successful burial. Reuse working CLI transcripts and the browser rather than duplicating normative prose.

### Steps

1. Write a short path: pure value, demand, hole, grant, witness, replay, linking each rule to the spec.
2. Make every command/example verifiable through tests/transcripts and state installation and supported-version prerequisites.
3. Observe a newcomer working through it without corrective coaching; record the commit, outcome and sticking points.

### Acceptance and evidence

- [ ] The newcomer writes and buries a working program in one sitting. Passing snippets is necessary but does not replace the human proof; leave the item open until observed.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-09-19

The document exists: docs/manual/, ten chapters, baked to site/manual/. Every `$ nether` line in it is executed by tests/transcripts/run against the binary, in a scratch store holding tests/programs/, so a chapter that drifts from the implementation turns CI red. The harness also refuses a Nether C sample whose first line names a fixture it does not byte-match, so a program printed in the prose is the program that ran.

The proof is not met and this stays descending. Steps 1 and 2 are done; step 3 is observing somebody who has not read the specification write and bury a program from this, without coaching. Nothing in a repository can produce that.

Writing it found two bugs, 0273 and 0274, both because documenting a rite truthfully means running it.
