---
id: 225
title: The canonical encoding is injective
type: chore
status: unmarked
milestone: assay
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: l
area: proof/
stratum: '0'
proof: Two distinct values are proved to have distinct encodings, so a collision is a blake3 collision and nothing else
---

7.1 requires one value, one encoding. Every claim in the specification about
names rests on it, and the argument for it is a table and some prose.

The proof obligation is small and worth having written down: the tag
disambiguates the shape, and within a shape the lengths disambiguate the parts.
If that is wrong anywhere, two different values share a name and nothing
downstream can tell.
