---
id: 141
title: Nothing bounds recursion outside burial
type: bug
status: unmarked
milestone: surface
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

- [ ] No input to the parser overflows the stack
- [ ] The limit is a number written down, not a consequence of the host
- [ ] Burial's fallback path cannot overflow either
- [ ] The limits are stated where §6.4 says implementation limits are stated
