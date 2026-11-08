---
id: 282
uid: 946c6e60-6e7a-41dc-84bb-99fef9086dc2
title: Say what the proofs say, and number it once
type: spec
status: buried
milestone: assay
depends_on:
- 227
assignee: Oddur Sigurdsson
claimed: 2026-11-07
created: 2026-11-07
updated: 2026-11-07
priority: p1
effort: s
stratum: 0
area: spec/90-rationale.md
proof: tests/links/run fails on a section number that heads two sections and passes on the tree, proof/check.py fails on a proof script that names the kernel seal, and every claim in §90.7 names a theorem, test or item that says what the claim says
---

The section 0227 added to `spec/90-rationale.md` reused the number §90.3,
which already heads "Things Nether C is worse at". The links check did not
catch it, because the two headings have different anchors.

The section also claimed more than the tree holds:

- The staging law was described as `bury(bury(e, A), B) = bury(e, A ∪ B)`.
  The theorem proves that `StagedEval`, which defines both reductions side by
  side, gives both the same result. That `StagedEval` describes `nether-bury`
  is believed.
- 0226 was described as agreement with "the mechanized model". It is agreement
  with `model_check`, a Rust transcription of it.
- Tier 2 claimed byte-identical stores across architectures. No test builds
  stores on two architectures.
- Tier 3 named open items by titles they do not have.

The kernel's seal is a Python name, and nothing stopped a proof script from
importing it.

### Acceptance and evidence

- [x] The section is §90.7, and `tests/links/run` fails on a duplicated number.
- [x] Each corrected claim matches the theorem, test or item it cites.
- [x] `proof/check.py` rejects a proof script that names `_SEAL`, `_thm` or `_token`.
