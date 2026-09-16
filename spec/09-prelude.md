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
Refusal refusal(Answer<T> a)           @0;   // which no was it? collapses if given
T       must(Answer<T> a)              @0;   // the value. collapses if refused
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
the program, and it collapses (§9.9).

Neither returns empty. A build that silently behaves differently because a
variable was absent is exactly the class of bug this language exists to make
impossible.

The declaration lives outside the program, in the exhumation that answers the
hole: [§8.3.1](08-rites.md#831-declaring-an-environment) is the invocation.

A trace records every value read from it, as a `Witness` like any other, and
does **not** record the declared set. It does not need to. A read produces a
witness; a refusal is an answer and is witnessed the same way; and a name that
was never declared collapses, which produces no trace at all. So everything the
declaration did to the program is already named by the trace, and what is left
over is a name the program never read — which changed nothing, and which two
cairns therefore must not be made to differ by.

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

A URL names a scheme, and an implementation MUST state which schemes it serves.
It is not required to serve all of them: `https` is TLS, and an implementation
that wrote its own would be writing its own cryptography. One that will not
serve a scheme MUST refuse it with `denied` rather than `unreachable`, because
those are different sentences — `denied` is this build saying no, and
`unreachable` is the world not answering. A trace would otherwise record "the
host was down" for a build that never dialled.

Where an implementation may reach at all is
[§8.3.2](08-rites.md#832-declaring-a-reach), and is a fact about the invocation
rather than about the language.

## 9.7 Stratum 7 — `entropy`

```signatures
Bytes draw(I64 n)                      @7;
```

`draw` is the only source of nondeterminism in the language, and it is nearly
the deepest thing in it.

> Entropy is stratum 7: the deepest recordable thing. Once drawn bytes affect
> a trace, the source alone is not enough to re-derive it. Replay needs the
> recorded answer.

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

What may be loaded at all is [§8.3.3](08-rites.md#833-declaring-what-may-be-loaded).

### 9.8.1 The other side

`sym` names a function with C linkage and this signature:

```
int32_t sym(const uint8_t *args, size_t args_len,
            uint8_t *out, size_t out_cap, size_t *out_len);
```

That is C, not Nether C. It is here the way a header file is here: to be read,
and written against.

It is handed the `Bytes` the program passed, and a buffer to write its answer
into. It MUST set `*out_len` to the length of its answer whether or not the
answer fit, and MUST NOT write more than `out_cap` bytes. It returns:

| Value | Meaning |
| ---: | --- |
| `0` | the answer is in `out`, and is `*out_len` bytes |
| `1` | `out_cap` was too small; `*out_len` is what is needed |
| `2`..`7` | a refusal, in the order [§5.1.1](05-types.md#511-answers-and-refusals) lists them |
| anything else | malformed |

On `1` the implementation MUST call once more with a buffer of at least
`*out_len`, and MUST NOT call a third time: a callee whose answer grows every
time it is asked is a callee that never finishes.

Neither side frees the other's memory. The buffer belongs to the caller for its
whole life, and `args` belongs to the caller too — a callee that keeps either
pointer after it returns has kept a pointer to something it does not own, and
no wording here can stop it. That is what stratum 8 *is*, and it is why §1.7
marks the trace rather than trying to make the call safe.

An implementation MUST NOT assume anything else about the callee. In
particular it MUST record the witness before the call, so that a callee which
does not return leaves a trace saying what was attempted.

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

### A collapse is a bug

An expression **collapses** when it cannot produce a value and never will:
`must` on a refusal, `refusal` on a given answer, an out-of-range slice, `env`
on a variable that was never declared.

A collapse is not catchable. There is no `rescue`, no `try`, no recovery form,
and there will not be one. A burial that hits one caves in: it stops and
reports the source span, because every way to collapse is a mistake in the
program rather than a fact about the world, and a mistake that can be caught
is a mistake that will be ignored.

A collapse is not a *starvation*, which is what an expression does while it
waits on a hole ([§6.4](06-evaluation.md#64-starvation-and-fuel)). That is
ordinary: a starved expression is residualised and finishes later, at somebody
else's exhumation. A collapsed one never finishes.

### What it costs

Every call site that touches the world gets a check, or an explicit `must` that
says a check is unnecessary. Code that reads six files reads as six answers.

That is the bargain C has always offered — `if (fp == NULL)` — and Nether C is a
C dialect, so it offers the same one rather than a better-looking one that hides
where the world can say no. The rejected alternatives are in
[§90.2](90-rationale.md#902-rejected-alternatives).
