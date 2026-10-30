---
id: 218
title: Fuzz every parser, not only the decoder
type: chore
status: buried
milestone: warden
assignee: Oddur Sigurdsson
claimed: 2026-10-30
depends_on:
- 216
created: 2026-09-13
updated: 2026-10-30
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

- [x] All named parsers have passing soak evidence and bounded failure behavior. Short CI corpora do not replace soaks or 0219's resource accounting.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Buried. crates/nether-syntax/tests/fuzz.rs fuzzes lex, parse and lower, then check and print on whatever lowers, with xorshift64* as the decoder's fuzzer has it. Corpus: every tests/programs fixture plus samples for escapes, \u{…}, a #cairn literal, every integer spelling, comments, structs and typedefs, shades and a latent set. Inputs: raw bytes (invalid UTF-8 included), byte mutations, splices of other programs, opening and closing punctuation, multi-byte characters at every offset, and inputs built to reach a limit: nesting to twice MAX_NESTING in parentheses, blocks and unary minus, 8 KiB escaped literals and over-long numbers. The short pass, 32000 inputs over 8 seeds, runs in scripts/task test; the soak runs under scripts/task fuzz, which already selected every test target named fuzz. Soak on 169e8b2 plus this branch: NETHER_FUZZ_ROUNDS=10000000, seed 0x5EED0218F00DCAFE, 154 s, 4065657 lexed, 2276770 parsed, 2137295 lowered, no panics, peak RSS 10 MiB. The cairn parser's # literal is in the lexer and covered here; Cairn::from_str was already fuzzed in nether-ledger. Limits: inputs stay under a few KiB, so MAX_SOURCE (4 GiB) is not reached, and per-path resource accounting is 0219. The first run also checked that whatever lowers prints as source that lowers, and that is false: a flat 80-term sum prints 79 parentheses deep and its trace can never be exhumed. That property is 0279 and comes back with its fix.
