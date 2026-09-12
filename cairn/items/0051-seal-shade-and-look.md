---
id: 51
title: seal, shade and look
type: feature
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 14
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-core
stratum: '0'
proof: The Orpheus rule behaves exactly as decided in the codex, including the error text
---

Implements whichever form of the Orpheus rule the spec settled on. If the
error messages are unreadable in practice, that is evidence against the rule
and it goes back to the spec rather than getting patched here.

## 2026-09-12

The error text is not copied into the test. It is read out of section 1.6's fenced block and compared character for character, so the specification is the fixture and a change on either side fails the build. That is what made the column and the fetch/get mistakes worth fixing first — an error text a specification prints is one an implementation owes.

## 2026-09-12

Blame is a walk that only ever runs on the way to an error, so it can be as slow as it likes. From the look it follows the operand back to what bound it and finds the call whose latent depth is the origin, which is how the error blames a get three lines above the line it underlines. Section 2.4 promises this is findable by construction; this is the construction.

## 2026-09-12

The two rejected forms are tested as rejected: a legal look leaves the ambient depth where it found it, so a read after it is still ungranted, and an illegal look is one fault and leaves the binding a depth-0 shade that still seals to Cairn@0.
