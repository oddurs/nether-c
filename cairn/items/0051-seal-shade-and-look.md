---
id: 51
title: seal, shade and look
type: feature
status: unmarked
milestone: calculus
depends_on:
- 14
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: crates/nether-core
stratum: '0'
proof: The Orpheus rule behaves exactly as decided in the codex, including the error text
---

Implements whichever form of the Orpheus rule the spec settled on. If the
error messages are unreadable in practice, that is evidence against the rule
and it goes back to the spec rather than getting patched here.
