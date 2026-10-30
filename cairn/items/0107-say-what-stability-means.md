---
id: 107
title: Say what stability means
type: spec
status: buried
milestone: after
assignee: Oddur Sigurdsson
claimed: 2026-10-30
depends_on:
- 21
created: 2026-09-11
updated: 2026-10-30
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

## Delivery plan — 2026-09-15

### Starting point and scope

The ledger's format promise and the language's draft status are separate. State policy before promising compatibility or locking constructors in 0115.

### Steps

1. Inventory current version/domain separators and what old traces require to remain readable, verifiable and replayable.
2. Write a compatibility matrix for syntax, canonical encoding, prelude and world/replay semantics, including unsupported versions.
3. Add illustrative old/new fixtures and document rejected migration options in the rationale.

### Acceptance and evidence

- [x] A reader can determine the promised treatment of an old trace after a release. The policy names limits and breaking-change procedure; it does not claim 1.0 or compatibility already tested.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Buried. §0.6.1 states it once. A trace written by an older release is readable and verifiable exactly when its domain separator is this implementation's (§7.7); replayable by the release that wrote it and by any later one whose burial of that residue is unchanged; and true always, because it says what that implementation did. Replay is the claim a language change can take away, since a residue is source and replay buries it again, so the promise is that the writing release can always be had again: release binaries are built reproducibly and hashed (scripts/task release:build, 0221). After 1.0, semver by layer: any encoding change moves the domain and is major; a change that makes a program bury differently or stop lowering is major, one that only lets more lower is minor; adding a prelude function is minor, changing one's signature, stratum or answers is major; a rite flag that changes what is written is major, one that changes only what is printed is minor, with --json counted as written. A major release names in its changelog which of the four claims its older traces lose. §90.2 records the rejected stronger promise, a compatibility mode in every later release. Limits: no illustrative old-domain fixtures were added; §7.7's refusal of a foreign domain is already the implemented behaviour, and before 1.0 none of this binds.
