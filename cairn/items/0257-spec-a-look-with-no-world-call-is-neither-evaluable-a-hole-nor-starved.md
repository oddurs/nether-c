---
id: 257
title: 'Spec: a look with no world-call is neither evaluable, a hole, nor starved'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
priority: p0
effort: m
stratum: '0'
area: spec/06-evaluation.md
proof: Burying a look with no grant has one stated outcome and a transcript
---

## The expression with no outcome

```c
Shade<Bytes@5> s = /* ... */;
demand len(descend net { look s });
```

buried with no `--grant`. `look s` is at depth 5 by [LOOK]. Three rules are
asked what burial does with it and none of them answers:

- §6.1 — burial "can evaluate an expression when the expression's depth is
  within the granted capabilities". Nothing was granted, so it cannot.
- §6.3 — a hole records a `call`, "the prelude function and its
  fully-evaluated arguments". `look` is not a prelude function. So it is not a
  hole.
- §6.4 — an expression starves when it "depends, transitively, on a hole".
  There is no hole. So it has not starved.

It has not collapsed either: it can produce a value, and will, the moment
somebody grants `net`. `descend disk { 1 + 1 }` is the same gap with no shade
in it.

## Which way out

**A grant gates a call, and depth alone never blocks evaluation.** Then §6.1's
sentence is wrong as written: what a grant permits is a *call*, and depth is
what a call costs. `look` always evaluates and the Orpheus rule is entirely
static.

That has a consequence worth stating out loud, because
[§90.2](../spec/90-rationale.md) rejected binding-taint with it — "if looking
is free, nothing has been carried". Under this reading looking *is* free: it
costs the writer `descend net`, which they may write anywhere, and costs the
operator nothing. The rule still buys a local check and a diagnostic that names
the fix. It does not buy a capability, and §1.6 currently reads as though it
does.

**Or depth gates evaluation, and there is a fourth thing an expression can be.**
Deep, ungranted, not a call: neither evaluated nor a hole, and §6.5 has to say
what it residualises as — the descent as written, since there is nothing inside
it to reduce.

The first is almost certainly right, and it is not what §6.1 says.

## Acceptance criteria

- [ ] §6.1 says whether a grant gates a call or a depth
- [ ] §1.6 says what the Orpheus rule buys and what it does not
- [ ] A transcript buries `descend net { look s }` with no grant
- [ ] §90.2's binding-taint entry is consistent with the answer
