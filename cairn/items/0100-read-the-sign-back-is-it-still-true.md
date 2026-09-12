---
id: 100
title: 'Read the sign back: is it still true?'
type: chore
status: buried
milestone: sign
assignee: Oddur Sigurdsson
depends_on:
- 98
created: 2026-09-11
updated: 2026-09-12
priority: p1
effort: s
area: site/src/index.html
proof: Every factual claim on the landing page is traced to the spec section that makes it, or removed
---

The hazard of writing in absolutes is that absolutes drift from the thing they
describe, and nobody notices because they sound confident.

After the rewrite, walk the page claim by claim. Each one either points at a
spec section that says it, or comes out. A manifesto that has stopped being
accurate is just a mood.

This is deliberately a separate item from the rewrite, done after it, because
the person who wrote a sentence is the worst person to ask whether it is true.

## 2026-09-12

Walked the page claim by claim. 40-odd assertions checked; one was false.

THE ERROR: the page said 'the store holds a million nodes and hands them back byte for byte'. It does not. The million-node proof exercises the CODEC — encode, decode, compare bytes — and never touches the Store type at all. The largest the store has ever been tested with is 64 objects. Corrected to say what was actually measured: a million nodes encode and decode byte for byte, and the store keeps them, names them by content, and remembers what pointed at what.

That is exactly the drift this item exists to catch, and it is the kind that sounds confident. 'The ledger is built' is true; attributing the ledger's most impressive number to the wrong component is not.

VERIFIED MECHANICALLY: exits 64 (and prints what to use instead, run against the real binary); eleven typing rules, counted; site/bake is one file, stdlib imports only; 14 of 16 VGA colours complement back into the palette, recomputed from scratch, with 5555FF and AA5500 outside it; .decay-ceiling holds a number; one binary; nine doctrine passages; six rites; fourteen anchored spec links, all resolving.

VERIFIED AGAINST THE SPEC: 17 claims, each grepped to the section that makes it normative — no ascend (1.2), witness written before the value (1.4), cairn always depth 0 (1.5), the Orpheus rule (1.6), stratum 8 marked permanently and transitively (1.7), nothing printed (6.8), replay holds no capabilities (6.7), holes carry finished arguments (6.3), a refusal is an answer and starvation is not catchable (9.9), no run (8.0), immutability after first read (5.4), blake3 under a versioned domain (7.2).

STRANGENESS AUDIT: 90-odd short declarative lines, every one about determinism, records or depth. The closest to the line is 'entropy is the deepest recordable sin' — 'sin' is borrowed from the temple's register rather than the nether's — and it stays, because the lattice really does order strata by how much of the world you disturbed (1.8) and the word carries the Oracle inversion. Kept deliberately, not by omission.
