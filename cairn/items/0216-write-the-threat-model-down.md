---
id: 216
title: Write the threat model down
type: docs
status: unmarked
milestone: warden
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: m
area: SECURITY.md
stratum: '0'
proof: Every trust boundary in the system is named, with what crosses it and what is assumed about the other side
---

SECURITY.md names four things worth attacking and calls the decoder the one
genuinely hostile surface. That was true when the decoder was all there was.

There are now eight crates, a wasm build, a store meant to be shared, and
foreign code at stratum 8. Somebody should say, in one place, who is trusted
and who is not: the author of a program, the author of a trace, the owner of a
store, the machine a burial runs on, and whoever wrote the shared object
stratum 8 dlopens.

## Delivery plan — 2026-09-15

### Starting point and scope

SECURITY.md still says nothing is implemented. Eight crates, WASM workers, shared stores and foreign code now exist; this is the first recommended security task.

### Steps

1. Inventory source/trace authors, providers, store writers, browser transport, build inputs and foreign libraries.
2. For each boundary, name crossing data, verification, limits, trusted components and excluded guarantees; link tests and gaps.
3. Correct stale implementation language while preserving the reporting channel and avoiding unsupported release promises.

### Acceptance and evidence

- [ ] Every real boundary has a threat and defense or linked gap. Distinguish integrity from availability and recorded replay from trusting foreign code.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
