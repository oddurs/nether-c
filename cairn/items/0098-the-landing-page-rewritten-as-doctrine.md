---
id: 98
title: The landing page, rewritten as doctrine
type: feature
status: buried
milestone: sign
created: 2026-09-11
updated: 2026-09-12
priority: p1
effort: l
area: site/src/index.html
proof: A stranger who has never heard of TempleOS reaches the bottom of the page and can state the thesis
---

## Problem

The front page explains Nether C. It should *declare* it. The difference
matters: an explanation invites you to evaluate a proposal, and a declaration
tells you what is true and lets you decide whether you are the kind of person
who agrees.

HolyC's documentation had a texture — abrupt certainty, tonal whiplash, a
numbered absolute where an argument was expected, technical precision sitting
directly beside cosmology — and a homage that files that off is a costume.

## Proposal

Longer. Louder. More numbered absolutes and fewer hedges. The doctrine section
grows from nine lines to nine passages. The palette fact gets room to be as
strange as it actually is. The prose stops apologising for the two-command
hello world and starts insisting on it.

## The line this must not cross

The strangeness has to come from **Nether C's own subject** — the finished
past, the record, things that cannot change — and not from reenacting a man's
symptoms. spec/90-rationale.md 90.4 and CLAUDE.md both already state this.
Take the method: build it yourself, keep it small, follow one idea all the way
down, say plainly what you think is true. Leave the rest of his life alone.

If a passage is only strange, cut it. If it is strange *because the language
is*, keep it.

## Acceptance criteria

- [x] Every claim on the page is still true and still checked by tests/transcripts
- [x] No sentence is strange without being about determinism, records or depth
- [x] tests/links and tests/contrast still pass
- [x] It reads aloud

## 2026-09-11

Nether-themed, and built out of what is already here rather than a new idiom laid on top.

The vocabulary is the one in spec/10-glossary.md and nowhere else: bury, exhume, trace, hole, cairn, shade, lamp, stratum, descend, starve, witness, residue, graft. No new metaphor system. If a passage needs a word the glossary does not have, either the glossary is short a term or the passage is reaching.

The material is what has been built: the nine strata and their core-sample colours, the six rites and the missing seventh, the complemented VGA palette and the fourteen-of-sixteen fact, the lamp that gives back the temple's own colours, the 80-column grid, the DOS title bars, and site/gfx.py which can draw anything the page needs. Nothing is fetched and nothing new is imported.

Every claim stays anchored: a passage that asserts something about the language links to the spec section that makes it normative. That is what keeps this a manifesto about a real design rather than a mood board.

## 2026-09-12

Rewritten. The doctrine grew from nine lines to nine passages, each ending in the spec section that makes it normative — 14 anchored links, all resolving, checked by tests/links.

One doctrine entry changed rather than grew: the Decay Rule came out (it is a fact about the project, not the language) and 'a refusal is an answer, a mistake is not' went in, which 9.9 settled after the page was last written. Decay moved to its own plaque under the specification section.

Stopped apologising for the two-command hello world. The plaque now says what it buys: there is no code path from a running program to your terminal, so a leak is not discouraged, it is absent. You cannot get that by being careful.

The palette section got the room the item asked for — fourteen of sixteen, stated as the strange arithmetic it is, with ROT and BILE named as the two colours that could not be derived from anything.

Four rules instead of one repeated eight times, and the new graphics carry three sections that were previously prose alone.

tests/links caught a bad anchor (#97-stratum-7--entropy, double hyphen). The rewrite also orphaned rule.gif and descent.gif, which are deleted with their generators.

Median sentence is 10 words.

## 2026-09-12

Criteria verified by 0100 rather than asserted here. One claim was wrong and is corrected; the rest hold.
