---
id: 153
title: The parser and checker do not implement aggregate construction
type: feature
status: buried
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

## 2026-09-12

Built the declaring and the assigning; split the freezing into 0154.

let_decl's initialiser is optional at block level and required at unit level. A declaration with no value lowers to Stmt::Declare, which is its own IR form rather than a Let of nothing — the printer emits it back as one, which a residue needs since 6.5 makes a residue source.

All three unreachable constructs now work:

    $ nether bury loop.nc      # for (I64 i = 0; i < n; i += 1)
    $ nether lamp <cairn>
    10                          # 0+1+2+3+4

and 5.4's own sample buries.

What is NOT built is the freeze: naming a local has to stop it being assignable, and without that 'seal h; h.len = 4;' compiles, which makes the cairn a lie. That needs an escape analysis, and it is 0154 at p0.

The sample's entry in the spec harness moved from Shape::Blocked to a new Shape::Unchecked — parses, and the checker ought to reject it and does not yet. Blocked already modelled 'the parser cannot do this and here is the item'; Unchecked is the same idea one stage later, so the harness keeps saying what is untrue rather than going quiet.
