# Nether C

[![CI](https://github.com/oddurs/nether-c/actions/workflows/ci.yml/badge.svg)](https://github.com/oddurs/nether-c/actions/workflows/ci.yml)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)

**A C dialect in which nothing runs.**

A Nether C program is not executed. It is *buried* — evaluated as far as the
world currently permits — leaving behind a **trace**: a complete, immutable,
content-addressed record of everything that happened, together with the
**holes** where the program asked the world a question nobody has answered yet.

Answering a hole is a separate, later act called *exhumation*, and it produces a
new trace rather than changing the old one. Nothing in the ledger is ever
revised.

That is the whole language. Everything else is consequence.

> HolyC compiles the future. Nether C compiles the past.

## Why

Nether C is an inversion of Terry Davis's HolyC, in which every commitment
collapses toward the present moment of total trust: the command line is the
compiler, top-level statements run as they are read, a bare string prints, and
every task holds every privilege over all of memory at all times.

Turn each of those over and you get a language where evaluation is staged rather
than immediate, effects are a one-way descent through nine **strata** recorded
in the type of every value, output is deposited rather than printed, and every
value is found at its content address rather than allocated.

Which is, as it happens, a rather good description of the language people keep
almost-building for reproducible builds, data pipelines, and forensic debugging.
The inversion is the method; the utility is the point.

Every inversion, and what it cost, is in [`spec/90-rationale.md`](spec/90-rationale.md).

## Status

**Nothing is implemented.** The specification comes first, deliberately.

What exists today: twelve sections of specification, a static site baked from
it, a roadmap of 82 items across nine descents, and a binary that refuses
correctly.

```console
$ nether run hello.nc
nether: there is no `run`.

  A Nether C program is not executed. It is buried — evaluated as far as the
  world allows — and what remains is a trace and the holes the world still
  owes an answer to.
```

That is the one behaviour the language will never change, so it seemed like the
honest thing to build first.

## What it will look like

```c
// hello.nc — there is no main; there is a demand.

U0 greet()
{
  "Hello from the nether\n";   // deposited, not printed
}

demand greet;
```

```console
$ nether bury hello.nc
buried   hello.nc → 8f3a1c0e   depth 0   holes 0   17 nodes

$ nether lamp 8f3a1c0e
Hello from the nether
```

Burying says nothing about the greeting and it never can: a Nether C program
holds no capability that reaches a terminal. It left a deposit in the ledger.
Seeing it is a second act, performed by a person who decided to carry a light
down there.

## The nine strata

Every value carries a **depth** between 0 and 8 — how far into the world its
history reaches. Depth composes by `max`, descent is one way, and there is no
`ascend`.

| | Stratum | Grants | Costs |
|---|---|---|---|
| 0 | Pure | arithmetic, data, functions | nothing |
| 1 | Store | reading the ledger by cairn | nothing observable |
| 2 | Frozen Environment | declared variables, a pinned clock | must be declared |
| 3 | Read the Disk | reading files | depends on a filesystem |
| 4 | Write the Disk | creating and modifying files | not undoable |
| 5 | Read the Network | fetching | depends on a stranger |
| 6 | Write the Network | sending | the world remembers |
| 7 | Entropy | true randomness | breaks re-derivation |
| 8 | The Unrecorded | foreign code | breaks provenance |

In TempleOS, entropy was the Oracle: revelation arriving from above, the highest
channel in the system. Here it is stratum 7, nearly the deepest thing there is,
because it is the one act that makes a program impossible to re-derive.

## Reading the specification

Start with [`spec/00-overview.md`](spec/00-overview.md). Sections
[01](spec/01-strata.md), [02](spec/02-calculus.md) and
[06](spec/06-evaluation.md) carry the design; the rest can wait until you need
it.

The specification is canonical as Markdown in [`spec/`](spec/). The site in
[`site/`](site/) is baked from it by [`site/bake`](site/bake) — one file of
standard library Python, no dependencies, no framework — and the resulting HTML
is committed. CI fails if the two disagree.

Every graphic on the site is drawn by [`site/gfx.py`](site/gfx.py), which
contains a GIF89a encoder, an LZW compressor and a 5×7 bitmap font, typed out
rather than imported. There is no image library in this repository, and the
Rust workspace has no third-party crates at all.

## Development

```sh
git clone https://github.com/oddurs/nether-c
cd nether-c
scripts/setup            # once per clone: wires the git hooks
scripts/agent doctor     # check the environment
scripts/task check       # format, lint, test, build, site, decay
```

Work happens one unit at a time, in its own worktree, on its own branch,
through its own pull request:

```sh
scripts/agent start spec/0031-failure-handling   # or: descend
cd ../.worktrees/nether-c/spec/0031-failure-handling
# ... do the work ...
scripts/agent commit "spec(prelude): give starvation a recovery form"
scripts/agent pr
scripts/agent done                               # or: surface
```

The roadmap lives in the repository as Markdown, managed with
[cairn](https://github.com/oddurs/cairn):

```sh
cairn next          # what is ready to start
cairn claim --next  # take it
cairn board         # where everything is
```

`ROADMAP.md` is generated from those items. Never edit it by hand.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the rest, including the Decay Rule.

## Licence

Public domain, via the [Unlicense](LICENSE) — the same terms TempleOS shipped
under. Take it. It was always there.
