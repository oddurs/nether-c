---
id: 107
title: Say what stability means
type: spec
status: unmarked
milestone: after
depends_on:
- 21
created: 2026-09-11
updated: 2026-09-11
priority: p1
effort: m
area: spec/
proof: A stated policy for what may change after 1.0, and what a trace buried under an older version is still entitled to claim
---

The specification currently says it is a draft and changes without notice,
which is honest today and useless the moment anybody depends on it.

A language whose central promise is that the past does not change needs an
unusually precise answer to 'what happens when the language changes'. A trace
buried under an older version is a fact about the past. Is it still readable?
Still replayable? Still *valid*?

Section 7.7 already takes the hard line for the ledger format: there is no
migration path and there is not meant to be one. Whether the language can
afford the same answer is a different question and needs deciding out loud.
