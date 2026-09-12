---
id: 152
title: lamp adds a newline to a value that has one
type: bug
status: buried
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

## 2026-09-12

Fixed, and it was two bugs in one path.

println! added a newline to a value that already ended in one. Now: print the bytes exactly, and add a newline only when the text does not already end in one — so a value with its own newline is exact, and a bare 7 still ends the line it is printed on.

deposits were joined with newlines. That inserts bytes the program never wrote: two deposits of "a\n" and "b\n" became two lines with a blank between. They are concatenated now. A program's deposits ARE its output, and a program that wants a separator writes one — which is what printf has always meant.

That second part changes a decision from #83. The test there is about ORDERING (deposits grouped by source, not interleaved by offset) and only depended on the separator incidentally. Its fixture now gives each deposit its own newline, as a program that wanted lines would, so the ordering claim survives and is more realistic than it was.

0.7's transcript still cannot be executed by tests/transcripts: running it needs a scratch store, a written hello.nc, and the cairn substituted from the first command into the second. That is 0066's job and it is filed there. crates/nether-cli/tests/rites.rs covers the behaviour end to end meanwhile.
