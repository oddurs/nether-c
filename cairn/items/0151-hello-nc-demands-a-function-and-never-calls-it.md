---
id: 151
title: hello.nc demands a function and never calls it
type: bug
status: buried
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

## 2026-09-12

Fixed in all four places: spec/00-overview.md, README.md, site/src/index.html, tests/programs/hello.nc.

    $ nether bury tests/programs/hello.nc
    buried   hello.nc → 0fd0e5e9   depth 0   holes 0   4 nodes
    $ nether lamp 0fd0e5e9
    Hello from the nether

That is 0.7's transcript, run, and it is now a test in crates/nether-cli/tests/rites.rs so it cannot quietly stop being true.

The grammar was NOT the thing that gave, and 90.2 records why. Letting a bare identifier of function type call itself would put back exactly the ambiguity the rest of the language spent its budget removing — greet would mean the function in one position and its result in another, and the reader would need the type to tell which, in a language whose proposition is that a value's type tells you what it cost. It would also make 'seal greet' unwritable: the cairn of a function and the cairn of what it returns are different names for different things.

The cost is two characters and a little of the homage. HolyC's parenthesis-free call is one of its most characteristic gestures and this is a place Nether C simply does not follow it.

Found on the way: lamp adds a newline to a value that already ends in one, so the greeting comes out with a blank line after it. Filed.
