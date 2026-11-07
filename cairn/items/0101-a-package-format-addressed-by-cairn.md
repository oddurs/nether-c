---
id: 101
title: A package format, addressed by cairn
type: feature
status: buried
milestone: after
assignee: Oddur Sigurdsson
claimed: 2026-11-07
closed_at: 2026-11-07
depends_on:
- 215
created: 2026-09-11
updated: 2026-11-07
priority: p1
effort: xl
area: crates/nether-ledger
stratum: '1'
proof: Two machines that have never spoken resolve the same dependency to the same cairn, offline
---

## Problem

The roadmap ends where the compiler works, which is not where a language starts
mattering. Nobody adopts a language they cannot share code in.

## Proposal

There is no registry and there does not need to be one. A package is a cairn. A
dependency is a cairn. Resolution is a lookup in a content-addressed store, and
two people who have never spoken resolve the same name to the same bytes
because the name *is* the bytes.

This is the part of the design that was free all along and it should be claimed
deliberately rather than discovered.

## Open questions

- [x] How does a human name a package they have not seen? Cairns are not memorable
- [x] Version ranges have no meaning when a dependency is a hash. Is that a feature?
- [x] Who hosts the store, and what happens when they stop (see 'A public store')

## Delivery plan — 2026-09-15

### Starting point and scope

A cairn establishes byte identity, not availability, trust or a human name. 0215 specifies exchange; offline resolution requires dependencies already present.

### Steps

1. Specify package roots, dependency closure, entrypoints and compatibility against spec/07-ledger.md before implementation.
2. Decide explicit aliases and exact-cairn references; keep version-range solving and a central registry out of the first format.
3. Build offline resolution with missing, corrupt and incompatible dependency diagnostics; use independently populated stores.

### Acceptance and evidence

- [x] Two disconnected machines with the same verified dependency closure resolve identical cairns. Missing bytes fail explicitly; a hash alone cannot fetch a package.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-07

Specified canonical `"Package"` and `"Dep"` struct values, local aliases with exact 32-byte dependency cairns, `netherc/cairn/v4` compatibility checking, and topological post-order offline closure resolution in `spec/07-ledger.md` §7.5.5 and `spec/90-rationale.md` §90.2 (PR #249, `b2b3dbd`). Implemented `PACKAGE_DOMAIN`, `Package`, `ResolvedPackage`, and `PackageError` in `crates/nether-ledger` (stratum 1).

Verified with `cargo test -p nether-ledger` (`two_disconnected_stores_resolve_the_same_package_closure_to_identical_cairns_offline` and `missing_corrupt_incompatible_and_malformed_packages_fail_explicitly`) and `cargo test -p nether-cli --test rites two_disconnected_machines_resolve_and_bury_a_package_closure_to_the_same_cairn`: two disconnected machines with independently populated stores resolve a diamond dependency closure to identical package cairns, alias bindings, topological closure order, and combined source cairn offline, and bury the resolved unit to the same trace cairn; missing objects fail with `StoreError::Absent`, corrupt objects fail with `StoreError::Corrupt`, incompatible domain strings fail with `PackageError::Incompatible`, and malformed or unsorted dependencies fail with `PackageError::Malformed`. Core line count rose from 9314 to 9452 (+138 lines in `crates/nether-ledger`). Remaining limits: offline resolution requires the dependency closure to be present in the local store beforehand via §7.5.3 or §7.5.4; entrypoints share a single top-level declaration namespace when concatenated in topological order.
