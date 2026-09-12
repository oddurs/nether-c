---
id: 6
key: surface
title: The Surface
type: milestone
status: buried
created: 2026-09-10
updated: 2026-09-12
priority: p2
due: 2027-02-28
---

## 2026-09-12

Buried. The lexer, the recursive descent parser, lowering, depth inference, diagnostics that name the cause, and the three sample programs — plus four specification holes the implementation found and closed.

A struct could not be constructed: 4.4 had an assign production and a for step that nothing could reach, and 5.4's own sample did not parse. Settled by drawing the line at the ledger — a local may be assigned until it is NAMED — and then built, and then the freeze rule that makes it sound.

A shade parameter had an origin nothing could infer. Settled with no new syntax: a shade's origin is the depth of the value it holds, and Shade<Bytes@5> already parsed.

5.6 said types were inferred in a body and the grammar had no way to write such a binding. Settled the other way, and the rule that came out is better than the one it replaced: types are written, depths are inferred.

Every one of those was found by building the thing rather than by reading it again.
