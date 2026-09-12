---
id: 127
title: 'Spec: section 5.3 says two different things about shade equality'
type: spec
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
depends_on:
- 19
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/05-types.md
proof: One rule for equality, and the Shade encoding in section 7.1 agrees with it
---

## The contradiction

`spec/05-types.md` §5.3 opens:

> Equality is **structural** and total: two values of the same type are equal
> exactly when their canonical encodings are equal, which is exactly when
> their cairns are equal.

and closes:

> `Shade` is comparable: two shades are equal when their underlying values
> are.

`spec/07-ledger.md` §7.1 encodes a `Shade` as **one byte origin stratum, then
the thirty-two byte cairn of the value**. So two shades of the same bytes, one
from stratum 3 and one from stratum 5, have different encodings and different
cairns, and are therefore unequal by the first rule and equal by the second.

`spec/01-strata.md` §1.5 leans the same way as the second: `seal` on a shade
"yields the cairn of the underlying value" rather than a name for the wrapper.

Found while making both of them work in burial, where they are two lines that
cannot both be right.

## What this must decide

Whether a shade's origin is part of its identity.

- **It is.** Then §5.3's last line goes and `seal` on a shade returns the
  shade's own cairn, which makes §1.5 wrong instead. A shade from the network
  and a shade of the same bytes from the disk are then different values, which
  is arguably true and is certainly checkable.
- **It is not.** Then the origin is a fact the *type* carries and the value
  does not, the §7.1 encoding drops the origin byte, and `Shadeᵈ⟨T⟩` encodes
  exactly as the cairn of what it holds. §7.1 is frozen, so this one is not
  free.

## Acceptance criteria

- [x] §5.3 states one rule
- [x] §7.1's `Shade` encoding agrees with it
- [x] §1.5's sentence about sealing a shade agrees with it
- [x] `nether-ledger` and `nether-bury` follow

## 2026-09-12

Settled the way the frozen encoding was already pointing: a shade's origin is part of what it is. What decided it was not that section 7.1 is frozen but that section 1.6 says a shade may be stored, and the only place a Nether C value is stored is the ledger -- a shade read back with no origin has lost the thing that makes the Orpheus rule checkable, and look on it could not be typed.

## 2026-09-12

The cost is section 1.5: seal on a shade no longer reaches through to the value inside, it names the shade. That is the tighter rule and it takes nothing away, since seal is legal at any depth and can be written before the shade.

## 2026-09-12

The cross-stratum case cannot be proved in burial: the checker will not let a stage-one burial hold a finished deep value, so a shade folded at burial time always has origin 0. The proof lives in nether-ledger's encoding tests, where the claim is about encoding anyway.
