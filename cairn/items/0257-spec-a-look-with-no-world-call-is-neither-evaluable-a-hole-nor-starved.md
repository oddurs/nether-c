---
id: 257
title: 'Spec: a look with no world-call is neither evaluable, a hole, nor starved'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
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

- [x] §6.1 says whether a grant gates a call or a depth
- [x] §1.6 says what the Orpheus rule buys and what it does not
- [x] A transcript buries a call-free descent with no grant, and the item records why the `look` instance is not reachable in one burial
- [x] §90.2's binding-taint entry is consistent with the answer

## 2026-09-16

A grant gates a call, not a depth. An expression that reaches no prelude function is evaluated whatever its depth, so descend disk { 1 + 1 } is 2 with nothing granted and the fourth outcome never has to exist. The rejected alternative — a deep, ungranted, call-free expression residualising as the descent written out — would have refused to fold arithmetic because somebody wrote descend around it.

## 2026-09-16

The consequence for §1.6 is now stated rather than left to be found: look is checked and never granted. What shade buys is that a deep value can be held at depth 0 without staining what holds it; what look costs is naming the stratum again in the source. Neither is an authority, and the witness was written when the value was fetched.

## 2026-09-16

The look instance of the gap is not reachable in a single burial, and the criterion was rewritten rather than faked. A shade with an origin past 0 needs a world call to have been answered, and that needs the grant — so with nothing granted the call is a hole and the look starves on it, which is the ordinary path. The gap is reachable only through a descent that reaches no prelude function, which is what tests/programs/peek.nc is. It becomes reachable for a look once exhume can re-bury a residue with fewer grants than the burial that made it.
