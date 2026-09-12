---
id: 55
title: The recursive descent parser
type: feature
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 18
- 54
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every sample in the spec parses; malformed input produces one error, not a cascade
---

The technique is also the thesis. Written by hand from the EBNF in spec
section 04, with error recovery good enough that one mistake produces one
message.

## 2026-09-12

The samples are found rather than listed. Every fenced c block under spec/ is extracted and looked up in a table, so a sample added to the specification fails the proof until somebody says what it is — and what each one is has to be said, because the specification writes three different things in a c fence: whole units, fragments of a body, and the signature listings in section 09 that no program ever writes.

## 2026-09-12

One sample does not parse, and it is section 5.4's Header h. It is classified as blocked on 0116 with the item named in the test, so the day 0116 lands this file fails until somebody reclassifies it. A known gap that announces itself is worth more than a known gap in a notebook.

## 2026-09-12

A missing terminator is reported and then assumed rather than aborting the item. Giving up there costs two things for one mistake — a message and the declaration after it — because resynchronisation skips past the next semicolon, which is the one belonging to the next item. Three missing semicolons are three messages and three items, not one message and nothing.

## 2026-09-12

Section 4.6's table is read back out of the specification and compared against the precedence table in the parser, which needed handling escaped pipes in a markdown table to do at all. Filed 0132: seven of section 09's c fences are not the language.
