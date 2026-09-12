---
id: 137
title: Sample tables in tests are keyed by line number
type: chore
status: unmarked
milestone: after
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-syntax/tests
proof: A paragraph can be inserted anywhere in spec/ and nothing under crates/ has to change
---

## The tax

Four places name a sample in the specification by the line its fence opens on:

- `crates/nether-syntax/tests/grammar.rs`
- `crates/nether-syntax/tests/inference.rs`
- `crates/nether-syntax/tests/lowering.rs`
- `tests/transcripts/MANIFEST.tsv`

Insert a paragraph into a spec section and every sample below it moves. The
manifest re-records itself, so that one is a command; the three tables are
hand-edited, and the failure they give is `no entry found for key`, which does
not say that a line moved.

This has been paid three times: 0132 (`09-prelude.md:72` → `:78`), 0126
(`90-rationale.md:340` → `:368`), and the transcript manifest on both. It is
not a bug — everything is caught — but it means editing prose has a cost in
another crate, and prose is what this project mostly writes.

## What would fix it

Name a sample by the nearest preceding heading and its ordinal within that
heading: `90-rationale.md#one-number-on-an-arrow:1`. Both are stable under
insertion, both are what a person would say out loud, and the anchor is
already computed by `site/bake`.

## Acceptance criteria

- [ ] A paragraph can be inserted anywhere in `spec/` and nothing under
      `crates/` has to change
- [ ] The failure names the sample the way the specification names it

## 2026-09-12

Filed from 0126, which paid it for the third time.
