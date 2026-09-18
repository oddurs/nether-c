---
id: 266
title: Decide whether the prelude gains an Answer constructor
type: spec
status: unmarked
milestone: codex
created: 2026-09-17
updated: 2026-09-17
priority: p1
effort: m
stratum: 0
area: spec/09-prelude.md
proof: A program can return an Answer it built, or §9.2 says why it may not
part_of: [263]
---

## What this section must answer

`spec/09-prelude.md` §9.2 binds three inspectors over `Answer⟨T⟩` — `given`,
`refusal`, `must` — and no constructor, so every `Answer` that exists came out
of the prelude. 0263 established that this is not a rule anybody chose: `utf8`
is at depth 0 and refuses `malformed`, so a pure prelude function does what a
pure program function cannot, and §9.9 now says the type is about what can be
refused rather than about what the world said.

The question is whether §9.2 gains `answered` and `refused`, and it is two
questions wearing one coat.

## Constraints it inherits

- **The name.** `given` is the predicate, so the constructors cannot be named
  for §5.1.1's two cases without one name meaning both the question and one of
  the answers. `answered` and `refused` are the obvious pair and `refused` sits
  one letter from `refusal`, which is the inspector pointing the other way.
- **The type.** `answered(v)` takes its `T` from its argument, the way `must`,
  `given` and `refusal` all do — `crates/nether-syntax/src/lower.rs` says the
  shapes live there "because three of them are polymorphic in `T` and the IR's
  types have no variables". `refused(absent)` carries no `T` at all. It has to
  come from the return type or the binding, and nothing in the IR threads an
  expected type into a call.
- **The test.** `the_prelude_is_the_one_in_section_09` reads the signature
  fences out of §09 and compares them with `Prim::ALL`, so the specification
  cannot gain these two ahead of the implementation. Whatever is decided lands
  as one change.

## Open questions

- [ ] Does `refused` take its `T` from an expected type threaded through
      lowering, or does the IR grow type variables, or is there a third shape
      — a refusal that is not an `Answer` until something accepts it?
- [ ] Is `answered` worth having on its own? It needs no new machinery, and a
      program that can say yes and not no is worse than one that can say
      neither.
- [ ] Does a depth-0 refusal a program built need anything from §1.4? 0263
      says no — nothing was asked, so there is no witness — and that is the
      sentence a reviewer is most likely to want to argue with.

## Delivery steps and dependencies

1. Settle the type question first; the name is a paragraph once it is decided.
2. §9.2 gains the signatures and §5.1.1's paragraph stops naming this item.
3. `Prim`, `prim_arrow`, the folding in `crates/nether-bury`, and a program in
   `tests/programs/` that returns an `Answer` it built.
4. `spec/90-rationale.md` records whichever shape lost.

## Acceptance criteria

- [ ] Every normative claim is stated once, in one place
- [ ] Every code sample in it is in `tests/transcripts/`
- [ ] A reader who has not read the rest of the spec can follow it
