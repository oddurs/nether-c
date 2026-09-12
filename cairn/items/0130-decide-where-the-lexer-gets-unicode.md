---
id: 130
title: 'Decide: where the lexer gets Unicode'
type: spec
status: buried
milestone: surface
depends_on:
- 17
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/90-rationale.md
proof: The rationale names every dependency in Cargo.toml, and the reason each one is not written here
---

## The question

`spec/03-lexical.md` §3.3 requires two things a lexer cannot do without
Unicode's tables:

- identifiers are `XID_Start XID_Continue*`;
- identifiers are compared after **NFC normalisation**, and an implementation
  MUST normalise before comparing or recording.

`CLAUDE.md` has one stated exception to writing everything here, and it is
cryptography, on the grounds that a hash function that is subtly wrong fails
silently and years late. Two identifiers that should compare equal and do not
fail exactly that way.

## How many lines the part we actually need is

The rule says to work it out, so:

- `XID_Start` and `XID_Continue` are about seven hundred code point ranges. A
  binary search over them is ten lines; the ranges are fourteen hundred.
- NFC is canonical decomposition, then canonical ordering by combining class,
  then canonical composition minus the exclusions. The algorithm is about a
  hundred and fifty lines and every one of them is interesting. The tables are
  roughly three thousand: two thousand decomposition mappings, nine hundred
  combining classes, and the exclusion list.

So: two hundred lines of algorithm and forty-five hundred lines of data.

## Why the Decay Rule settles it

`.decay-ceiling` counts every line of `crates/` that is not a test. Vendoring
those tables puts four and a half thousand lines of Unicode data into the
trusted core — more than twice everything written so far — and every future
commit measures itself against a number that is mostly UCD. The rule exists so
that the code every guarantee rests on can only get smaller. Data that is not
ours, that we would not read, and that changes once a year when Unicode
publishes, is not that code.

Generating the tables at build time moves the lines and not the problem, and
adds a build step that has to be trusted to produce the same bytes twice.

## What this decides

Two dependencies, `unicode-ident` and `unicode-normalization`, and a rule for
the next one: a dependency is admissible here when what it holds is **data
somebody else is the authority for**, and inadmissible when what it holds is
behaviour we could write and should understand. `blake3` is the first kind.
Unicode is the first kind. A markdown renderer, an encoder, a parser and a
web framework are the second kind, which is why this repository has none.

## Acceptance criteria

- [x] §90 says why each dependency in the workspace is not written here
- [x] The admissible/inadmissible line is stated, not implied
- [x] The Unicode version the language pins is named
- [x] The site is rebaked

## 2026-09-12

The Decay Rule settled it rather than taste. Four and a half thousand lines of UCD in crates/ would make every future commit measure itself against a number that is mostly somebody else's data, and the ceiling exists to count the code every guarantee rests on. Generating the tables at build time moves the lines and not the problem.

## 2026-09-12

The rule the three permitted dependencies share is now stated rather than implied: admissible when what it holds is data or a construction somebody else is the authority for, inadmissible when it holds behaviour this project could write and should understand. blake3 and Unicode are the first kind. A markdown renderer, a GIF encoder, an argument parser and a web framework are the second, which is why this repository has written four of those and imported none.
