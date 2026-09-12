---
section: "08"
title: The rites
status: draft
---

# The rites

Six verbs. The one that is missing is the point.

## 8.0 There is no `run`

An implementation MUST NOT provide a command that executes a Nether C program
without producing a trace, under any name.

`nether run` MUST exit with code 64 and explain what to use instead.

```console
$ nether run hello.nc
nether: there is no `run`.

  A Nether C program is not executed. It is buried — evaluated as far as the
  world allows — and what remains is a trace and the holes the world still
  owes an answer to.

  You probably want:

    nether bury <file.nc>     to evaluate it
    nether exhume <cairn>     to answer its holes
    nether lamp <cairn>       to see what it left behind
```

It is spelled out here, in the specification, so that it is a rule rather than
a joke, and so that anyone proposing to add it has something to argue against.

This is the only frozen requirement in this draft.

## 8.1 Common behaviour

Every rite:

- accepts `--json` and, when given it, writes a single JSON object to stdout
  and nothing else;
- writes human-readable output to stdout and diagnostics to stderr;
- treats a cairn argument as a full cairn or an unambiguous hexadecimal
  prefix, and MUST fail on an ambiguous prefix rather than choosing;
- names a source position by the cairn of the source and not by a path,
  because that is all a span holds ([§7.3](07-ledger.md#73-nodes));
- exits with a code from §8.8.

No rite writes to the ledger except `bury`, `exhume` and `graft`, and each of
those writes before it prints.

## 8.2 `bury`

```
nether bury <file.nc> [--grant <cap>]... [--fuel <n>] [--json]
```

Evaluates as far as the granted capabilities allow; writes a trace; prints a
summary of what was written.

```console
$ nether bury build.nc
buried   build.nc → dd1289f4   depth 3   holes 1   4 nodes
  hole ①  read("main.nc")                stratum 3  disk
```

With no `--grant`, burial holds no capabilities and every world-touching
expression becomes a hole. This is the default because it is the only default
that cannot surprise anyone.

Until [§8.3](#83-exhume) is built, an implementation that cannot honour a grant
MUST refuse one rather than accept it and evaluate as though it were not there.
A flag that validates its argument and changes nothing looks like it worked,
which is the failure §8.0 is about in a smaller place.

`--fuel` sets the budget of [§6.4](06-evaluation.md#64-starvation-and-fuel).
Its default MUST be finite, and the rite MUST report what it was when asked.

It is **not** recorded in the trace, because it cannot have affected one.
Running out of fuel produces a halt and no trace at all
([§6.4](06-evaluation.md#64-starvation-and-fuel)), so a budget has exactly two
outcomes: the burial finished, or there is nothing to record it in. A budget
that did not bind left no mark, and a budget that bound left no trace.

What a trace does record is `fuel_spent`, which is a fact about the burial that
happened rather than about the room it was given.

## 8.3 `exhume`

```
nether exhume <cairn> [--grant <cap>]... [--replay] [--json]
```

Grants capabilities, answers holes, records every answer, re-buries the
residue, and writes a new trace.

`--replay` grants nothing and serves only from the ledger. It MUST fail rather
than reach the world if an answer is missing, and it MUST report the first
node whose cairn differs from the original.

`--grant` and `--replay` are mutually exclusive.

## 8.4 `lamp`

```
nether lamp <cairn> [--provenance] [--depth <n>] [--json]
```

Carries light down. With no flags, renders the value at that cairn — including
every `Deposit` node in a trace, in source order, which is how a program's
output is read.

*Source order* is by offset within one source, and a trace may hold more than
one. Deposits are therefore grouped by the cairn of the source they were made
from and ordered by offset within each group, rather than interleaved by
offset across all of them — which would be an order corresponding to nothing
anybody wrote. Which group comes first is the order the cairns sort in, because
a source has no other order: one trace's two sources were never written down in
a sequence.

```console
$ nether lamp fd996152
Hello from the nether
```

`--provenance` walks backwards instead: from a value to the nodes that
produced it, to their inputs, to the literals and holes at the bottom.

```console
$ nether lamp d3ea6558 --provenance
d3ea6558  hole     read("main.nc")   stratum 3
└─ trace    depth 3   residue f1221324   1 hole(s)   0 witness(es)   0 deposit(s)   11 steps
```

That is the walk over the trace from [§8.2](#82-bury), which has a hole and no
answers yet: the hole, and the trace waiting on it. Once
[§8.3](#83-exhume) has recorded a witness the same walk reaches it, which is
what [§7.4](07-ledger.md#74-provenance) means by the forward edge read
backwards — a hole records no dependents and does not need to.

`lamp` is a tool the operator carries, not a capability the program holds. A
program cannot invoke it, cannot reach a terminal, and has no way to know
whether anyone is looking. This is the inversion of HolyC's bare-string
`PrintF`, and it is what makes it structurally impossible for a Nether C
program to leak to a log.

## 8.5 `cairn`

```
nether cairn <path> | --verify <cairn> [--json]
```

Names a thing by its content, or checks that a name still holds. `--verify`
MUST report *what* differs, not merely that something does.

## 8.6 `strata`

```
nether strata <cairn> [--json]
```

Blame for depth. Reports the deepest stratum a trace reached and the source
span that took it there.

```console
$ nether strata dd1289f4
depth 3   disk

  3  read("main.nc")           7bced69b:10:35   pending
  0  everything else

  replayable: yes
  of that, 0 (pure) has happened; the rest is what exhuming will cost.
```

Holes are reported apart from witnesses, because they are not the same depth:
a hole is a stratum the trace will need and a witness is one it reached
([§7.3.2](07-ledger.md#732-what-a-decoder-cannot-check)). A trace that has
answered nothing has depth 0 and says what exhuming it will cost.

If the trace is marked as having reached stratum 8, `strata` MUST say so
without being asked, and MUST report `replayable: no`. A witness deeper than
the recorded depth is a malformed trace (§7.3.2), and `strata` is where it is
caught.

"Why is this value at depth 5" has to be a command, not an investigation.

## 8.7 `graft`

```
nether graft <cairn> --replace <cairn> --with <cairn> [--json]
```

Substitutes a subtrace and re-buries only what changed, which
[§6.5](06-evaluation.md#65-residue) guarantees is sound.

`graft` MUST report how many nodes were reused and how many were recomputed.
That ratio is the whole argument for content addressing, and a build tool that
hides it is asking to be trusted rather than measured.

## 8.8 Exit codes

| Code | Meaning |
| ---: | --- |
| 0 | success |
| 1 | the operation failed for a stated reason |
| 64 | usage error, including `run` |
| 65 | the source or trace is malformed |
| 66 | a cairn was not found in the ledger |
| 69 | not implemented in this build |
| 70 | internal error; this is a bug |
| 75 | fuel exhausted |
| 77 | a capability was needed and not granted |
