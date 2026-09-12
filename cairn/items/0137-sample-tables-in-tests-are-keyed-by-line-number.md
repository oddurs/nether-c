---
id: 137
title: Sample tables in tests are keyed by line number
type: chore
status: buried
milestone: after
assignee: Oddur Sigurdsson
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

## What fixed it

A sample is named by the heading it is under and its ordinal within that
heading: `90-rationale.md § One number on an arrow #1`. The heading text
rather than the slug, so nothing has to reimplement `site/bake`'s anchors, and
so the name reads as what a person would say out loud.

The three hand-edited tables became one. `crates/nether-syntax/tests/spec/mod.rs`
is now the single place that says what each sample is, and the parser's proof,
depth inference's, lowering's and the three lifted programs all read it. That
also surfaced a distinction the old table did not make: §1.6's sample parses
and is *meant* not to check — it is the source the Orpheus error is printed
from — so `Shape::Illegal` says so instead of three files silently omitting it.

## Acceptance criteria

- [x] A paragraph can be inserted anywhere in `spec/` and nothing under
      `crates/` has to change
- [x] The failure names the sample the way the specification names it
- [x] `tests/transcripts/MANIFEST.tsv` too, so its diff shows what changed
      rather than what moved

## 2026-09-12

Filed from 0126, which paid it for the third time.

## 2026-09-12

Proved by copying spec/ with a paragraph pushed in above every section heading and checking that every sample still has the name it had. That test is in grammar.rs, so the naming is itself under proof rather than merely intended.
