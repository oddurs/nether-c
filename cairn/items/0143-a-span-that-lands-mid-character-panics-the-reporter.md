---
id: 143
title: A span that lands mid-character panics the reporter
type: bug
status: buried
milestone: surface
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: crates/nether-core
stratum: '0'
proof: A span that lands inside a character renders instead of panicking
---

## The panic

`report::line_of` and `report::snippet` do

```rust
let start = (span.start as usize).min(source.len());
let before = &source[..start];
```

with no char-boundary floor. A span that lands inside a multi-byte character
panics. Token spans are always boundaries, because the lexer advances by
`char::len_utf8` — but `report` is public and takes the source as an argument,
so the source it is handed need not be the one the spans were computed against.
A file edited between burial and rendering is enough.

`nether-cli`'s `strata::place` already floors to the nearest boundary. The
place the rest of the project renders from does not.

## Acceptance criteria

- [x] A span that lands inside a character renders rather than panicking
- [x] A test with a span pointed into the middle of a character

## 2026-09-12

Both offsets go through one cut(), which floors to the nearest boundary. A character early is a worse diagnostic than an exact one and a better one than a panic. The test walks a span across every byte of a source with an astral character in it.
