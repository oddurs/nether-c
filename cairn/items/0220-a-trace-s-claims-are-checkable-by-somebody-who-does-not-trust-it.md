---
id: 220
uid: 15fc1173-3891-4a8b-9355-4fdb9e2b9e50
title: A trace's claims are checkable by somebody who does not trust it
type: feature
status: buried
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: m
area: crates/nether-cli
stratum: '1'
proof: nether strata on a trace that under-reports its depth says so, and says it about a trace nobody trusts
---

A trace records its own depth and its own stratum-8 mark. A hostile one can
record whatever it likes.

`strata` already refuses a trace that under-reports — that landed in 0064 —
and the general principle should be stated and tested: every claim a trace
makes about itself is either derivable from its nodes or worthless. Which ones
are which belongs in section 08.

## Delivery plan — 2026-09-15

### Starting point and scope

Depth understatement already fails in crates/nether-cli/tests/rites.rs. Remaining work is the broader claim inventory in section 08.

### Steps

1. Classify depth, stratum-8 marking, completeness, witnesses and source/residue identity as derivable or externally trusted.
2. Specify absent/corrupt-node and untrusted-index behavior before extending CLI output.
3. Add hostile canonical fixtures for every derivable claim, retaining the existing regression.

### Acceptance and evidence

- [x] The CLI checks claims from verified objects and reports uncertainty. Do not redo 0064 or claim to prove that a world's recorded answer was true.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- `spec/08-rites.md` §8.6 classifies trace claims into three classes: derivable from verified objects in the store (`depth` hole/witness halves, `unrecorded` mark against reachable `Witness` nodes, `holes` completeness, and content-addressed `source`/`residue` checked when re-burying), bounded by missing objects vs refused on corrupt ones (`StoreError::Absent` is reported as a lower bound; `StoreError::Corrupt` exits 65), and externally trusted (whether a `Witness`'s `answer` came from the world rather than being invented by its writer).
- `crates/nether-cli/src/strata.rs` and `crates/nether-cli/src/exhume.rs` derive `unrecorded` from reachable `Witness` nodes at stratum 8, reject a trace with `unrecorded: false` over a stratum-8 witness with exit code 65 (`code::MALFORMED`) in `strata` and refuse it in `exhume --replay`, and fail with exit code 65 on `StoreError::Corrupt` during `survey` instead of counting corrupt nodes as `missing`.
- Trusted core reduced by 2 lines (`8741` -> `8739` in `.decay-ceiling`).
- Verified with `cargo test --package nether-cli --test rites` (`a_trace_that_hides_a_stratum_8_witness_is_malformed`, `a_corrupt_node_under_a_trace_is_malformed_and_not_merely_missing`, `a_poisoned_reverse_index_does_not_alter_what_strata_checks`).
