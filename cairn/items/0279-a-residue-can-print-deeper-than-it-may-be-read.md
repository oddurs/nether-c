---
id: 279
title: A residue can print deeper than it may be read
type: bug
status: buried
milestone: warden
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-10-30
updated: 2026-10-30
priority: p1
effort: m
stratum: 0
area: crates/nether-syntax
proof: For every input that lowers and checks in tests/fuzz, print of the unit parses and lowers, and a trace whose residue holds an 80-term sum exhumes
---

## What happens

`print` parenthesises every expression, on purpose: its header says a printer
that leans on §4.6 to rebuild the shape proves §4.6 rather than lowering. The
parser bounds nesting at `MAX_NESTING` (64). Printing adds a level the source
did not have for every binary and unary operator, so source well inside the
limit prints as a residue outside it, and §6.5 makes a residue a program that
is re-buried on every exhume.

## What should happen

Whatever lowers prints as source that lowers. A trace whose residue cannot be
read again is a trace that can never be exhumed, and nothing tells its owner
until they try.

## Reproduction

1. A function body that is a flat sum of 80 terms, and one disk hole:

   ```c
   U0 f(I64 x)
   {
     x + x + … + x;   // 80 terms, no parentheses
   }

   I64@3 n = len(must(descend disk { read("who.txt") }));
   demand f(n);
   ```

2. `nether bury` succeeds. `nether exhume <trace> --grant disk` refuses the
   residue with `nested more than 64 deep`, pointing at 79 opening parentheses.

The fuzzer's first find was smaller: `I64 a = ` followed by 48 `-` and `1`
prints as 48 nested `(-…)`.

## Cairn of the offending trace

Any trace whose residue holds such an expression. Seed `0x9e3779b97f4a7c15`,
round 2641 of `crates/nether-syntax/tests/fuzz.rs` before the round-trip
property was taken out to land the harness.

## Fix boundary and regression proof

Two shapes, and choosing is a design decision because it touches `print.rs`'s
stated principle:

- **Print by §4.6's table.** Parenthesise a child only when its precedence is
  looser than its parent's, or equal on the side associativity does not take.
  Small and standard, and it is the reliance the header refuses.
- **Keep parenthesising, and stop the printer adding depth.** A left-nested
  chain of one operator prints as one parenthesised run (`(x + x + x)`), and a
  unary operand that is itself unary or an atom takes no parentheses. Leans on
  §4.6 only for left associativity within one level.

Either way, put the round-trip property back into `tests/fuzz.rs` — it was
there, and it is how this was found — and add the 80-term exhume as a rite
test. The rejected shape goes in `spec/90-rationale.md`.

## 2026-10-30

Buried by printing by precedence. print parenthesises a child only when its level is looser than its place allows: the left operand of a binary operator may sit at the operator's level and the right may not, a prefix operand at 2, a postfix base at 1, and anything goes as an argument, an index, a value or a statement. The levels come from parse::LEVELS by spelling, so the printer and the parser read one table. Each operand and each parenthesis passes once through the parser's unary(), which is where MAX_NESTING is counted, so minimal parentheses never nest deeper than the source that lowered. a_long_sum_in_a_residue_exhumes in rites.rs fails on the old printer and passes now. tests/fuzz.rs holds the round-trip property again; stated over units that check, because only those become traces. Soak: NETHER_FUZZ_ROUNDS=10000000, seed 0x5EED0218F00DCAFE, 170 s, 2059693 lowered, no failures, 10 MiB peak. The first soak with the property found a local inside a global's initialiser printing as «local 0»; the checker already refuses that program, so it is 0280 and not this. §90.2 records keeping every parenthesis, and flattening chains, as the alternatives.
