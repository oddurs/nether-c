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
buried   build.nc → 4c02ab7f   depth 3   holes 1   903 nodes
  hole ①  read("main.nc")              stratum 3  disk
```

With no `--grant`, burial holds no capabilities and every world-touching
expression becomes a hole. This is the default because it is the only default
that cannot surprise anyone.

`--fuel` sets the budget of [§6.4](06-evaluation.md#64-starvation-and-fuel).
Its default MUST be finite and MUST be reported in the trace, because a trace
buried under a different budget is a different trace.

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

```console
$ nether lamp 8f3a1c0e
Hello from the nether
```

`--provenance` walks backwards instead: from a value to the nodes that
produced it, to their inputs, to the literals and holes at the bottom.

```console
$ nether lamp a1f0c93d --provenance
a1f0c93d  Bytes, 11204 bytes
└─ witness  read("main.nc")            stratum 3   b2e7d410:1:15
   └─ hole ①  answered by exhumation of 4c02ab7f at 2026-09-10T11:04:02Z
```

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
$ nether strata 77de9b31
depth 3   disk

  3  read("main.nc")          b2e7d410:1:15
  0  everything else

  replayable: yes
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
