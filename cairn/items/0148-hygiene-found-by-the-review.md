---
id: 148
title: Hygiene found by the review
type: chore
status: unmarked
milestone: after
created: 2026-09-12
updated: 2026-09-12
priority: p3
effort: s
area: crates
stratum: '0'
proof: Each of the five is fixed or has a stated reason not to be
---

The `find` word-splitting went with 0139, which rewrote that loop.

Small things the review turned up. None of them is wrong today; each is a way
for something to become wrong quietly.

- **`store::referrers` returns `io::Result`** where `get`, `put` and `resolve`
  return `Result<_, StoreError>`. One error type per API.
- **`nether-bury` and `nether-cli` list `nether-ledger` in both
  `[dependencies]` and `[dev-dependencies]`.** The second does nothing.
- **`lex::at` and `parse`'s span constructor clamp with
  `u32::try_from(..).unwrap_or(u32::MAX)`.** A source larger than four
  gigabytes gets silently wrong spans rather than being refused.
- **`CLAUDE.md`'s seam list omits `proofs` and `fuzz`**, which
  `scripts/task`'s own usage line has.

## Acceptance criteria

- [ ] Each of the five is fixed, or has a sentence saying why not
