---
id: 224
title: Mechanize the staging law
type: chore
status: unmarked
milestone: assay
depends_on:
- 222
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: xl
area: proof/
stratum: '0'
proof: bury(bury(p, A), B) = bury(p, A union B) is proved for the calculus in section 02
---

6.5's staging law is what makes descent incremental instead of all-or-nothing,
and it is what `graft` relies on. It is stated as a law and has never been
proved.

It is also the property most likely to be subtly false — a residue is source
now (0128), so the law has a print and a parse in the middle of it, and those
have to be exactly inverse for it to hold.
