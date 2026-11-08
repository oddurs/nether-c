---
id: 227
uid: 99161b99-c504-4863-9ba3-8270d7a1e3a9
title: Say what the proofs are worth
type: docs
status: buried
milestone: assay
depends_on:
- 223
- 224
- 225
- 226
assignee: Oddur Sigurdsson
claimed: 2026-11-07
created: 2026-09-13
updated: 2026-11-07
priority: p2
effort: s
area: spec/90-rationale.md
stratum: '0'
proof: Section 90 states what is proved, what is tested, and what is merely believed, with no overlap between the three
---

A project that mechanizes half its metatheory and tests the other half owes a
reader a clear account of which is which.

The temptation once there are proofs is to let the word cover everything
nearby. The honest version names the three tiers and puts each claim in exactly
one.

## Delivery plan — 2026-09-15

### Starting point and scope

Build the evidence ledger after 0222–0226, keeping failed/deferred work visible and model claims distinct from implementation claims.

### Steps

1. Inventory normative claims with theorem identifiers, test paths or explicit unproved status.
2. Record strongest evidence, assumptions, supported subset and reproduction command, allowing supporting lower-tier tests.
3. Update spec/90-rationale.md without duplicating rules or calling differential testing a proof.

### Acceptance and evidence

- [x] Each claim has one primary evidence class and traceable support. No unfinished theorem, observation or long measurement is described as complete.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-11-07

- Added `spec/90-rationale.md` §90.3 (*What is proved, what is tested, and what is believed*), partitioning the specification's claims into three mutually exclusive primary tiers with zero overlap:
  1. **Tier 1 — Proved (in the `proof/` LCF model, reproduced by `scripts/task proofs` / `python3 proof/check.py`):**
     - §2.4 ambient soundness (`ambient_soundness` in `proof/calculus.py`)
     - §2.4 substitution depth monotonicity (`substitution_monotonicity` in `proof/calculus.py`)
     - §1.2 / §2.4 soundness of the depth bound under reduction (`soundness_of_bound` in `proof/calculus.py`)
     - §1.6 / §90.2 Orpheus premise sufficiency, necessity, and non-interval separation (`look_sufficiency_ambient`, `look_sufficiency_confinement`, `look_necessity_confinement`, `look_necessity_numeric`, `numeric_vs_membership_counterexample`, `binding_taint_violates_ambient`, `scope_restain_shifts_obligation_to_caller` in `proof/orpheus.py`)
     - §6.5 / §6.8 core residual round-trip and staging law (`roundtrip_identity`, `hole_set_union_law`, `staging_idempotence_set`, `staging_law`, `dependent_hole_non_monotone_counterexample`, `cumulative_dependent_hole_law` in `proof/staging.py`)
     - §7.1–§7.3 canonical wire model prefix-free stream round-trip and injectivity (`value_decode_encode`, `node_decode_encode`, `injectivity_from_left_inverse`, `cairn_preimage_injectivity` in `proof/encoding.py`)
  2. **Tier 2 — Tested (in the Rust implementation, reproduced by `scripts/task test` and `scripts/task fuzz`):**
     - `0226` differential agreement between `nether_core::check` and `model_check` (`10,000,000` seeded valid and invalid programs across all 8 `FaultKind` variants in `crates/nether-core/tests/calculus.rs`)
     - `0052` generative burial monotonicity and residue well-typedness (`10,000,000` programs in `crates/nether-bury/tests/monotone.rs`)
     - Surface syntax round-trip and staged burial (`crates/nether-bury/tests/residue_is_source.rs`, `crates/nether-syntax/tests/lowering.rs`, `crates/nether-syntax/tests/fuzz.rs`)
     - §5.4 single-assignment naming and loop/branch freezing (`crates/nether-syntax/tests/frozen.rs`, `crates/nether-syntax/tests/aggregates.rs`)
     - Production codec fixtures and fuzzing (`crates/nether-ledger/src/codec.rs`, `crates/nether-ledger/tests/fuzz.rs`)
     - Store concurrency, audit, exchange, portability, and packages (`0205`–`0208`, `0213`–`0215`, `0220`, `0101`)
     - Refusal of `nether run` (§8.0) and bounded evaluation fuel (§6.4)
  3. **Tier 3 — Believed or open (neither proved nor closed):**
     - Cryptographic collision resistance of BLAKE3 on domain-separated preimages (§7.2)
     - Type-level exclusion of invalid in-memory `Value` and `Node` states (`0115`)
     - Two-machine independent verification of a tagged release binary (`0221`)
     - Open roadmap items (`0075`, `0078`, `0081`, `0082`, `0103`–`0106`, `0108`, `0210`, `0235`)
- Verified `cairn check`, `cairn render`, `scripts/task site`, `scripts/task gfx`, `python3 tests/transcripts/run --record`, `python3 tests/links/run`, `python3 tests/font/run`, `python3 tests/attribution/run`, and `python3 proof/check.py`.
