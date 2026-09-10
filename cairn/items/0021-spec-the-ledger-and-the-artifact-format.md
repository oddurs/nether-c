---
id: 21
title: 'Spec: the ledger and the artifact format'
type: spec
status: descending
milestone: codex
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: spec/07-ledger.md
stratum: '1'
proof: Two independent implementations of the encoder produce byte-identical output
---

## What this section must answer

The format is the contract; everything else is negotiable. Canonical binary
encoding, the cairn construction, the node model, and the on-disk store
layout.

## Constraints it inherits

Canonical means canonical: one value, one encoding, one cairn, on every
platform, forever. Any ambiguity here is a reproducibility bug that will be
found years later by somebody who cannot fix it.

## Acceptance criteria

- [ ] Canonical encoding fully specified, including integer and map ordering
- [ ] Cairn = blake3 of the canonical encoding, with a stated domain separator
- [ ] Versioning: how a future format change is signalled and refused

## 2026-09-10

Draft landed: spec/07-ledger.md. Canonical encoding, blake3 under a versioned domain separator, the node model, and a stated position on garbage: there is none, and 7.6 admits that is a real cost.
