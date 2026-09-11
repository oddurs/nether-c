---
id: 38
title: Freeze the canonical encoding
type: chore
status: buried
milestone: ledger
depends_on:
- 21
- 95
created: 2026-09-10
updated: 2026-09-11
priority: p0
effort: m
area: crates/nether-ledger
stratum: '1'
proof: The encoding document is marked frozen and every later change requires a format version bump
---

Blocked on spec section 07. Nothing else in this descent starts until the
format is written down and frozen — the whole project's reproducibility claim
rests on this one document being right before there is code depending on it.

## 2026-09-11

Frozen. The tag table named nine types and never said what their bytes looked like, which is not something you can freeze — two implementations could have agreed with the old text and disagreed byte for byte. 7.1 now gives the payload of every tag, and a new 7.1.1 enumerates what a decoder must reject rather than leaving rejection to judgement, with the round-trip law stated outright: for every b that decodes to v, encode(v) == b.

One real change fell out of writing it down. The draft had the ledger encode Str over its NFC form, which means cairn(s) is not the name of s but the name of what the store decided s ought to be. Normalisation moved up to the lexer, where the ambiguity actually matters (two spellings of an identifier must resolve to one binding) and where 3.3 already put it. Recorded in 90.2. It also drops a megabyte of Unicode tables, but that is a consequence, not the reason.
