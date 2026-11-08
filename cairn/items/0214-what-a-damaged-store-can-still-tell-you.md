---
id: 214
uid: f7e9775b-c60b-462b-840e-ced21eeb7756
title: What a damaged store can still tell you
type: feature
status: buried
milestone: ossuary
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: m
area: crates/nether-ledger
stratum: '3'
proof: A store with N corrupted objects reports exactly those N, serves the rest, and says which traces are affected
---

`get` already catches a tampered object, one at a time, when something asks
for it. There is no way to ask the store how it is.

An fsck that walks every object, verifies its cairn, and — the part that
matters — reports which *traces* are now incomplete. A corrupt object is a fact
about bytes; a broken provenance chain is a fact somebody has to act on.

## Delivery plan — 2026-09-15

### Starting point and scope

Store::get already detects tampering. The missing feature is a read-only whole-store audit and affected-trace reporting.

### Steps

1. Specify corrupt objects, missing references, malformed filenames and derived-index errors as distinct classes.
2. Verify objects independently of reverse indexes; derive affected traces from verified forward edges, labelling unknowable reachability.
3. Test N corrupt objects, healthy traces, broken indexes and absent children.

### Acceptance and evidence

- [x] Exactly N corrupt objects are distinguished from other errors; healthy objects remain readable. Do not claim complete dependency knowledge for undecodable objects; repair is out of scope.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-11-07

- `spec/07-ledger.md` §7.5.2 (`What a damaged store can still tell you`) and `spec/90-rationale.md` §90.2 (`Walking the reverse index to find damaged traces`) specify the four classes of store damage (`corrupt` objects, `missing` forward references, `malformed` filenames, and `index` reverse-index errors), require deriving affected traces from verified forward edges rather than `refs/`, and require marking a trace's reachability as `unknowable` when one of its node references (`holes`, `witnesses` or `deposits`) is corrupt or absent.
- `Store::audit`, `Audit`, `AffectedTrace` and `IndexError` in `crates/nether-ledger/src/store.rs` perform a read-only whole-store audit independently of `refs/` and `Seen`.
- Verified by `n_corrupt_objects_are_reported_and_affected_traces_are_named` and `broken_indexes_malformed_names_and_absent_children_are_distinguished` in `crates/nether-ledger/src/store.rs`.
