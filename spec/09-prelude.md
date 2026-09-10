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

```c
I64   min(I64 a, I64 b)                @0;
I64   max(I64 a, I64 b)                @0;
I64   abs(I64 x)                       @0;

I64   len(Bytes b)                     @0;
Bytes slice(Bytes b, I64 from, I64 to) @0;
Bytes concat(Bytes a, Bytes b)         @0;
Bool  starts_with(Bytes b, Bytes p)    @0;

Str   utf8(Bytes b)                    @0;   // fails if not well-formed
Bytes raw(Str s)                       @0;
Str   join(Str[] parts, Str sep)       @0;
Str[] split(Str s, Str sep)            @0;

Cairn cairn_of(Bytes b)                @0;   // the same value `seal` would give
Str   hex(Cairn c)                     @0;
```

`compile` is not in the prelude. Nether C does not know how to compile Nether
C until [Self-Burial](../ROADMAP.md); until then it is an ordinary function a
program supplies for itself.

## 9.3 Stratum 1 — `store`

```c
Bytes fetch_node(Cairn c)              @1;
Bool  has_node(Cairn c)                @1;
```

Reading the ledger is at depth 1 rather than 0 because the ledger is a place
on a machine, and a program that reads it can fail in a way a pure program
cannot. The value it returns is nevertheless fully determined by the cairn,
which is why it is only one stratum down.

## 9.4 Stratum 2 — `env`

```c
Str   env(Str name)                    @2;   // fails if not declared
I64   clock()                          @2;   // the pinned build time
Str   target()                         @2;   // the target triple
```

`env` fails on an undeclared variable rather than returning empty. A build
that silently behaves differently because a variable was absent is exactly the
class of bug this language exists to make impossible.

The declaration lives outside the program, in the burial invocation. A trace
records the declared set and every value read from it.

## 9.5 Strata 3 and 4 — `disk`, `disk!`

```c
Bytes read(Str path)                   @3;
Str[] list(Str path)                   @3;
Bool  exists(Str path)                 @3;

U0    write(Str path, Bytes contents)  @4;
U0    remove(Str path)                 @4;
```

`read` becomes a hole; the answer is sealed and recorded before it reaches the
program. `write` is the first genuinely irreversible thing the language can
do, and the only prelude function whose witness records something the ledger
cannot later reproduce on its own.

## 9.6 Strata 5 and 6 — `net`, `net!`

```c
Bytes get(Str url)                     @5;
Bytes post(Str url, Bytes body)        @6;
```

The response is sealed on arrival, with the request as its witness. Replay
serves the recorded response and MUST NOT open a socket.

## 9.7 Stratum 7 — `entropy`

```c
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

```c
Bytes call_foreign(Str symbol, Bytes args) @8;
```

Marks the trace, permanently and transitively. Everything downstream of the
call is at depth 8 and the trace can no longer claim to be replayable.

An implementation MUST make this unpleasant to reach for — at minimum, naming
the symbol in `nether strata` output — because a stratum-8 call is a hole in
the record that no later care can fill in.

## 9.9 Failure

There are no exceptions and no error type in this draft. A prelude function
that cannot answer — `read` on a missing file, `utf8` on invalid bytes —
**starves**: it produces no value and the burial reports it, naming the span.

> This is under-specified and known to be. A language for build systems needs
> a way to say *try this, and if it fails do that*, and starvation as
> specified here has no recovery. The roadmap has no item for it yet; it needs
> one before [The Surface](../ROADMAP.md) lands.
