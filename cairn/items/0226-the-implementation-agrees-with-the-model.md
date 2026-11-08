---
id: 226
uid: e33d08e2-4e8b-4cf5-9ec1-87c777d72384
title: The implementation agrees with the model
type: chore
status: buried
milestone: assay
depends_on:
- 222
assignee: Oddur Sigurdsson
claimed: 2026-11-07
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: xl
area: crates/nether-core
stratum: '0'
proof: A generated program is checked by both the Rust checker and model_check, a Rust transcription of the mechanized model, and they agree, ten million times
---

A proof about a model is worth what the model has to do with the code.

Differential testing between the two is the cheap version of extraction and it
finds the interesting bugs: the ones where the proof is right and the
implementation is not. It also keeps the model honest, because a model that has
drifted from the code stops disagreeing with it.

## Delivery plan — 2026-09-15

### Starting point and scope

The model needs comparable outputs before differential checking means anything. Reuse 0222 and seeded calculus generators.

### Steps

1. Define a shared input subset and normalized success/depth/error results, including invalid programs.
2. Drive both checkers on identical seeded inputs with bounds and mismatch minimization.
3. Run ten million cases, recording versions/seeds/counts; retain a smaller deterministic CI corpus.

### Acceptance and evidence

- [x] Both checkers agree on the ten-million-case corpus and regressions. This tests correspondence, not Rust correctness or syntax outside the shared model.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-11-07

- Added a clean-room reference model (`model_check`) in `crates/nether-core/tests/calculus.rs` transcribed directly from the 11 rules of `spec/02-calculus.md` §2.2 / `proof/calculus.py` (`[LIT]`, `[VAR]`, `[PRIM]`, `[APP]`, `[ABS]`, `[DESCEND]`, `[SEAL]`, `[SHADE]`, `[LOOK]`, `[OPAQUE]`, `[DEMAND]`) plus §5.4 single-assignment naming and §5.6 depth assertions, along with a seeded valid/invalid `Unit` generator (`gen_unit`) and counterexample minimizer (`shrink_disagreement`).
- Fixed two implementation bugs uncovered by comparing the model against `crates/nether-core/src/check.rs` and `crates/nether-syntax/src/lower.rs`, keeping regression tests in `crates/nether-core/tests/calculus.rs` (`a_local_declared_inside_a_loop_is_not_frozen_across_iterations`, `select_arms_join_local_depths_without_bleeding_across_branches`), `crates/nether-syntax/tests/frozen.rs` (`a_local_declared_inside_a_loop_is_fresh_each_turn`), and `crates/nether-syntax/tests/aggregates.rs` (`assigning_a_deep_value_deepens_the_local_across_blocks_branches_and_loops`):
  1. `Checker::bind` in `crates/nether-core/src/check.rs` now clears `self.named[id]` when a local is bound so a local declared inside a loop body (`Stmt::Declare` / `Stmt::Let`) starts fresh on each iteration, and `ExprKind::Select` joins per-arm `self.locals` depths rather than bleeding one arm's assignment depth into the sibling arm.
  2. `Lowering::assign`, `Lowering::branch`, and `Lowering::looping` in `crates/nether-syntax/src/lower.rs` now propagate and join local depths across assignments, branches, and loop fixed points.
- Deterministic CI check (`cargo test --package nether-core --test calculus the_implementation_agrees_with_the_mechanized_model`, seed `0x0226_CAFE_BEEF_0001`, `50,000` programs) runs on every `cargo test`.
- Ten-million-program soak (`cargo test --release --package nether-core --test calculus ten_million_programs_agree_with_the_model -- --ignored --nocapture`, also wired into `scripts/task fuzz`, seeds `0x0226_0000_0000_0001..=0x0226_0000_0000_0008`, `1,250,000` programs per seed) completed in `18.88s` with zero disagreements:
  - Total: `10,000,000` programs (`4,379,907` valid, `5,620,093` invalid)
  - Fault distribution: `Ungranted = 557,330`, `Orpheus = 555,605`, `Stated = 556,228`, `Asserted = 554,480`, `StatedLatent = 689,154`, `AssertedLatent = 600,051`, `Frozen = 555,106`, `Malformed = 1,814,010`.
- Correction, 2026-11-07 (0282): `model_check` is a hand transcription of `proof/calculus.py` into Rust, not the mechanized model itself. Nothing checks the transcription against `proof/calculus.py`, so this compares two Rust programs. `spec/90-rationale.md` §90.7 says so.
- Remaining scope limit: differential checking operates on `nether_core::Unit` terms and `FaultKind` sequences; surface lexing/parsing outside `nether_core` is covered by `crates/nether-syntax/tests/fuzz.rs` and `crates/nether-bury/tests/residue_is_source.rs`.
