---
id: 153
title: The parser and checker do not implement aggregate construction
type: feature
status: unmarked
milestone: surface
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: crates/nether-syntax
stratum: '0'
proof: The sample in 5.4 buries, and reading an unassigned field collapses with a diagnostic that names the field
---

## What 0116 decided

`let_decl := type identifier [ "=" expr ] ";"`. A local may be assigned, and
its fields and elements may be assigned, until it is **named** — sealed,
shaded, deposited, returned, or passed as an argument.

## What is not built

    $ nether bury agg.nc
    error: expected `=` after a binding
     --> agg.nc:8:11
      |
    8 |   Header h;
      |           ^

Three pieces:

- the parser accepts a declaration with no initialiser;
- lowering gives it a local with no value, and the checker tracks which fields
  have been assigned;
- naming a local freezes it, and assigning after that is a diagnostic that says
  where it was named.

## Why it is separate

0116 is the specification decision and it is made. This is the parser, the
lowering and the checker, which is a different size of job.

## Watch out for

`for (I64 i = 0; i < n; i += 1)` depends on this too — the step assigns the
counter. Until it lands, a `for` loop cannot advance, so anything that needs
one is also waiting here.

Reading an unassigned field must collapse, not produce a zero. 5.4 says there
is no value there and there never was one, and a zero would be the
implementation deciding what the program meant.
