---
id: 215
uid: 45db6dc5-c1e3-46ba-9356-823b43ce4d39
title: A second store, and what they owe each other
type: spec
status: buried
milestone: ossuary
created: 2026-09-13
updated: 2026-11-07
priority: p2
effort: l
area: spec/07-ledger.md
stratum: '5'
proof: Two stores can be compared and reconciled by cairn alone, specified before it is built
---

Two people with two stores have a set-difference problem and nothing else: a
cairn is either there or it is not, and if it is, it is the same bytes.

This is the substrate 0101's package format needs and the thing a public store
serves. It should be specified once, here, rather than three times by whoever
needs it first.

## Delivery plan — 2026-09-15

### Starting point and scope

Cairns establish identity, not availability, authorization or witness agreement. Specify exchange rather than deploy a service.

### Steps

1. Define root closure, bounded comparison, missing/corrupt responses and version compatibility.
2. Specify receiver verification, interruption/resume and discovery privacy; distinguish read/fetch from active-send strata.
3. Write transcripts for equal, partial, divergent and malicious stores and record rejected formats.

### Acceptance and evidence

- [x] Verified objects reconcile under a precise bounded contract. Possessing a cairn does not imply authentication, mutation authority or agreement about world answers.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Base commit: `fa193cf` (`spec/07-ledger.md` §7.5.3 `A second store, and what they owe each other`, `spec/90-rationale.md` §90.2 `Reconciling stores by whole-inventory diff or by shipping refs/`).
- Specified:
  - What a cairn establishes (byte identity under `DOMAIN`) and what it does not (availability, authentication/mutation authority, or agreement across world witnesses).
  - Forward root closure $\operatorname{cl}(R)$ over verified `Node` references, domain-separator handshake (`netherc/cairn/v4\x00`), bounded `want` batches (`MAX_WANT = 1,024`) and object byte bounds (`MAX_OBJECT_BYTES = 16 MiB`).
  - Receiver verification (`len <= MAX_OBJECT_BYTES`, canonical `encode(decode(bytes)) == bytes`, `cairn(bytes) == asked`, atomic commit via `put` deriving `refs/` locally, refusal of unsolicited or corrupt objects and of peers with a different domain separator).
  - Interruption/resume safety via verified upper prefixes of $\operatorname{cl}(R)$, discovery privacy (refusing prefix resolution and store-wide enumeration from untrusted peers), and stratum boundaries (stratum 3 `disk` + 4 `disk!` locally, stratum 5 `net` + 4 `disk!` for remote fetch, stratum 6 `net!` for active push).
  - Pinned exchange transcripts for equal, partial, divergent and hostile stores in `tests/transcripts/MANIFEST.tsv`.
- Remaining limits: specification only; wire transport and CLI rites are deferred to implementation items (starting with `0213` for portable export/import and `0101` for package resolution).
