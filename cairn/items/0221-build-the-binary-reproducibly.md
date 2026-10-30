---
id: 221
title: Build the binary reproducibly
type: chore
status: descending
milestone: warden
assignee: Oddur Sigurdsson
claimed: 2026-10-30
depends_on:
- 216
created: 2026-09-13
updated: 2026-10-30
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

## 2026-10-30

Recipe landed, proof pending a release. Built twice from one commit, two release binaries differed only in where each machine keeps the dependencies' sources, which their panic locations carry. scripts/task release:build <target> remaps CARGO_HOME to /cargo and the checkout to /nether and builds --locked; on this machine it gave 646233976f26b8d41507823189bb4e69d8bf1ba46b9eb8d86fdb839bd6d76af3 from two checkouts at different paths with different CARGO_HOMEs, NETHER_VERSION=9.9.9, x86_64-unknown-linux-gnu, rustc 1.98.1 per rust-toolchain.toml; unversioned it gave 296e5ebe...f63 from both. That is one machine. The release workflow now builds through the verb, and a reproduce job on a second runner, with no cache from the first, rebuilds the Linux binary and fails the release if its hash differs from the shipped one; publish waits for it. Bury this on the first tag whose reproduce job passes, with its run and both hashes. Limits: Linux only, since the macOS targets are built on runners of a different kind and comparing them would need a second macOS job.
