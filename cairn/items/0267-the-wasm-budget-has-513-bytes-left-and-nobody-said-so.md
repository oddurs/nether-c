---
id: 267
title: The wasm budget has 513 bytes left, and nobody said so
type: chore
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: m
stratum: '0'
area: tests/wasm
proof: The headroom is a number somebody chose, and where the bytes are is printed beside it
---

## Starting point and scope

`scripts/task wasm` prints this today:

```
wasm: 153087 bytes compressed, 513 under the 153600-byte budget
      that is less than a kilobyte of room.
```

0074 chose 150 KiB and argued for it well — a budget rather than a high-water
mark, because a Rust release build is not byte-reproducible and a ceiling set
to the last measurement fails on the next machine. That argument is still
right. What is missing is that the budget is nearly spent and no open item
says so.

The trend is in the closed items' own notes and nowhere else:

| When | Compressed | Free |
| --- | ---: | ---: |
| 0074, 2026-09-13 | 147,512 | ~6.1 KB |
| 0075, ~2026-09-15 | 146,840 | ~6.8 KB |
| now | 153,087 | 513 B |

So the next commit that touches `nether-core`, `nether-syntax` or
`nether-bury` turns the build red, and the person who hits it has one obvious
escape — edit `.wasm-ceiling` — which is the exact move 0074 designed the
budget to make deliberate. The argument for why 150 KiB was chosen is in a
closed item they have no reason to read.

## Where the bytes actually are

Read out of the module's own sections, the way `tests/wasm/run` already reads
its exports:

| Section | Raw | Compressed |
| --- | ---: | ---: |
| `code` | 231,675 | 84,810 |
| `data` | 141,243 | 66,964 |
| everything else | 1,694 | ~1,000 |

Of the 141 KB data section, 10 KB is printable strings — every diagnostic this
project writes, the prelude's names, the keyword table. The other 131 KB is
`unicode-normalization`'s tables: `perfect_hash`, `decompose`, `lookups`.

That is the largest single thing a browser downloads here, it is 44% of the
budget, and it is **not** what grew. The 6.2 KB since 2026-09-15 is `code`, from
0254, 0255, 0261, 0263 and 0264 landing. The tables have been there since the
module existed and nothing recorded that either.

Three ways not to pay for them, none of which is taken here:

- **Drop normalisation in the browser build.** Refused outright. §3.3 puts NFC
  in the lexer so that two spellings of a name cannot resolve to two bindings,
  and a build that skips it is a second implementation that disagrees with the
  first about what a program means.
- **Cut the tables down to the identifiers.** The characters an identifier may
  hold are `XID_Start`/`XID_Continue`, which is far less than all of Unicode.
  This is writing our own subset of somebody else's data, and
  [§90.2](../spec/90-rationale.md) draws the dependency line exactly there: a
  dependency is admissible when what it holds is data somebody else is the
  authority for. Generating a subset at build time moves the lines and not the
  problem, which is the same sentence that section already uses.
- **Shrink it with a tool.** `wasm-opt` is a dependency and this repository
  does not take one to save bytes.

## Steps and prerequisites

1. Teach `tests/wasm/run` to print the section breakdown beside the headroom.
   It already parses the export section by hand; sections are a tag, a length
   and a body, and reading them is fewer lines than the export table. A budget
   conversation with no numbers in it is how a ceiling gets raised twice.
2. Choose the next figure, in the open, and write down what a browser pays for
   it rather than only the number.

## Acceptance and evidence

- [x] `scripts/task wasm` prints where the bytes are, not only how many
- [x] `.wasm-ceiling` is a round number somebody chose, with the reasoning in
      the pull request body
- [x] The Unicode tables are recorded as the largest item and as a thing this
      project has decided to pay for, so the next person does not rediscover it
- [x] No dependency is added, and no build skips normalisation

## 2026-09-18

160 KiB, raised from 150. The module is a lexer, a parser, a depth checker, burial and the ledger encoder, plus NFC tables, and 160 KiB compressed for all of that is small. 10,753 bytes of room, which is the posture 0074 chose originally — about six to seven kilobytes free and a round number.

## 2026-09-18

The tables are 44% of the download and are not what grew. The 6.2 KB since 2026-09-15 is code, from 0254, 0255, 0261, 0263 and 0264. Recording both in §90.2 and in the build output is what stops the next raise being argued from a single number, which is the only direction a single number can be argued.
