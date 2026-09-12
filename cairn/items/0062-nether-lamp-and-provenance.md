---
id: 62
title: nether lamp, and --provenance
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
- 41
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-cli
stratum: '1'
proof: Any value in any trace can be rendered, and its causal history walked back to a hole or a literal
---

Carrying light down. The most-used verb in practice and the one that decides
whether the language is pleasant, because it is the only way to see anything
at all.

## 2026-09-12

Provenance is the forward edge read backwards, which is exactly what section 7.4 says and exactly what the store's referrers index does. A node names what it was made from; what a value was made for is found by asking who names it. No new index and no new field.

## 2026-09-12

A lamp on a trace shows every deposit under it in the order the program made them, which is how a program's output is read. Nothing is printed when it is made. It is read afterwards by somebody who decided to go and look, and that is the whole inversion of HolyC's bare-string PrintF.

## 2026-09-12

A lamp on a shade shows its stratum and its name and no more, because that is what the value holds. Going further means lamping what is inside it, deliberately, which is the operator's business and not the program's.

## 2026-09-12

Tested against a store built by hand, because nether bury is blocked on 0128 and nothing else fills one yet. Everything the lamp does works on any graph the ledger holds, so it will work on a real trace the day there is one.
