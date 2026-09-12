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

Function types carry a **latent depth**, written `τ₁ --d--> τ₂`: the deepest
stratum the function reaches when applied. In source syntax this is written as
a trailing annotation on the signature (`Bytes read(Str path) @3`).

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


              Γ ; δ ⊢ f : (τ₁ --dƒ--> τ₂)@d_f      Γ ; δ ⊢ a : τ₁@d_a
              max(dƒ, d_f, d_a) ≤ δ
  [APP]     ──────────────────────────────────────────────────────────
              Γ ; δ ⊢ f a : τ₂ @ max(dƒ, d_f, d_a)


              Γ, x : τ₁@d₁ ; δ ⊢ b : τ₂@d₂
  [ABS]     ──────────────────────────────────────────
              Γ ; δ ⊢ λx.b : (τ₁ --d₂--> τ₂)@0


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

**[APP]** takes the maximum of three things, not two: the function's latent
depth `dƒ` (how deep it goes when run), the depth of the function value itself
`d_f` (a function fetched over the network is a deep value even before it is
called), and the depth of the argument. Forgetting the middle one is the
classic soundness hole in effect systems that carry effects only on arrows.

**[ABS]** is where latency is introduced: the body's depth becomes the arrow's
latent depth, and the closure itself is pure. Building a function that will
touch the disk does not touch the disk.

**[DESCEND]** is the only rule that raises δ, and it raises it only inside its
own premise. Nothing lowers δ. Nothing lowers `d`.

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

> **Monotonicity.** If `Γ ; δ ⊢ e : τ@d` and `e ⟶ e′`, then
> `Γ ; δ ⊢ e′ : τ@d′` with `d′ ≥ d` — never less.

Evaluation can only ever learn that something is deeper than it looked. This
is what makes a depth printed in a trace trustworthy: it is a lower bound that
has already been reached, not a prediction.

> **Ambient soundness.** If `Γ ; δ ⊢ e : τ@d` then `d ≤ max(δ, g(e))`, where
> `g(e)` is the deepest `s(κ)` over the `descend κ` expressions in `e`, and 0
> when there are none.

A value can never be deeper than the capabilities that were held while it was
made — held at the point it was made, which is the ambient depth or a descent
inside the expression that granted more. Nothing else in the system grants
anything, which is what makes `nether strata` a blame tool rather than a
guess: a value at depth 5 means some `descend net` is responsible, and it is
either enclosing or written in the expression itself. Either way it can be
found by construction.

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
