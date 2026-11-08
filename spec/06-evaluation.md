---
section: "06"
title: Burial, holes and replay
status: draft
---

# Burial, holes and replay

## 6.1 Burial

**Burial** is the only form of evaluation Nether C has. It takes source and a
set of granted capabilities, and produces a **trace**.

```
bury : Source × Capabilities × Fuel → Trace
```

Burial evaluates every expression it can and stops at every expression it
cannot. It can evaluate a call when the capability that grants the call's
stratum is among the granted ones and every argument has a value. It cannot
when the world has not granted that capability, and every such point becomes a
**hole**.

Granted capabilities are a set and one grants no other, on
[§2.1](02-calculus.md#21-judgement-form)'s reasoning. A burial holding `net`
does not answer a `read`: the read is a hole owed to `disk`, and an exhumation
that grants `disk` is what fills it.

A grant gates a **call**, and not a depth. An expression that reaches no
prelude function is evaluated whatever depth it has, because there is nothing
for the world to answer:

```console
$ nether bury peek.nc
buried   peek.nc → f51bdfe5   depth 0   holes 0   2 nodes
```

`peek.nc` is `demand descend disk { 1 + 1 };`. It holds the disk and asks it
nothing, so it is 2, and no capability was granted to get there. This is worth
saying outright because it decides what
[§1.6](01-strata.md#16-shade-and-the-orpheus-rule)'s `look` costs: `look s` on
a shade that already has a value reaches nothing either, and burial evaluates
it with nothing granted. What the Orpheus rule buys is settled there.

A trace is complete and immutable. Burial MUST NOT modify a trace it was
given; every burial produces a new trace with a new cairn.

## 6.2 Demand

There is no `main`. A file's top level is a sequence of declarations and
`demand` statements, and nothing is evaluated except what a `demand`
transitively requires.

```c
Bytes@3 src = must(descend disk { read("main.nc") });
Bytes   obj = compile(src);      // pure, but starves: src is a hole
Bytes   unused = compile(other); // never evaluated: nothing demands it

demand obj;
```

`must` here says *this file is not allowed to be missing*. A program that wants
to handle a missing file inspects the `Answer` instead
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)).

Evaluation is demand-driven. The implementation MUST NOT evaluate any
expression that no `demand` depends on, transitively — not as an optimisation
but as a semantic guarantee, because evaluating an undemanded expression could
reach the world and leave a witness for something the program never asked for.

Where several `demand`s exist, they are evaluated in source order. Within a
single demand, evaluation is **left to right and innermost first**:

- a call evaluates the expression naming the function, then its arguments in
  order;
- an operator evaluates its left operand and then its right;
- a block evaluates its statements in order and then its tail;
- `if` evaluates its condition and then only the arm the condition chose,
  which is also what `&&` and `||` are ([section 04](04-grammar.md));
- a loop evaluates its body and then its step, once per turn;
- every other construct evaluates the one subexpression it has.

This is specified rather than left to the implementation because burial puts
the order in the trace three times over. The order holes were discovered in is
a field of the trace ([§7.3](07-ledger.md#73-nodes)). Which of several places
asked a shared question first decides the span the hole keeps
([§6.3](#63-holes)). And which node is reached first decides where a fuel
budget runs out ([§6.4](#64-starvation-and-fuel)), since a step is charged when
evaluation of a node begins. Two burials of the same source and the same
capabilities MUST produce the same trace, and a trace that holds the order
cannot do that while the order is anyone's business.

### 6.2.1 Burying in parallel

The order above is the order a trace records, not an order evaluation has to
happen in. An implementation MAY evaluate on several threads, out of order, or
ahead of need, on one condition:

> **Serial equivalence.** A parallel burial MUST leave exactly the trace the
> order above would, and where that order would halt, it MUST halt with the
> same diagnostic at the same place.

What that fixes, because each is part of a trace or of what a burial reports:

- **Holes**, in the order the serial evaluation discovered them, and for a
  shared question the span of the place that asked it first ([§6.3](#63-holes)).
- **Deposits**, in the order they were made ([§8.4](08-rites.md#84-lamp)).
- **Witnesses**, in the order the world answered ([§6.6](#66-exhumation)).
- **The halt.** If the serial order would collapse before it ran out of fuel,
  the burial collapses there; if it would run out first, it runs out at the
  step the serial order would have reached ([§6.4](#64-starvation-and-fuel)).
  A collapse found ahead of need on another thread is not one the program
  reached until the serial order reaches it.
- **The steps.** The rite reports the steps the serial order spends, whatever
  the threads spent between them.

What is free:

- Pure work in any order, on any thread, including work that is thrown away.
  Evaluating ahead of need is §6.5's speculation by another name, and the same
  three refusals keep it invisible: it forms no hole, deposits nothing and
  collapses nothing until the serial order commits to it.
- The order objects reach the store. A store is a set
  ([§7.5](07-ledger.md#75-the-store)), and two writes of one object are one.

Nothing that reaches the world is free. A burial holds no capability, and
`exhume` asks the world in the order its holes are recorded; asking two at once
is a matter for [§6.6](#66-exhumation), not for this section.

```c
// order.nc — the order a burial leaves, which a parallel one must leave too.
// spec/06-evaluation.md §6.2.1.
//
// `b` is declared first and `a` is asked for first, so the holes are in the
// order the demand reached them and not the order they were written. The
// deposits are in demand order, with a starved demand between them.
Bytes@3 b = must(descend disk { read("b.txt") });
Bytes@3 a = must(descend disk { read("a.txt") });

U0 first() { "one\n"; }
U0 second() { "two\n"; }

demand first();
demand concat(a, b);
demand second();
```

```console
$ nether bury order.nc
buried   order.nc → 22644e59   depth 3   holes 2   10 nodes
  hole ①  read("a.txt")                  stratum 3  disk
  hole ②  read("b.txt")                  stratum 3  disk
$ nether lamp 22644e59
one
two
```

`b` is declared first and `a` is asked for first, so `a` is hole ①. The
deposits are in demand order, with a demand that starved between them. A
parallel burial that discovered `b` first, or deposited `two` first, has
written a different trace.

## 6.3 Holes

A **hole** is a suspended world-question. An implementation MUST record, for
each hole:

| Field | Meaning |
| --- | --- |
| `call` | the prelude function and its fully-evaluated arguments |
| `stratum` | the depth the call would reach |
| `span` | the source location that asked |

A hole does **not** record what it depends on. It cannot depend on anything: a
hole is only formed once every one of its arguments is a finished value, so
nothing it needs can still be waiting on another hole. A field for it would be
empty in every trace any burial could produce.

A hole does **not** record its dependents. It cannot: a node is immutable and
named by its content, so a hole that listed the things waiting on it would get
a new name every time something else came to wait, and every reference to the
old name would be to a hole that no longer exists.

Which nodes are suspended on a hole is derived by reading the graph backwards,
the same way provenance is, and for the same reason
([§7.4](07-ledger.md#74-provenance)).

A hole's arguments are themselves fully buried. `read(concat(dir, name))` does
not leave a hole containing `concat`; it leaves a hole containing the finished
path string. A hole is always a question the world can answer immediately.

A hole is answered by whatever the world says, **including a refusal**. A
refusal is an ordinary witness: it is recorded, it is served back on replay, and
a trace whose program handled it replays identically to one whose program did
not. Refusal does not make a trace incomplete and does not mark it.

Two holes with identical `call` fields **at a read stratum** in the same trace
MUST be the same hole. The read strata are 1, 2, 3 and 5. This is what makes
exhumation cheap: reading the same file twice is one question, asked once.

Such a hole keeps the `span` of the first place that asked, in the order
[§6.2](#62-demand) fixes. Which place that is, is therefore a fact about the
program rather than about the implementation, and two burials of the same
source agree on it. A hole is a question for the world, and the world does not
care how many places were waiting on the answer.

Holes at 4, 6, 7 and 8 are **never** merged, however identical their calls.
[§1.8](01-strata.md#18-why-write-is-deeper-than-read) is the reason and it is
the whole of it: those strata are ordered where they are because of what they
disturb rather than what they learn, and a thing that cannot be taken back
cannot be done once and counted twice. Two `post`s to the same address with
the same body are two messages, and §1.1 prices stratum 6 at *the world
remembers what was said*. Two `draw`s are two draws, or `draw` is a pure
function of its argument and [§9.7](09-prelude.md#97-stratum-7-entropy) is wrong to call
it the only source of nondeterminism in the language. `write` and `remove` are
the arguable ones — writing the same bytes twice changes nothing the second
time — and they are not merged either, because a trace that describes fewer
acts than the program performed is a trace that lies about what was done.

So a call can be asked more than once, and then it is answered more than once:

> The answers to one call are served in the order [§6.2](#62-demand) fixes —
> the *n*th hole that asks it takes the *n*th answer recorded for it. An
> implementation MUST NOT address an answer by the place that asked. A span is
> a fact about one text, a residue is printed and lowered again
> ([§6.5](#65-residue)), and an answer that could not survive that would break
> the staging law wherever the world had already said something.

At a read stratum there is one hole and one answer, and this says nothing new
about it.

## 6.4 Starvation and fuel

An expression **starves** when it cannot be evaluated and cannot become a hole
— because it depends, transitively, on a hole. Starvation is normal; a starved
expression is residualised and waits.

An expression that can never produce a value has not starved. It has
**collapsed**, and a burial that hits one caves in rather than waiting
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)).

Burial takes a **fuel budget**: a bound on the number of evaluation steps.

A **step** is one evaluation of one expression node. It is charged when
evaluation of that node begins, before anything else happens to it, and it is
charged once. Three consequences, because each is a place two implementations
could otherwise disagree:

- A unit-level binding is evaluated at most once, however many times it is
  named, and the steps its value costs are charged to the first demand that
  reaches it.
- `opaque e` costs one step and `e` costs nothing, because `e` is not
  evaluated.
- A residue costs nothing for what has already been reduced, which is what
  makes the staging law in [§6.5](#65-residue) affordable rather than merely
  true.

Fuel accounting MUST be deterministic — the same source and capabilities must
exhaust at exactly the same point on every implementation and every machine.
That is a requirement about agreement between implementations and not only
with oneself, which is why the step is defined here rather than left to one.
It rests on the evaluation order in [§6.2](#62-demand) and not only on the
definition of a step: a step is charged when evaluation of a node begins, so
where a budget runs out is which node was reached first.

Exhausting fuel is a diagnostic, not a crash. The implementation MUST report
the source span at which fuel ran out and the shape of what was being
evaluated:

```
error: burial ran out of fuel after 1,000,000 steps
  --> lib/table.nc:31:3
   |
31 |   for (I64 i = 0; i < n; i++)
   |   ^^^ unrolled 262,144 times; `n` is a literal, so this will not stop
   |
   = raise the budget with --fuel, or place an `opaque` barrier at line 29
```

Burial has one other bound, and it is the implementation's own. Fuel is a
bound on work; it is not a bound on space, and no single budget is both — a
program can spend a million steps a thousand frames deep or a million steps
two frames deep. So an implementation has limits of its own: how deep a chain
of calls it can hold, how large a value it can address.

Reaching one MUST be reported the way exhausted fuel is — naming the limit,
its value, and where it was reached — and MUST NOT be a crash. An
implementation MUST state its limits.

Two implementations with different limits may therefore disagree about whether
a given program buries at all. They MUST NOT disagree about the result when
both of them finish, and that is the sentence the reproducibility claims in
this document actually rest on.

`opaque e` evaluates to `e` but is never burned through: burial residualises
it whole. It is how a programmer says *do not evaluate through this, even
though you could* — for a loop that would unroll into a gigabyte, or a
function whose specialised form would be larger than its general one.

`opaque` has a typing rule ([OPAQUE], [section 02](02-calculus.md)) rather than
being a compiler flag, because whether an expression is burned through changes
the artifact, and anything that changes the artifact belongs in the language.

This is **settled**. The rejected alternative — leaving both the budget and the
barrier to the command line — is recorded in
[§90.2](90-rationale.md#902-rejected-alternatives).

## 6.5 Residue

What survives burial is the **residue**: the program that is left once
everything that could be evaluated has been.

A residue is a **program**, not a set of nodes. It has branches, loops,
bindings and blocks in it — a `while` whose condition waits on a hole
residualises whole, and so does the `if` around it — and none of those is a
thing [§7.3](07-ledger.md#73-nodes) knows how to store. Nor should it: a node
records something that *happened*, and a residue is something that has not
happened yet.

So a residue is written down the way programs are written down. It is source,
in the syntax of [section 04](04-grammar.md), held in the ledger as an ordinary
`Bytes` value and named like any other. Burying it again means lexing, parsing
and lowering it again, which is exactly what burying anything means.

> An implementation MUST be able to print any residue it can produce, and
> lowering what it printed MUST give back the same program. Without that the
> staging law below is unprovable, because the second burial would not be
> burying the first one's residue.

The residue is a complete program. It can be buried again, with more
capabilities, and the result is the same as if those capabilities had been
granted the first time:

> **Staging law.** For capability sets *A* and *B*,
> `bury(bury(p, A), B) ≡ bury(p, A ∪ B)`.

An implementation MUST satisfy this. It is what makes descent incremental
rather than a single all-or-nothing act, and it is the property `nether graft`
relies on.

> Earlier drafts of this section counted nine hundred nodes for a program with
> one hole in it. That number was the residue being counted as nodes, which is
> the reading [§6.5](#65-residue) rejects. A trace names its residue and does
> not contain it, so the count is small: the transcripts in
> [§8.2](08-rites.md#82-bury) are what an implementation actually prints.

**A call that starved.** Burial reduces applications ([APP],
[section 02](02-calculus.md)), and an application can fail to produce a value
in two ways. They residualise differently, and the difference is what a residue
can be specialised to.

An argument that is still waiting takes the call with it. The residue is the
call as it was written, its arguments reduced as far as they went:
[§6.2](#62-demand)'s `compile(src)` is `compile(src)`, because inlining the
body would only put the same hole inside it.

When every argument is a value, burial evaluates the body. If the body
finishes, the call is what it produced. If the body starves, the residue is
**the body, reduced as far as those arguments permitted**, and not the call.
The arguments were all values, so the reduced body needs none of them: it is
written down as a function of no parameters and the call site names that.

> An implementation MUST residualise a starved call whose arguments are all
> values as its reduced body, except where
> [a residue keeps its depths](#a-residue-keeps-its-depths) says the call
> stays. Leaving the call as written is also a program and also correct, and
> it is not a residue: the next burial would redo the work this one already
> did, and nothing could ever be specialised to anything.

A block reduces its statements in order. A statement that finished and bound
nothing is not in the residue — it has already happened, and what it deposited
is already in the trace. Once a statement starves, that statement stays in
place, with whatever it bound left as an unknown, and the statements and tail
after it are reduced speculatively in place under the rules below. Statement
order is kept: a `return` after a starved statement stays after it with its
operand reduced, and nothing after that `return` is in the residue.

**What this is for.** Bury an interpreter against a program it is to
interpret, with the program's own questions unanswered, and the residue does
not name the interpreter's entry point. The syntax read before the first
unanswered question is not read again. That is the first Futamura projection,
and it is a consequence of this section rather than a feature of its own: a
specialiser is what a partial evaluator is when it is pointed at an
interpreter.

### Reducing under an undecided branch

An `if` whose condition waits on the world residualises whole. Both arms
survive, because which one runs is a thing only the answer settles. But the
arms may be **reduced** where they stand, and so may the statements after a
starved statement in a block, and an implementation SHOULD reduce both: without
them a first projection stops at the first binding of an unanswered value or
the first branch on it, which for an interpreter is the first one it reaches.

Reduction in both places is **speculative**, and a speculative
reduction may produce a value or it may stop. It MUST NOT do anything else.
Three things in particular:

1. It MUST NOT form a hole. A world-question inside an undecided arm stops
   where it is and residualises, even where the other arm asks the same
   question and a hole for it already exists
   ([§6.3](#63-holes)). A trace that recorded it would be owed an answer to
   something the program has not asked.
2. It MUST NOT deposit. A deposit is the program saying *this value matters*
   ([§4.7](04-grammar.md#47-the-bare-expression-statement)), and an arm that
   may not run has not said it.
3. It MUST NOT collapse. An out-of-range slice in an arm the world never takes
   is a mistake nothing reaches, and a burial that caved in on it would make
   whether a program buries depend on how far a specialiser got — which is what
   [§90.2](90-rationale.md#902-rejected-alternatives) refused when it refused
   `rescue`. The reduction stops there and the arm residualises as it stands.

So speculation folds, and nothing it does is visible in the trace. What it
leaves is a smaller program inside the same branch.

This does not weaken [§6.2](#62-demand), which is the guarantee it looks like
it touches. §6.2 forbids evaluating what no demand requires *because such an
expression could reach the world*; the three clauses above are that reason,
made into a rule. A reduction that cannot ask, cannot deposit and cannot
collapse leaves nothing behind for §6.2 to be protecting.

Nor does it disturb the staging law. Reducing an arm does not change which arm
is taken, so the composite trace is the one a single burial would have written.
What differs is fuel, and a budget is not in a trace
([§8.2](08-rites.md#82-bury)) precisely because it is not part of what was
produced.

**What it costs.** A specialised body can be larger than the general one, and
there is one per call site that starved, each carrying the constants it was
specialised to. Speculation adds to that: both arms of an undecided branch are
reduced and only one will ever be taken, so the work is charged for a path that
may not exist and the residue carries both.

The sharper cost is that a program which buries today may stop burying. An arm
that never terminates is reduced until the budget runs out, and fuel exhausted
is a halt with no trace at all ([§6.4](#64-starvation-and-fuel)) — where before
the arm simply residualised and nobody noticed.

That is a smaller loss than it reads as. Such a program only buried because the
world answered the way that avoided the arm; had it answered the other way, the
burial would not have finished either. Speculation turns a non-termination that
was conditional into one that is certain, which is a bug found rather than a
bug introduced.

`opaque` ([§6.4](#64-starvation-and-fuel)) is the control for the case where it
was meant, and is why that sentence is in §6.4. It is also the only control:
there is no construct that makes burial reduce *more*, because everything here
already reduces as far as it can and the one direction worth asking for is
less.

### A residue keeps its depths

A residue is a program, so it is checked like one, and it MUST check wherever
the program it came from checked. Reduction may lower a depth
([§2.4](02-calculus.md#24-metatheory)), and most of the time that is the point.
[LOOK] is where it is not. Its premise is membership, `d ∈ δ`, not a ceiling
([§1.6](01-strata.md#16-shade-and-the-orpheus-rule)), and a depth that falls
can fall out of what a `look` holds:

```c
Bytes@5 g(Bool c) {
  Bytes x;
  if (c) { x = must(descend net { get("u") }); }
  else   { x = must(descend disk { read("f") }); }
  descend net { look shade x }
}
demand g(false);
```

One arm writes `x` from 5 and the other from 3, so `x` is 5 deep, the shade
comes from 5, and `descend net` holds 5. The condition is decided, and taking
the `else` arm leaves an `x` written only from 3. `net` does not hold 3. The
program checks and the residue that reduction would leave does not.

The depth reaches the shade by more than one path. A local can lose the deeper
of the arms that wrote it, as here. A call specialised to a constant can return
less deeply than its function says. And in a unit built other than from §04
source, a branch can sit under the shade itself. So the rule is stated about
depths, not about `shade`:

> A residual that is not a value MUST NOT be shallower than the code it
> replaced. Where reducing would make it so, the code stays as written, with
> what it reads assigned first as in the rest of this section:
>
> 1. A branch whose condition is decided stays whole, its condition reduced,
>    when the arm not taken is deeper than the arm taken, or writes a local
>    more deeply than the arm taken does.
> 2. A starved call whose reduced body would return less deeply than the
>    function stays the call, its arguments reduced.

A value is exempt. It is pure, and stratum 0 is in every δ
([section 02](02-calculus.md)), so no `look` can be refused for one.

**What it costs.** Less is specialised. A branch decided by a constant, with
arms of different depths, is no longer folded, and a call that the constant
would have made shallower is no longer minted. For an interpreter, that is a
branch on a guest's syntax whose arms reach the world differently. The residue
is larger, and the next burial decides the same branch the same way and leaves
it the same, so the staging law is untouched.

The rejected forms of this rule are in
[§90.2](90-rationale.md#a-residue-that-lowers-a-depth).

## 6.6 Exhumation

**Exhumation** grants a capability, answers the holes it can, records every
answer, and buries the residue again.

```console
$ nether bury build.nc
buried   build.nc → 76c4b655   depth 3   holes 1   4 nodes
  hole ①  read("main.nc")                stratum 3  disk

$ nether exhume 76c4b655 --grant disk --root .
  ①  read("main.nc")  →  29 bytes  6940fecb
sealed   76c4b655 + 6940fecb → bb9ed429   depth 3   holes 0
```

The result is a new trace with a new cairn. The original trace still exists,
unchanged, and still has its hole. Nothing in the ledger is ever revised.

The new trace **names** the witnesses it recorded
([§7.3](07-ledger.md#73-nodes)). It has to: a witness records the call it
answered and not the hole it answered, so a witness nothing names is a witness
nothing can find, and [§6.7](#67-replay) is re-burying with only the answers
already recorded.

A trace with no holes is **sealed**. A sealed trace has a value.

### Answering in parallel

A trace's `holes` list is in the order [§6.2](#62-demand) discovered them, and
the new trace's `witnesses` list MUST record the answers in that same order. An
implementation MAY answer a contiguous run of read-stratum holes (strata 1, 2,
3 and 5) concurrently, under a fixed bound on how many questions are in flight
at once, provided that:

1. No hole at an act stratum (strata 4, 6, 7 and 8) overlaps any other hole. An
   act is answered alone, in sequence, after every hole before it in the trace
   has been answered and before any hole after it is asked, so a `read` never
   races a `write` or `remove` in the same exhumation and two `post`s or
   `draw`s never reorder.
2. Every answer is written to the ledger before it is returned
   ([§1.4](01-strata.md#14-what-the-trace-records)), and the witnesses and
   answers are committed to the trace and to the resumed burial in the order
   the holes were listed, whatever order the threads finished in.
3. If a hole fails with a collapse or an unavailable provider, the rite stops
   at the first such hole in discovery order, cancels any hole after it in the
   run that has not yet been asked, and reports that first failure without
   writing a new trace.

### Answering across machines

A trace's open questions MAY be partitioned across isolated machines whose
stores exchange verified objects ([§7.5.3](07-ledger.md#753-a-second-store-and-what-they-owe-each-other),
[§7.5.4](07-ledger.md#754-moving-a-store)). Content addressing verifies the
objects exchanged; it does not decide which question a witness answered or
reconcile two machines that observed different answers from the world. An
implementation that distributes hole answering across workers and merges their
results into one trace MUST obey four rules:

1. **Assignment by hole slot.** A trace's `holes` list is ordered by discovery
   ([§6.2](#62-demand)), and at act strata (`4`, `6`, `7` and `8`) two entries
   in `holes` may share the same `call` or the same `Hole` cairn
   ([§7.3.1](07-ledger.md#731-node-encoding)). A worker assignment for a base
   trace `T` is therefore a subset of **slot indices** `i` in `0..T.holes.len()`,
   not a set of `Call`s or deduplicated `Hole` cairns. A worker answers its
   assigned slots in ascending slot order, obeying the act-stratum sequencing
   and failure rules of [§6.6](#answering-in-parallel) within its assignment,
   records each answer and `Witness` node in its own store before returning
   ([§1.4](01-strata.md#14-what-the-trace-records)), and delivers `(i, w)` pairs
   naming the slot index `i` and the recorded `Witness` cairn `w` alongside its
   store objects.
2. **Answer identity.** A delivered pair `(i, w)` is valid for `T` if and only
   if `i < T.holes.len()`, `T.holes[i]` decodes from the store as
   `Node::Hole { call, stratum, span }`, `w` decodes from the store as
   `Node::Witness { stratum: w_stratum, call: w_call, answer, span: w_span }`,
   `answer` decodes from the store as a `Value`, and
   `(w_stratum, w_call, w_span) == (stratum, call, span)`. If any of those
   objects is absent or corrupt in the merging store (for example, after an
   interrupted transfer), or if `w` does not match `T.holes[i]`, merging MUST
   fail rather than skip the slot.
3. **Duplicate delivery and conflicts.** If two deliveries (from a worker retry
   or overlapping assignments) supply the same `(i, w)` — identical slot `i` and
   identical `Witness` cairn `w` — the duplicate delivery is idempotent and
   accepted once. If two deliveries supply `(i, w_1)` and `(i, w_2)` with
   `w_1 != w_2` for the same slot `i`, or if at a read stratum (`1`, `2`, `3`
   or `5`) two slots with the same `call` were given different `answer` cairns,
   the workers observed conflicting answers. Merging MUST reject the conflict,
   naming the slot and both witness cairns, rather than picking one by arrival
   order.
4. **Deterministic merge order and capability-free assembly.** Merging holds no
   world capabilities ([§6.7](#67-replay)). Starting from `T.witnesses`, it
   appends the verified witness `w_i` for each answered slot `i` in strict
   ascending slot order `0..T.holes.len()`, regardless of the order in which
   workers finished or delivered their bundles, feeds each answered slot's
   `(call, value)` in that same ascending slot order to the resumed burial of
   `T.residue`, and leaves any unanswered slot open.

When the union of worker assignments covers `0..T.holes.len()` with the same
recorded answers as a single serial machine, the merged trace is byte-identical
to the serial trace and satisfies [§6.7](#67-replay)'s replay law. No equality
is promised when workers independently sample nondeterministic or mutable worlds
without agreeing on the recorded answers, and content addressing does not
prevent a retried act-stratum worker from repeating an external effect before
its witness reaches the coordinator.

## 6.7 Replay

```console
$ nether exhume bb9ed429 --replay
identical.
```

Replay re-buries a trace using only the answers already recorded in the
ledger. The implementation MUST hold no capabilities at all during replay: it
does not prefer the ledger over the world, it cannot reach the world.

> **Replay law.** Replaying a sealed trace produces a trace with the same
> cairn, unless the original trace is marked as having reached stratum 8.

This is why a trace does not record what its burial spent
([§8.2](08-rites.md#82-bury)). Replay buries a residue that is already folded,
which costs less than folding it did, so a trace holding that number could
never come back as itself.

"Identical" means byte-identical under the canonical encoding of
[section 07](07-ledger.md) — not merely equal in value. A trace that replays to
a different cairn is a bug in the implementation, and `nether exhume --replay`
MUST report the first differing node rather than only the mismatch.

Stratum 7 (entropy) does not break replay: the drawn value is a recorded
witness like any other, and replaying serves it back. What entropy breaks is
*re-derivation* — burying the same source again produces a different trace —
which is a different and weaker property that Nether C does not promise for
programs that reached stratum 7.

Stratum 8 breaks replay outright, because there is no witness to serve back.
See [§1.7](01-strata.md#17-stratum-8-the-unrecorded).

## 6.8 What burial prints

Burial prints a summary of what it wrote, and nothing else:

```
buried   <source> → <cairn>   depth <n>   holes <n>   <n> nodes
```

It MUST NOT print any value the program deposited. A program's deposits are
read with `nether lamp` ([section 08](08-rites.md)), by a person who decided to
go and look. A Nether C program cannot leak to a log: it holds no capability
that reaches one.
