---
id: 221
title: Build the binary reproducibly
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-13
priority: p2
effort: m
area: .github/workflows
stratum: '4'
proof: Two machines build byte-identical nether binaries from one commit
---

A language whose entire proposition is that you can prove what produced an
artifact should be able to prove what produced its own compiler.

It is also the honest test of the idea: if reproducing a Rust binary is hard,
that is worth knowing before telling anybody else that reproducibility is easy
once you address by content.
