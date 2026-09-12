---
id: 152
title: lamp adds a newline to a value that has one
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: crates/nether-cli/src/lamp.rs
stratum: '0'
proof: Lamping the hello.nc trace prints exactly the bytes the program deposited, and 0.7's transcript is a transcript test
---

## What happens

`hello.nc` deposits `"Hello from the nether\n"` — the newline is in the string,
which is what the program wrote. `nether lamp` prints it with `println!`, so
what comes out is `Hello from the nether\n\n`.

§0.7's own transcript shows one line:

    $ nether lamp 8f3a1c0e
    Hello from the nether

## What should happen

§8.4: lamp *renders the value*. Rendering it means the bytes that are in it and
nothing else — a program that wants a newline puts one in, and a program that
does not want one should not get one.

Concatenating deposits exactly also makes the multi-deposit case right. Two
deposits of `"a\n"` and `"b\n"` should be two lines, not two lines separated by
blanks.

## How it was found

0151's end-to-end test. The greeting is printed correctly; there is one byte
too many after it.

## Note

Once this is fixed, §0.7's transcript can move from `pinned` to `checked` in
`tests/transcripts` — it becomes a transcript the binary actually satisfies,
which is worth more than the fix.
