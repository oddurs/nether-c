---
section: "02"
title: The calculus of depth
status: draft
---

# The calculus of depth

This section is the formal core. It fits on one page, and that is a
constraint rather than an observation: a depth system that cannot be stated on
one page cannot be taught, and a lattice nobody can hold in their head will be
worked around rather than used.

## 2.1 Judgement form

```
Γ ; δ ⊢ e : τ @ d
```

Read: *in context Γ, holding capabilities down to depth δ, the expression e
has type τ and depth d.*

- **Γ** maps identifiers to depth-annotated types, `x : τ@d`.
- **δ** is the **ambient depth**: the deepest stratum whose capability is
  currently held. It is 0 at the top level of a file and is raised only by
  `descend`.
- **d** is the **value depth**: how far into the world this value's history
  reaches.

`d ≤ δ` is required where it is written, which is the premises of [APP] and
[LOOK], and an implementation MUST reject any program in which either fails.
It is not a property of every judgement: [DESCEND] is precisely the rule that
concludes at the ambient depth it raised, so
`descend disk { read(p) }` has depth 3 in a scope whose ambient depth is 0.
What holds everywhere is [§2.4](#24-metatheory).

Function types carry two depths, written `τ₁ --dƒ--> τ₂@d_r`.

- **dƒ** is the **latent depth**: what a caller must already hold to apply it.
  In source syntax it is the trailing annotation on the signature
  (`Bytes read(Str path) @3`).
- **d_r** is the depth of what comes back. It rides on the return type, where
  it is written at all (`Bytes@3 load(Str path)`), and like every other depth
  it is inferred unless written.

Both are needed, because they are different numbers. A function that does its
own descending asks its caller for nothing and still hands back something
deep.

## 2.2 The rules

```
                x : τ@d ∈ Γ
  [VAR]     ───────────────────
              Γ ; δ ⊢ x : τ@d


  [LIT]     ─────────────────────        ℓ a literal
              Γ ; δ ⊢ ℓ : τ@0


              Γ ; δ ⊢ eᵢ : τᵢ@dᵢ      (i = 1..n)
  [PRIM]    ────────────────────────────────────────
              Γ ; δ ⊢ ⊕(e₁..eₙ) : τ@max(d₁..dₙ)


              Γ ; δ ⊢ f : (τ₁ --dƒ--> τ₂@d_r)@d_f      Γ ; δ ⊢ a : τ₁@d_a
              dƒ ≤ δ
  [APP]     ────────────────────────────────────────────────────────────
              Γ ; δ ⊢ f a : τ₂ @ max(d_r, d_f, d_a)


              Γ, x : τ₁@0 ; dƒ ⊢ b : τ₂@d₂       dƒ least
  [ABS]     ──────────────────────────────────────────────────
              Γ ; δ ⊢ λx.b : (τ₁ --dƒ--> τ₂@d₂)@0


              Γ ; max(δ, s(κ)) ⊢ e : τ@d
  [DESCEND] ──────────────────────────────────
              Γ ; δ ⊢ descend κ {e} : τ@d


              Γ ; δ ⊢ e : τ@d
  [SEAL]    ─────────────────────────────
              Γ ; δ ⊢ seal e : Cairn@0


              Γ ; δ ⊢ e : τ@d
  [SHADE]   ────────────────────────────────
              Γ ; δ ⊢ shade e : Shadeᵈ⟨τ⟩@0


              Γ ; δ ⊢ s : Shadeᵈ⟨τ⟩@d′        d ≤ δ
  [LOOK]    ──────────────────────────────────────────
              Γ ; δ ⊢ look s : τ @ max(d, d′)


              Γ ; δ ⊢ e : τ@d
  [OPAQUE]  ──────────────────────────────────
              Γ ; δ ⊢ opaque e : τ@d      (residual; see §06)


              Γ ; δ ⊢ e : τ@d
  [DEMAND]  ─────────────────────────────
              Γ ; δ ⊢ demand e : U0@d
```

`s(κ)` is the stratum of capability κ, fixed by
[section 01](01-strata.md#11-the-lattice).

That is the whole system: eleven rules.

## 2.3 What each rule is doing

**[LIT]** is why pure code disappears at burial. A literal is at depth 0, and
by [PRIM] anything built only from literals is at depth 0, and by
[section 06](06-evaluation.md) anything at depth 0 is fully evaluated before
the artifact exists.

**[PRIM]** ranges over every primitive operation, and a conditional is one of
them: `if`, `while`, `&&` and `||` take their condition and their arms as
operands, and the maximum is over all of them. The condition belongs in that
maximum because which arm was taken is itself something the condition knew —
`if (secret) { 0 } else { 1 }` tells you about `secret` whichever arm runs.

The arms are why [PRIM] is the one rule whose depth is a bound rather than a
fact, and §2.4 is about that.

**[APP]** takes the maximum of three things, not two: the depth of what the
function hands back `d_r`, the depth of the function value itself `d_f` (a
function fetched over the network is a deep value even before it is called),
and the depth of the argument. Forgetting the middle one is the classic
soundness hole in effect systems that carry effects only on arrows.

The latent depth is the premise and not one of the three. A capability is what
it takes to *reach* a stratum, and `dƒ` is what the function needs its caller
to have reached already. The other three are facts about where values have
been, and whoever took them there held the capability at the time: applying
`len` to a depth-3 `Bytes` reaches nothing. That is why [PRIM] has no ambient
premise either, and why §2.5 can say a shallow value combines with a deep one
without coercion.

**[ABS]** is where both numbers come from, and the closure itself is pure:
building a function that will touch the disk does not touch the disk.

The body is checked at `dƒ` rather than at the ambient depth of wherever the
function happened to be written, and `dƒ` is the *least* depth at which the
body checks — what the function asks of whoever calls it. A body containing
its own `descend` asks for nothing, because the descent supplies the depth
from inside; a body that calls a prelude function bare asks for that stratum,
which is how `Answer<Bytes> read(Str path) @3` works and why it is written
that way in [section 09](09-prelude.md). Both are ordinary, and the choice
between them is the choice of who holds the capability.

The body's own value depth `d₂` is what comes back, separately. Parameters are
bound at depth 0: a parameter's real depth arrives at the call site, and [APP]
joins it there.

```c
Bytes load(Str p)      { descend disk { must(read(p)) } }  // --0--> Bytes@3
Answer<Bytes> raw(Str p) @3 { read(p) }                    // --3--> Answer<Bytes>@3

Bytes@3 a = load("kernel.nc");                   // legal at δ 0
Bytes@3 b = descend disk { must(raw("kernel.nc")) };
```

`load` descends for itself and is callable anywhere. `raw` does not, so it
asks, and its callers descend instead. Neither launders anything: both hand
back a `Bytes@3`, and nothing anywhere lowers a depth.

**[DESCEND]** is the only rule that raises δ, and it raises it only inside its
own premise. Nothing lowers δ. Nothing lowers `d`.

[ABS] does not raise δ; it starts a new one. A function body is checked once,
against its own signature, and not once for every place the function is
called.

**[SEAL]** is the first escape: a name is pure regardless of what it names.
See [§1.5](01-strata.md#15-seal) for why this is sound.

**[SHADE]** is the second escape, and unlike `seal` it keeps the value —
opaquely. The origin depth `d` is carried in the type as `Shadeᵈ⟨τ⟩`, which is
what makes [LOOK] checkable.

**[LOOK]** carries the entire Orpheus rule in one premise: `d ≤ δ`. You may
look only where you already hold the capability the value came from. The
result also inherits `d′`, the depth of the shade *value* — a shade that was
itself fetched over the network is deep for two independent reasons.

## 2.4 Metatheory

Two properties an implementation MUST preserve. Both are stated here and
tested generatively by the roadmap item *Property test: depth is monotone*.

> **Soundness of the bound.** If `Γ ; δ ⊢ e : τ@d` and `e ⟶ e′` with
> `Γ ; δ ⊢ e′ : τ@d′`, then `d′ ≤ d` — never more.

A depth in a type is an **upper bound** on how far into the world the value's
history reaches. Evaluation can only ever find out that something was
shallower than it looked:

```c
if (true) { 1 } else { must(descend disk { read("k") }) }
```

has depth 3 by [PRIM] and reduces to `1`, which is at depth 0. Nothing was
lowered — the depth 3 was a bound over an arm that was never taken, and burial
found out which arm it was.

The other direction is the one that matters. A type claiming depth 3 for a
value that turns out to be pure is conservative and harmless. A type claiming
depth 0 for a value that reached the disk is a value escaping at a depth its
type did not admit, and that is the single thing the lattice exists to
prevent.

This is not [§1.2](01-strata.md#12-the-monotonicity-law), which says something
else: that no *operation* takes a deep value and hands back a shallow one.
There is no `ascend`. Picking an arm takes no deep value anywhere, because the
deep value was never produced.

The depth in a **trace** is a third number and an exact one: what evaluation
actually reached, recorded with a witness for each stratum
([§1.4](01-strata.md#14-what-the-trace-records)). A bound is what a type
offers before anything runs; a trace says what happened.

> **Ambient soundness.** If `Γ ; δ ⊢ e : τ@d` then `d ≤ max(δ, g(e))`, where
> `g(e)` is the deepest `s(κ)` over the `descend κ` expressions in `e` and,
> transitively, in the body of everything `e` applies — and 0 when there are
> none.

A value can never be deeper than the capabilities that were held while it was
made — held at the point it was made, which is the ambient depth or a descent
inside the expression that granted more. Nothing else in the system grants
anything, which is what makes `nether strata` a blame tool rather than a
guess: a value at depth 5 means some `descend net` is responsible, and it is
either enclosing, or written in the expression, or written in the body of
something the expression calls. Either way it can be found by construction,
by following the calls.

A judgement with `g(e) = 0` therefore does satisfy `d ≤ δ`, and that is most
of them. The descent is the exception, and it is the only one.

## 2.5 Subsumption, and its deliberate absence

There is no subsumption rule. `τ@0` is not a subtype of `τ@3`, and a value at
depth 0 is not silently usable where a depth-3 value is expected — because
nothing needs that: [PRIM] and [APP] already take the maximum, so a shallow
value combines with a deep one without any coercion.

The reverse direction — using a deep value where a shallow one is required —
is exactly what the language exists to prevent, and it is available only
through `seal` and `shade`.

> HolyC made everything an `I64` and let it coerce freely. This is the
> inversion of that, and it is the place where the inversion is most likely to
> be unpleasant in practice. See [section 90](90-rationale.md).
