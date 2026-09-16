---
id: 218
title: Fuzz every parser, not only the decoder
type: chore
status: unmarked
milestone: warden
depends_on:
- 216
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: The lexer, the parser and the cairn parser each soak with no panic and no unbounded allocation
---

The decoder is fuzzed and `Cairn::from_str` is fuzzed, and both of those were
added because a bug was found in them.

The lexer and the parser read a stranger's source. They have a nesting limit,
which is the right instinct, and nothing has ever thrown ten million bytes of
malformed UTF-8 at them.

## Delivery plan — 2026-09-15

### Starting point and scope

Decoder/cairn fuzzing exists. Source parsers take text; malformed UTF-8 belongs at byte-to-text boundaries.

### Steps

1. Inventory input entrypoints, size/nesting limits and existing seeds.
2. Add deterministic nesting, length, escape, Unicode and truncation corpora through scripts/task fuzz.
3. Run recorded soaks per parser with duration, seeds, case counts and memory observations; minimize failures.

### Acceptance and evidence

- [ ] All named parsers have passing soak evidence and bounded failure behavior. Short CI corpora do not replace soaks or 0219's resource accounting.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
