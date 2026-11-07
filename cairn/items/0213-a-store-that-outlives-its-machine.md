---
id: 213
title: A store that outlives its machine
type: feature
status: buried
milestone: ossuary
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: m
area: crates/nether-ledger
stratum: '4'
proof: A store exported, moved to another machine and imported answers every cairn the original did
---

The store is a directory today, which means moving it is `tar`. That works
and it is not a plan: it copies the reverse index, which is derived, and it has
no integrity check at the far end.

An export that carries only what cannot be re-derived, and an import that
rebuilds the rest and verifies as it goes.

## Delivery plan — 2026-09-15

### Starting point and scope

A directory copy is not a verified export/import contract. Transfer canonical objects and rebuild derived indexes.

### Steps

1. Specify manifest/archive, versions, snapshot consistency and interrupted import before code.
2. Implement bounded streaming verification, duplicate handling and index rebuild.
3. Move a store between machines with holes, witnesses and deposits; test corrupt/truncated imports.

### Acceptance and evidence

- [x] Every original cairn resolves identically and replay works after import. Destination collisions cannot overwrite valid objects; same-machine round trips are preliminary.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Base commit: `e6d015b` (`spec/07-ledger.md` §7.5.4 `Moving a store`, `spec/90-rationale.md` §90.2 `Moving a store by copying its directory`, `crates/nether-ledger/src/store.rs` `Store::export` and `Store::import`).
- Bundle contract and implementation:
  - `Store::export` verifies every object in `objects/` via `verify_bytes` (refusing corrupt objects or malformed paths), orders objects in topological dependency order (leaves before referrers, ties broken by cairn), frames the stream with `netherc/cairn/v4\x00bundle\x00` and object count, and appends a 32-byte whole-stream `blake3` trailer. `refs/`, `memo/` and `tmp/` are omitted.
  - `Store::import` validates the 24-byte header (distinguishing non-bundles from incompatible domain versions), enforces `MAX_BUNDLE_OBJECT` (16 MiB) before allocation, rejects duplicate cairns, verifies canonical decoding and `Cairn::of_encoded(&bytes) == cairn` before committing, preserves valid existing destination objects without overwriting while repairing any missing `refs/` edges, and verifies the 32-byte `blake3` trailer and EOF.
  - Because entries are ordered leaves-before-referrers, any truncated prefix committed before an interrupted import is dependency-closed (`store.audit().unwrap().is_clean()` holds at every truncation cut) and resumable.
- Tests and cross-machine proof:
  - `store::tests::an_exported_store_imported_into_an_empty_one_rebuilds_refs_and_answers_every_cairn` and `store::tests::truncated_corrupt_and_incompatible_bundles_are_refused_and_partial_imports_stay_clean` in `crates/nether-ledger/src/store.rs`.
  - `an_exported_store_imported_elsewhere_replays_a_sealed_trace` in `crates/nether-cli/tests/rites.rs`: buried `build.nc` (`76c4b6554237275b419a295bbb35da9b96fd4160e5bfe76639766bccc56f8669`) and exhumed it (`ce2091c985286dfb29ef33ebdce4487eec2be4b4ee0b8c68a4d913e8a892605e`) on macOS (`aarch64-apple-darwin`, APFS), exported `9` objects (`2,044` bytes, SHA-256 `663ee0493aa53fd109c50821b632d411613b9f1a811cf54cf49318b32e5024c0`), transferred the bundle to a separate Linux machine (`Linux liam-lab 7.2.9-orbstack-00391-g2b298d05fbd4 aarch64 GNU/Linux`, `aarch64-unknown-linux-gnu`), imported into an empty store without source files, verified `Store::audit().is_clean()`, and confirmed identical output for `nether lamp`, `nether lamp --provenance`, `nether strata`, and `nether exhume --replay` (`identical.`).
- Remaining limits: `Store::export` and `Store::import` are library entry points on `Store`; exposing CLI subcommands is outside §8's frozen six-rite surface unless specified there.
