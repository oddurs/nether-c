---
id: 151
title: hello.nc demands a function and never calls it
type: bug
status: unmarked
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/00-overview.md
stratum: '0'
proof: Burying the hello.nc the specification shows, and lamping the trace, prints the greeting
---

## What happens

The first program in the specification is

    U0 greet()
    {
      "Hello from the nether\n";   // deposited, not printed
    }

    demand greet;

`demand greet;` demands the *function*. §4.5's `call_suffix := "(" [ args ] ")"`
requires parentheses, so a bare `greet` is a value and not a call: the body
never runs, nothing is deposited, and `nether lamp` on the trace says
`nothing was deposited`.

Verified once 0149 made burial deposit at all:

    $ nether bury hello.nc      # demand greet;
    buried   hello.nc → 886a7b08   depth 0   holes 0   2 nodes
    $ nether lamp 886a7b08
    nothing was deposited

    $ nether bury called.nc     # demand greet();
    buried   called.nc → 638e1436   depth 0   holes 0   4 nodes
    $ nether lamp 638e1436
    Hello from the nether

## Where it came from

HolyC does not need parentheses on a nullary call, and the example reads as
though Nether C does not either. §4.5 says it does, deliberately — Nether C
took out HolyC's implicitness on purpose.

## What should happen

`demand greet();`, in all four places the program appears: spec/00-overview.md,
README.md, site/src/index.html, and tests/programs/hello.nc.

The alternative — allowing a bare identifier to call a nullary function — is a
grammar change that reintroduces exactly the ambiguity between a function and
its result that §4.5 removed. It should be recorded as rejected rather than
left unsaid, since the homage makes it tempting.

## Why this matters more than its size

It is the first program anybody reads, it is on the front page, and until it is
fixed the language's own headline example does not do what its comment says.
