---
section: "90"
title: Rationale
status: non-normative
---

# Rationale

Non-normative throughout. Nothing here constrains an implementation.

This section exists because a design document that lists only advantages is
marketing. For each inversion: what HolyC did, what Nether C does, and what
that cost.

## 90.1 The inversions

### The command line is the compiler → there is no `run`

HolyC compiles and executes each top-level statement as it is read. The shell,
the REPL and the build are one act, and the result is a system with
extraordinary immediacy: you type, and the machine has already done it.

Nether C moves all of it to the other side. Burial is partial evaluation, and
the artifact is the residue.

**What it cost.** Immediacy. There is no REPL and there cannot be a good one,
because the thing a REPL is for — see the effect now — is precisely what this
language refuses to do. The Necropolis playground is the compensation, and it
is a worse experience than a REPL for the things a REPL is best at.

### A bare string prints → a bare expression deposits

The syntax is preserved exactly; the semantics are reversed. `"Hello\n";` is
still a complete statement, and it still means *this value matters*. It simply
no longer reaches anybody.

**What it cost.** `printf` debugging. The first thing every programmer does in
a new language is print something, and in Nether C that takes two commands
instead of one. This is the single largest ergonomic tax in the design, and it
is charged on the first thirty seconds of every newcomer's experience.

The defence is that it buys the strongest property in the language: a program
holds no capability that reaches a log, so it cannot leak to one. Whether that
trade is worth it is genuinely arguable, and if Nether C fails to find users,
this is the most likely reason.

### The Oracle → stratum 7

Terry's random number generator was a channel: entropy arriving from above,
the highest thing in the system. Nether C puts randomness at depth 7 — nearly
the deepest thing there is — because it is the one act that makes a program
impossible to re-derive.

This is the cleanest inversion in the design. The same mechanism sits at
opposite ends of the same axis, for reasons each system states outright.

**What it cost.** Nothing, in engineering terms. Everything, in the sense that
it is the point where Nether C is most obviously arguing with TempleOS rather
than merely differing from it. That is intentional and it is meant respectfully.

### Ring 0 for everyone → nine strata

TempleOS gives every task every privilege over all of memory at all times, and
gets a system that switches tasks in half a microsecond and is a joy to
program. Nether C partitions authority into a lattice and records the
partition in every type.

**What it cost.** Speed of expression. In HolyC you write what you mean. In
Nether C you write what you mean, discover it is at depth 5, and go back to
add a `descend`. Depth inference is what keeps this from being intolerable,
and if inference is not good enough the language is not usable.

### Everything mutable → nothing mutable

Both systems share everything. TempleOS lets every task write to it; Nether C
lets none write to anything.

**What it cost.** Every algorithm that wants an in-place update. §5.4 permits
building an aggregate before it is first read, which covers construction but
not, say, a hash table that is updated a million times. A Nether C program
that needs one will be slow, and the honest answer today is that such a
program should be written somewhere else and called at stratum 8.

### DolDoc → the ledger graph

TempleOS's file is alive: source, documentation, UI and runnable macros in one
illuminated hypertext, with sprites drawn inline. It is the most charming
thing about the system.

Nether C makes the *text* disposable and the *graph* canonical. The source is
one rendering of the program; the trace is what actually exists.

**What it cost.** Charm, so far. The ledger graph is a better substrate for
diffing, provenance and reproducibility, and it is currently much less
pleasant to look at than a DolDoc file. The Necropolis exists to close that
gap and has not yet been built, so this comparison is not yet one Nether C
wins.

### 640×480 forever → the Decay Rule

Terry fixed the dimensions of the system by covenant and never changed them.
Nether C's counterpart is that the trusted core may only ever get smaller —
checked in CI, recorded in `.decay-ceiling`.

A fixed constraint and a monotonically decreasing one are both ways of
refusing to let a system sprawl. The second is more demanding.

## 90.2 Rejected alternatives

### The Orpheus rule: two rejected forms

**Scope re-stain.** `look` raises the depth of the whole enclosing scope to
the shade's origin. Mythologically exact — turning around costs you everything
— and it produces errors nobody can read: one `look` deep inside a function
poisons every caller, and the diagnostic points at the caller rather than the
cause.

**Binding taint.** Only the bound value takes the depth. Tractable, familiar,
and it reduces `shade` to a newtype: if looking is free, nothing has been
carried.

The rule in [§1.6](01-strata.md#16-shade-and-the-orpheus-rule) — *you may only
look by going back down* — was chosen because it is a local check rather than
a propagation, gives an error that names the exact fix, and keeps the myth
intact. Orpheus could have kept her; what he could not do was look on the way
up.

### Stratum 8: refusing foreign code

The purer design has no stratum 8. Every value in every trace would then have
a complete history, unconditionally, and every claim in this specification
could drop its qualifier.

It was rejected because a language that cannot call C cannot be adopted, and
an unadopted language proves nothing about whether its ideas were good. The
quarantine in [§1.7](01-strata.md#17-stratum-8-the-unrecorded) is the
compromise: foreign code is permitted, and a trace that used it is permanently
forbidden from claiming to be replayable.

This remains the weakest part of the design.

### Fuel and `opaque` as command-line concerns

The obvious cheap answer: make the fuel budget a `--fuel` flag and the barrier a
`--no-inline` list, and keep both out of the language.

Rejected because a trace buried under a different budget is a different trace.
If the artifact depends on it, it is part of the artifact, and a thing that is
part of the artifact has to be visible in the source that produced it. A build
that succeeds on one machine and exhausts on another because somebody's shell
alias differed is precisely the class of failure this language exists to make
impossible.

The cost is a keyword. `opaque` is the only construct in Nether C that exists to
make the compiler do *less*, which is an odd thing to have to teach, and it will
be the first thing a newcomer mistakes for an optimisation hint.

### Depth as a monad stack

The obvious alternative to a lattice. Rejected because effects that compose by
`max` need no lifting, no transformers, and no ordering decisions, and because
`max` is a rule every programmer already holds in their head.

The cost is expressiveness: a lattice cannot distinguish *reads the disk* from
*reads the disk and the network* except by taking the deeper of the two. Nether
C accepts a coarser answer in exchange for one people will actually use.

### Floating point

Excluded from [§3.6](03-lexical.md#36-literals) because IEEE 754 has
platform-observable behaviour, and canonical encoding cannot be built on top of
it. This makes Nether C unsuitable for numerical work today, which is a real
and large exclusion.

## 90.3 Things Nether C is worse at

Stated plainly, because the roadmap requires at least one such statement and
because they are all true.

- **Interactive programs.** A language whose central act happens before the
  program is delivered is the worst possible fit for anything that responds to
  a person. Nether C should not be used for a UI, a game, or a server that
  holds a connection.
- **Numerics.** No floats. See above.
- **Debugging by print.** Two commands instead of one, forever.
- **Storage.** The ledger never shrinks. §7.6 argues that this is affordable;
  it has not yet been measured, and until it is that argument is a hope.
- **Learning curve.** Depth is a genuinely new thing to learn. HolyC's whole
  proposition was that you already knew it — it was C, plus permission.

## 90.4 On TempleOS

Nether C is an inversion of HolyC in the way a photographic negative is an
inversion: every value reversed, every edge in exactly the same place.

Terry Davis built a complete operating system, a compiler, a graphics stack, a
document format and a language, alone, and made every one of them follow from a
single stated idea. Whatever one makes of the idea, the coherence is the
achievement, and it is rarer than the code.

He was ill, and it is in the work. TempleOS is strange because he was strange,
and it is also technically excellent, and both of those are true at once and
neither one cancels the other. Treating the strangeness as the whole story
misses an operating system. Treating it as an embarrassment to be edited out
misses the person who wrote it. This document tries to do neither.

What is borrowed here is the method rather than the belief: build the thing
yourself, keep it small enough to hold in your head, follow one idea all the
way down even when it becomes inconvenient, and say plainly what you think is
true.

An inversion is a form of close reading. You cannot turn something over without
first working out which way up it was.
