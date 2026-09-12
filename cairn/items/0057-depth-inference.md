---
id: 57
title: Depth inference
type: feature
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 47
- 56
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every sample program in the spec compiles with all depth annotations deleted
---

The ergonomics risk, addressed directly. If people must write `@3`
everywhere they will leave, so the test is that the annotations can all be
removed and the program still checks.

## 2026-09-12

The proof is section 5.6's own sentence run as a test: every depth annotation is deleted from every sample the specification writes and every one of them still compiles. Twelve samples, four of them carrying an annotation, and a guard so that a corpus with nothing to delete cannot pass it by default.

## 2026-09-12

The other direction is worth as much and is not what the item asked for: every @n the specification does write is checked against what inference produces. A sample whose annotation disagrees is a sample that does not compile, and the deletion test would have passed anyway because it deletes them.

## 2026-09-12

Inference itself landed with lowering in 0056 — depths are derived there, by the rules in section 2.2 implemented a second time, and the checker compares. What this item adds is the evidence that the ergonomics claim holds.

## 2026-09-12

Filed 0134. Depth annotations are optional everywhere, which is what section 5.6 asks for. Type annotations are mandatory everywhere, which is not: section 5.6 says a type is inferred inside a body and section 4.2 has one production for both kinds of binding, with the type required.
