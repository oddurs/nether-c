---
id: 141
title: Nothing bounds recursion outside burial
type: bug
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 137
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: m
area: crates/nether-syntax
stratum: '0'
proof: No input to any rite can overflow the stack; every limit is reported
---

## Three places

`spec/06-evaluation.md` §6.4 makes it a requirement rather than a courtesy:
an implementation states its limits and reports reaching one, rather than
crashing into it. `nether-bury` does — `MAX_FRAMES`, `STACK`, and
`HaltKind::TooDeep`. Three other places do not.

**The parser.** One hundred nested parentheses aborts the process:

```
n=100 -> thread 'zz_deep_nesting' has overflowed its stack
```

Each level walks the whole §4.6 precedence ladder, so a level of nesting is a
dozen frames.

**The checker.** `check::expr` recurses over the IR with no bound, so the same
input reaches it if the parser ever stops refusing.

**Burial's own fallback.** When a thread cannot be spawned, `bury` runs
`burrow` on the caller's stack:

```rust
Err(_) => burrow(unit, source, fuel),   // "Better a shallower burial than none."
```

It is not shallower. `MAX_FRAMES` is unchanged at 2048, and the doc beside it
says roughly sixteen kilobytes of host stack per frame — thirty-two megabytes,
against an eight megabyte main thread.

## What to do

A stated nesting limit in the parser, reported as an ordinary fault. The
checker inherits it, because it only ever walks what the parser produced — but
it is handed IR by `nether-bury`'s tests too, so it needs its own bound or a
stated precondition. Burial's fallback either scales `MAX_FRAMES` to the stack
it actually has, or halts saying it could not get one.

Nothing shipped parses untrusted input yet: `cairn` and `lamp` do not, and
`bury` exits 69. This lands the day `bury` does.

## Acceptance criteria

- [x] No input to the parser overflows the stack
- [x] The limit is a number written down, not a consequence of the host
- [x] Burial's fallback path cannot overflow either
- [x] The limits are stated where §6.4 says implementation limits are stated

## 2026-09-12

Sixty-four, not a hundred and twenty-eight, because sixty-four is what a level costs rather than what a program could want: section 4.6's ladder is ten frames deep, so a parenthesis is about twenty kilobytes of host stack. Measured: eighty parses on a two megabyte thread and a hundred and twenty does not. Raising the number means making a level cheaper first, and the constant says so.

## 2026-09-12

Counted in unary and in ty, which is every place recursion passes through: the ladder above, a parenthesis, a block, an index, an argument and a prefix chain all arrive at unary, and a type argument recurses in ty. Lowering and the checker inherit it and say so rather than stating a second bound.

## 2026-09-12

Burial's fallback now halts with NoStack instead of running on the caller's stack. MAX_FRAMES is thirty-two megabytes of frames against a main thread that has eight, so the fallback was the overflow the scoped thread exists to prevent.
