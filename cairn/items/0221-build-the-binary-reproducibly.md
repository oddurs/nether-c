---
id: 221
title: Build the binary reproducibly
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-15
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

## Delivery plan — 2026-09-15

### Starting point and scope

Reproducibility means identical targets and pinned inputs on independent machines, not identical macOS/Linux binaries.

### Steps

1. Record toolchain, target, linker/system inputs, lockfile and environment; find paths/timestamps/build IDs that vary.
2. Build the same commit independently twice and inspect complete binary differences before changing flags.
3. Expose the recipe through scripts/task and publish hashes and input versions.

### Acceptance and evidence

- [ ] Two machines yield byte-identical binaries for the declared target. Stripping differences afterwards does not establish identical original builds.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
