---
id: 172
title: A sealed value has no syntax, so a residue holding one is not a program
type: spec
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: spec
stratum: '0'
proof: A residue that seals something parses
---

## Measured

```console
$ cat s.nc
Cairn c = seal b"hi";
demand c;

$ nether bury s.nc && nether lamp <residue>
Cairn c = (seal b"hi");

demand «a cairn has no syntax»;
```

[§6.5](../spec/06-evaluation.md) is a MUST: a residue is a program, and
printing one and lowering it again has to give the same thing. That residue
does not parse. No capability was granted and none is needed — `seal` is
depth 0, and any program that seals something and leaves the name in the
residue produces this.

[§3.6](../spec/03-lexical.md) has no cairn literal, so `crates/nether-syntax/src/print.rs`
has nothing to print and emits a placeholder that says so. The placeholder is
honest; the gap is in §3.6.

It also puts [§9.3](../spec/09-prelude.md) out of reach entirely. `fetch_node`
and `has_node` take a `Cairn` and nothing else, so every unanswered call to one
is a residue that cannot be read back, and 0068 can build the provider but
cannot demonstrate it.

## What it needs

A literal in §3.6, and a sentence in §5.4 or §3.6 saying a cairn is written the
way `nether lamp` and `nether cairn` already print one: sixty-four lowercase
hex digits, with a mark in front so it is not an identifier.

## Acceptance criteria

- [x] §3.6 gives a cairn a literal form
- [x] §5.4 or §3.6 says it is the same spelling the rites print
- [x] The rejected alternative is in `spec/90-rationale.md`

## 2026-09-13

The literal is #<64 lowercase hex>. The mark is needed because an identifier may begin with a letter, so deadbeef is both a plausible cairn prefix and a plausible variable name, and a lexer that counts to sixty-four before it knows which token it is reading cannot write an error message.

## 2026-09-13

No short form. nether lamp 76c4b655 works because a ledger is present to resolve the prefix; a program is read without one.
