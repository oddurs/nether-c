# NETHER C

**A C dialect in which nothing runs.**

[![CI](https://github.com/oddurs/nether-c/actions/workflows/ci.yml/badge.svg)](https://github.com/oddurs/nether-c/actions/workflows/ci.yml)
[![License: Unlicense](https://img.shields.io/badge/license-Unlicense-blue.svg)](LICENSE)

---

The program has already run.

You are not going to start it. That is not how this works. You are going to
read what is left of it.

A Nether C program is **buried**. Burial evaluates it as far as the world will
currently allow, and then stops. What is left behind is a **trace** — a
complete, immutable, content-addressed record of everything that happened — and
the **holes** where the program asked the world a question that nobody has
answered yet.

Answering a hole is a separate act. It happens later. Somebody has to decide to
do it. It is called **exhumation** and it produces a *new* trace.

It does not touch the old one.

Nothing in the ledger is ever revised.

That is the entire language. Everything below is consequence.

> HolyC compiles the future. Nether C compiles the past.

---

## THE NINE THINGS THAT ARE TRUE

**1. There is no `run`.**
`nether run` exits 64 and tells you what to use instead. This is the only
frozen requirement in the entire specification. Everything else is a draft.
This is not.

**2. Depth only ever increases.**
Every value carries a number from 0 to 8: how far into the world its history
reaches. Nothing lowers it. There is no `ascend`. Going down is a decision and
it is the last one you get to make about that value.

**3. Nothing down here changes.**
TempleOS let every task write to all of memory at all times. We kept the
sharing. We removed the writing. Immutability is not a safety feature here. It
is what the word *nether* means.

**4. You never make a value. You find it.**
Every value already exists at its content address. Allocation is something
other languages do because they have not worked out that the value was always
there.

**5. Nothing is printed.**
A bare expression is deposited into the ledger. A program holds no capability
that reaches a terminal, a log, or you. If you want to see it, carry a lamp
down and look. This costs you two commands instead of one, forever, starting
with your first thirty seconds in the language. It is the largest tax in the
design. We charge it anyway.

**6. Randomness is the deepest recordable sin.**
In TempleOS entropy was the Oracle — revelation, arriving from above, the
highest channel in the system. Here it is stratum 7. Nearly the bottom. It is
the one act that makes a program impossible to re-derive. Same mechanism.
Opposite orientation. Read that again.

**7. Every answer the world gives is written down before you get it.**
Not afterwards. Not usually. **Before.** A witness that is recorded after the
value is returned is a witness that can be lost, and a ledger with a gap in it
supports nothing.

**8. The trusted core only ever gets smaller.**
Terry fixed the size of his system by covenant and never changed it. We
inverted that too. `.decay-ceiling` holds a number. Today the number is
**64**. The build fails if the core exceeds it. Lowering it is an ordinary
commit. Raising it requires an argument in public.

Below, things only decay.

**9. You may carry a shade up. You may not look at it on the way.**
Orpheus could have kept her.

---

## THE FIRST PROGRAM

```c
// hello.nc — there is no main. There is a demand.

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

Burial said nothing about the greeting. It cannot. The program has no way to
reach your terminal, and would not know whether anybody was watching if it
did.

It left a deposit. Reading it is a second act and a person had to choose to
perform it.

This is the direct inversion of HolyC, where a bare string is a call to
`PrintF`. The gesture is identical. The semantics are its opposite.

---

## THE NINE STRATA

Every value carries a depth. Depth composes by `max` — a thing built from parts
is exactly as deep as its deepest part.

| | Stratum | Grants | Costs |
|---|---|---|---|
| **0** | Pure | arithmetic, data, functions | nothing |
| **1** | Store | reading the ledger by cairn | nothing observable |
| **2** | Frozen Environment | declared variables, a pinned clock | must be declared |
| **3** | Read the Disk | reading files | depends on a filesystem |
| **4** | Write the Disk | creating and modifying files | not undoable |
| **5** | Read the Network | fetching | depends on a stranger |
| **6** | Write the Network | sending | the world remembers |
| **7** | Entropy | true randomness | breaks re-derivation |
| **8** | The Unrecorded | foreign code | breaks provenance |

Stratum 8 is foreign code, whose history the ledger cannot keep. A trace that
reaches 8 is marked — permanently, transitively — and may never again claim to
be replayable.

It is in the lattice because a language that cannot call C cannot be adopted.

It is at the bottom because it is the only stratum that breaks the promise
rather than deepening it.

It is the weakest part of the design and [§90.2](spec/90-rationale.md) says so
in those words.

---

## ON THE PALETTE

This is not decoration and it is not a joke. Sit with it.

TempleOS was 640×480 and sixteen colours, fixed by covenant, forever. Nether C
takes those sixteen colours and complements every single one of them.

**Fourteen of the sixteen land back inside the palette.** Complement blue, get
yellow. Complement green, get light purple. Complement light grey, get dark
grey. The VGA palette is very nearly its own opposite. That is either a fact
about the 1980s or a fact about opposites and I have not decided which.

Two colours have nowhere to go.

Brown (`#AA5500`) complements to `#55AAFF`. Light blue (`#5555FF`) complements
to `#AAAA00`. Neither of those is a VGA colour at all.

We kept both. We named them **ROT** and **BILE**.

Fourteen of sixteen. Not sixteen. It is the most honest thing about this
project and every graphic on the site is drawn out of those exact values.

---

## WHAT IS TRUE TODAY

**Nothing is implemented.** The specification comes first. Deliberately. The
compiler will not be started until the specification stops moving.

What exists: twelve sections of specification, a static site baked out of it, a
roadmap of 82 items across nine descents, and one binary that refuses
correctly.

```console
$ nether run hello.nc
nether: there is no `run`.

  A Nether C program is not executed. It is buried — evaluated as far as the
  world allows — and what remains is a trace and the holes the world still
  owes an answer to.
```

That is the one behaviour the language will never change. It seemed like the
honest thing to build first.

---

## THE SPECIFICATION

Start at [`spec/00-overview.md`](spec/00-overview.md). Sections
[01](spec/01-strata.md), [02](spec/02-calculus.md) and
[06](spec/06-evaluation.md) carry the design. The rest can wait until you need
it.

The depth calculus is **eleven rules on one page**. That is a hard constraint,
not an observation. A system that cannot be stated on one page cannot be
taught, and a lattice nobody can hold in their head gets worked around rather
than used.

The Markdown in [`spec/`](spec/) is canonical. The site in [`site/`](site/) is
baked out of it by [`site/bake`](site/bake) — one file of standard library
Python, no dependencies — and the resulting HTML is committed. CI fails if the
two disagree.

Every graphic on the site is drawn by [`site/gfx.py`](site/gfx.py), which
contains a GIF89a encoder, an LZW compressor and a 5×7 bitmap font, typed out
rather than imported.

There is no image library in this repository.

There is no markdown library.

There is no web framework.

The Rust workspace has **zero** third-party crates.

A dependency is forever. The part you actually need is usually fewer lines than
you think, and then you understand it.

---

## DEVELOPMENT

```sh
git clone https://github.com/oddurs/nether-c
cd nether-c
scripts/setup            # once per clone: wires the git hooks
scripts/agent doctor     # check the environment
scripts/task check       # fmt, lint, test, build, site, gfx, decay
```

One unit of work. One worktree. One branch. One pull request.

```sh
scripts/agent start spec/0031-failure-handling   # also spelled: descend
cd ../.worktrees/nether-c/spec/0031-failure-handling
scripts/agent commit "spec(prelude): give starvation a recovery form"
scripts/agent pr
scripts/agent done                               # also spelled: surface
```

`main` only ever advances through a merged pull request. This is enforced by a
ruleset on the server and by a hook on your machine. Both have been tested by
trying to violate them.

The roadmap lives in the repository as Markdown, managed with
[cairn](https://github.com/oddurs/cairn). Work is `descending` while it is
underway, `starved` when it is waiting on somebody else, and `buried` when it
is evaluated as far as the world currently allows.

```sh
cairn next          # what is ready
cairn claim --next  # take it
cairn board
```

Every item carries a **proof**: the observable fact that settles whether it is
done. "The code exists" is not a proof. `cairn list --view unproven` finds the
ones that are cheating.

Never edit `ROADMAP.md`. It is generated.

See [CONTRIBUTING.md](CONTRIBUTING.md) for the rest.

---

## ON TERRY DAVIS

Terry Davis wrote an operating system, a compiler, a graphics stack, a document
format and a language. Alone. And made every one of them follow from a single
stated idea.

Whatever you make of the idea, the coherence is the achievement, and it is
rarer than the code.

He was ill, and it is in the work, and pretending otherwise would be its own
kind of disrespect. TempleOS is strange because he was, and it is also
genuinely, technically excellent, and both of those are true at once and
neither cancels the other.

Nether C is an inversion of his language the way a photographic negative is an
inversion: every value reversed, every edge in exactly the same place.

That is a form of close reading.

You cannot turn a thing over without first working out which way up it was.

---

## LICENCE

Public domain, via the [Unlicense](LICENSE). The same terms TempleOS shipped
under.

Take it. It was always there.
