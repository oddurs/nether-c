---
id: 254
title: 'Spec: an ambient depth is a number, so descend net grants the disk'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-16
updated: 2026-09-16
priority: p0
effort: m
stratum: '0'
area: spec/02-calculus.md
proof: A program holding only net that writes to the disk is rejected, or §1.1 states the subsumption
---

## The program that should not check

`spec/02-calculus.md` §2.2, the premise of [APP]:

```
dƒ ≤ δ
```

and §2.1: "δ is the **ambient depth**: the deepest stratum whose capability is
currently held."

[DESCEND] raises δ to `max(δ, s(κ))`. So inside `descend net` δ is 5, and
`write` — `@4` in [§9.5](../spec/09-prelude.md) — has `dƒ = 4 ≤ 5`:

```c
descend net { must(write("/etc/shadow", b"")) };
```

No `descend disk!` anywhere, and nothing rejects it. The same argument reaches
every stratum below the one that was named: granting `net` grants `disk!`,
`disk`, `env` and `store` with it.

## Two systems, one symbol

§1.3 says `descend κ` "acquires the capability named κ". §6.1 takes
`Capabilities`, a set. §8.2's `--grant` repeats. The rites and the prose think
in sets.

§2.2 thinks in a maximum, and a maximum cannot say *the disk and entropy but
not the network*.

## What has to be chosen

**The order is a subsumption order.** Then say so in §1.1, and say it in §8.2
where an operator reads it: `--grant net` is `--grant disk!` and every
shallower capability as well. §8.3.2 spends a subsection bounding which host
`net` may reach, beside a grant that silently carried the filesystem; that
subsection has to say so too.

**Or δ is a set.** `dƒ ≤ δ` becomes `κ(dƒ) ∈ δ`, [DESCEND] adds to δ rather
than maximising it, and §2.4's ambient soundness is stated over the set. The
calculus stays at eleven rules — nothing is added, one premise changes shape —
and the depth on a *value* stays a number, because that is a different thing
and always was.

The second is what §1.3, §6.1 and §8.2 already describe. The first is what §2.2
says.

## What §6.1 does with it

Unresolved either way:

> It can evaluate an expression when the expression's depth is within the
> granted capabilities.

With `{net}` granted, is a depth-3 `read` within it? The sentence reads both
ways, and the answer decides whether the call runs or becomes a hole.

## Acceptance criteria

- [x] §2.1 and §2.2 say whether δ is a number or a set, once
- [x] A program holding only `net` that writes to the disk is rejected, or §1.1 and §8.2 state the subsumption
- [x] §6.1 says what "within the granted capabilities" means
- [x] §8.3.2 is consistent with whichever was chosen
- [x] The calculus stays at eleven rules
- [x] §90.2 records the rejected reading

## 2026-09-16

δ is a set of strata, not a number. The capabilities are in bijection with the strata, so a set of one is a set of the other and nothing new enters the calculus: [APP] is dƒ ⊆ δ, [DESCEND] adds {s(κ)}, [LOOK] is d ∈ δ. Eleven rules, unchanged in count. Stratum 0 is in δ always, which is what lets both premises be stated without a special case.

## 2026-09-16

Three ways to keep one number were rejected and are in §90.2: declaring the subsumption outright, which leaves no invocation meaning 'fetch this URL and touch no files'; splitting the static check from the grant, which is sound and takes back the audit the fixed capability names exist for; and letting a function ask for at most one capability, which is an exception to remember rather than understand. The cost taken is a braced latent annotation, @{3,5}, and two nested descents where one used to do.

## 2026-09-16

The checker holds a Held rather than a Depth. [ABS] is where the difference shows: an unmet capability inside a function body is not a fault, it is what the function asks of its caller, so descend net { write(p, c) } is an error only where there is nobody to ask. §1.3's sample is a unit-level binding for that reason.
