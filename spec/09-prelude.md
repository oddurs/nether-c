---
section: "09"
title: The prelude
status: draft
---

# The prelude

Every function's signature carries the stratum it costs. Reading this section
is how a programmer finds out what a thing will do to their program's depth,
so nothing here may be deeper than it must.

The prelude is deliberately small. It should be readable in full in ten
minutes.

The listings below are a notation and not a program. A signature and a
semicolon is not something [section 04](04-grammar.md) parses — `func_decl`
wants a block — and there is no production for one because nothing a program
can write would supply a body for `read`. They are here the way a header file
is here: to be read.

## 9.1 Capabilities

These are the names `descend` accepts. An implementation MUST NOT define
others.

| Capability | Stratum | Grants |
| --- | ---: | --- |
| `store` | 1 | reading the ledger by cairn |
| `env` | 2 | the frozen environment |
| `disk` | 3 | reading files |
| `disk!` | 4 | creating and modifying files |
| `net` | 5 | fetching |
| `net!` | 6 | sending |
| `entropy` | 7 | true randomness |
| `unrecorded` | 8 | foreign code |

The `!` suffix marks the writing half of a pair, and reads as it does in the
rest of the language: this one does not take back.

## 9.2 Depth 0

```signatures
I64   min(I64 a, I64 b)                @0;
I64   max(I64 a, I64 b)                @0;
I64   abs(I64 x)                       @0;

I64   len(Bytes b)                     @0;
Bytes slice(Bytes b, I64 from, I64 to) @0;
Bytes concat(Bytes a, Bytes b)         @0;
Bool  starts_with(Bytes b, Bytes p)    @0;

Answer<Str> utf8(Bytes b)              @0;   // Refused malformed
Bytes raw(Str s)                       @0;
Str   join(Str[] parts, Str sep)       @0;
Str[] split(Str s, Str sep)            @0;

Cairn cairn_of(Bytes b)                @0;   // the same value `seal` would give
Str   hex(Cairn c)                     @0;

Refusal absent; denied; malformed;           // the six closed codes,
Refusal unreachable; exhausted; conflict;    // bound as prelude constants

Bool    given(Answer<T> a)             @0;   // did the world say yes?
Refusal refusal(Answer<T> a)           @0;   // which no was it? starves if given
T       must(Answer<T> a)              @0;   // the value. starves if refused
```

`given` and `refusal` are how a program handles a no. `must` is how it declares
it will not: *this cannot be refused, and if it is, that is a bug and I want the
burial to stop.* It is the right thing to write when a refusal would mean the
program's assumptions were already wrong, and the wrong thing to write anywhere
else.

There is no pattern matching in this draft, so an answer is inspected the way
C has always inspected one:

```c
Answer<Bytes>@3 a = descend disk { read("main.nc") };

if (given(a)) {
  Bytes@3 src = must(a);
  // ...
} else if (refusal(a) == absent) {
  // ...
}
```

`compile` is not in the prelude. Nether C does not know how to compile Nether
C until [Self-Burial](../ROADMAP.md); until then it is an ordinary function a
program supplies for itself.

## 9.3 Stratum 1 — `store`

```signatures
Answer<Bytes> fetch_node(Cairn c)      @1;   // Refused absent
Bool          has_node(Cairn c)        @1;
```

Reading the ledger is at depth 1 rather than 0 because the ledger is a place
on a machine, and a program that reads it can fail in a way a pure program
cannot. The value it returns is nevertheless fully determined by the cairn,
which is why it is only one stratum down.

## 9.4 Stratum 2 — `env`

```signatures
Answer<Str> env(Str name)              @2;   // Refused absent
I64         clock()                    @2;   // the pinned build time
Str         target()                   @2;   // the target triple
```

`env` distinguishes two different mistakes. Reading a variable that was
**declared and is not set** is a refusal: the world was asked and said no.
Reading a variable that was **never declared** is not a refusal, it is a bug in
the program, and it starves (§9.9).

Neither returns empty. A build that silently behaves differently because a
variable was absent is exactly the class of bug this language exists to make
impossible.

The declaration lives outside the program, in the burial invocation. A trace
records the declared set and every value read from it.

## 9.5 Strata 3 and 4 — `disk`, `disk!`

```signatures
Answer<Bytes> read(Str path)           @3;   // absent, denied
Answer<Str[]> list(Str path)           @3;   // absent, denied
Bool          exists(Str path)         @3;

Answer<U0> write(Str p, Bytes c)       @4;   // denied, exhausted
Answer<U0> remove(Str path)            @4;   // absent, denied
```

`read` becomes a hole; the answer is sealed and recorded before it reaches the
program. `write` is the first genuinely irreversible thing the language can
do, and the only prelude function whose witness records something the ledger
cannot later reproduce on its own.

## 9.6 Strata 5 and 6 — `net`, `net!`

```signatures
Answer<Bytes> get(Str url)             @5;   // unreachable, denied, absent
Answer<Bytes> post(Str url, Bytes b)   @6;   // unreachable, denied, conflict
```

The response is sealed on arrival, with the request as its witness. Replay
serves the recorded response and MUST NOT open a socket.

## 9.7 Stratum 7 — `entropy`

```signatures
Bytes draw(I64 n)                      @7;
```

`draw` is the only source of nondeterminism in the language, and it is nearly
the deepest thing in it.

> In TempleOS, entropy was the Oracle — revelation, arriving from above. Here
> it is stratum 7: the deepest recordable thing, and the one act that means a
> program can never be re-derived, only replayed. The inversion is exact and
> it is the reason the lattice is oriented the way it is.

A trace that called `draw` records the drawn bytes as a witness, so replay is
exact. What is lost is re-derivation: burying the same source again produces a
different trace. See [§6.7](06-evaluation.md#67-replay).

## 9.8 Stratum 8 — `unrecorded`

```signatures
Answer<Bytes> call_foreign(Str sym, Bytes args) @8;
```

Marks the trace, permanently and transitively. Everything downstream of the
call is at depth 8 and the trace can no longer claim to be replayable.

An implementation MUST make this unpleasant to reach for — at minimum, naming
the symbol in `nether strata` output — because a stratum-8 call is a hole in
the record that no later care can fill in.

## 9.9 Failure, and the difference between two of them

Nether C has no exceptions and no unwinding. Unwinding has no meaning in a
language where evaluation is demand-driven and partially staged, and a stack
that can be unwound is a stack that can be observed.

It has two things instead, and the whole of failure handling is knowing which
one you are looking at.

### A refusal is an answer

When the world is asked a question it is entitled to answer no to — the file is
not there, the response never came, the bytes are not UTF-8 — that no **is the
answer**. It is a value, of type `Answer⟨T⟩`, and it is recorded as a witness
exactly like a yes.

This is not a concession to practicality. It follows from
[§1.4](01-strata.md#14-what-the-trace-records): everything the world says gets
written down before the program sees it. A missing file is something the world
said. Replay serves it back, and a program that handled it replays identically.

### Starvation is a bug

An expression **starves** when it cannot produce a value and never will:
`must` on a refusal, `refusal` on a given answer, an out-of-range slice, `env`
on a variable that was never declared.

Starvation is not catchable. There is no `rescue`, no `try`, no recovery form,
and there will not be one. A starved burial stops and reports the source span,
because every way to starve is a mistake in the program rather than a fact
about the world, and a mistake that can be caught is a mistake that will be
ignored.

> Starvation on a *hole* is a third and different thing: an expression that
> cannot be evaluated yet because the world has not been granted. That is
> ordinary, it is what [§6.4](06-evaluation.md#64-starvation-and-fuel)
> describes, and it resolves by exhumation rather than by handling.

### What it costs

Every call site that touches the world gets a check, or an explicit `must` that
says a check is unnecessary. Code that reads six files reads as six answers.

That is the bargain C has always offered — `if (fp == NULL)` — and Nether C is a
C dialect, so it offers the same one rather than a better-looking one that hides
where the world can say no. The rejected alternatives are in
[§90.2](90-rationale.md#902-rejected-alternatives).
