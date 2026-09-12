---
id: 122
title: 'Spec: a function cannot contain a descent and be called'
type: spec
status: unmarked
milestone: calculus
depends_on:
- 13
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: m
area: spec/02-calculus.md
proof: A function whose body descends can be called where its body could be written, and inlining a call never changes whether a unit checks
---

## The hole

[ABS] gives an arrow its latent depth from the body's **value** depth:

```
              Γ, x : τ₁@d₁ ; δ ⊢ b : τ₂@d₂
  [ABS]     ──────────────────────────────────────────
              Γ ; δ ⊢ λx.b : (τ₁ --d₂--> τ₂)@0
```

[APP] then requires `dƒ ≤ δ`. Put together:

```c
Bytes load(Str p) { descend disk { must(read(p)) } }

Bytes src = load("kernel.nc");   // rejected: dƒ is 3 and δ is 0
```

while `spec/01-strata.md` §1.3 opens by accepting the identical expression
written out:

```c
Bytes@3 src = descend disk { read("kernel.nc") };
```

So a descent may be written but not named. There is no way to factor
`load` out of the program, and every use of the world has to be spelled at the
place that uses it.

The same rule goes wrong in the other direction. A function whose deep work is
deposited rather than returned —

```c
U0 stamp() { descend disk! { write("out", b); } }
```

— has a body of value depth 0 and therefore latent depth 0, while §2.1 says a
latent depth is "the deepest stratum the function reaches when applied". It
reaches 4.

## Why this is p0

Burial reduces applications. Under [ABS] as written, an application can be
rejected where its inlining is accepted, so the typability of a program is not
stable under the reduction the language is built out of. That is one step away
from the staging law in §6.5 — `bury(bury(p, A), B) ≡ bury(p, A ∪ B)` — and
the residue of a burial is supposed to be a program that checks.

## What it looks like settled

An arrow carries two numbers, not one, and the grammar already writes both:

```c
Bytes@3 load(Str p) @0 { descend disk { must(read(p)) } }
```

The `@0` after the signature is the **latent** depth: the smallest ambient
depth at which the body is well-typed — what the function asks of its caller.
The `@3` on the return type is the depth of what comes back. `load` asks for
nothing and hands back something deep, which is exactly what it does.

For a prelude function the two coincide, which is why one number looked like
enough: `Answer<Bytes> read(Str p) @3` asks for 3 and returns depth 3.

The alternatives, both of which need recording either way:

- keep one number and accept that a descent cannot be named, which removes
  abstraction over the world entirely;
- keep one number and drop the premise on `dƒ`, which makes `read(p)` legal
  with no descent anywhere and deletes the point of §1.3.

## Acceptance criteria

- [ ] §2.2 says what a latent depth is, and it is what a caller must hold
- [ ] Where the depth of a call's result comes from is stated
- [ ] §2.1's prose and [ABS] say the same thing
- [ ] A worked example of a function that descends and is called from the surface
- [ ] The rejected alternatives in §90.2
- [ ] nether-core's checker follows
