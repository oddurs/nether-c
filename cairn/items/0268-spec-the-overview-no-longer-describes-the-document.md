---
id: 268
title: 'Spec: the overview no longer describes the document'
type: spec
status: buried
milestone: codex
assignee: Oddur Sigurdsson
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: s
stratum: '0'
area: spec/00-overview.md
proof: §0.6 agrees with every section that flags an open question, and with every section that says it is frozen
---

## Three claims the churn falsified

Nine specification pull requests landed in two days. [§0.5](../spec/00-overview.md)
requires every normative claim to appear exactly once; three in §00 and §10 now
disagree with the sections they describe.

**§0.6 says no design question is open.**

> No design question in this draft is currently open. That is not a claim that
> none remain, only that none are known.

Two are, and both are flagged exactly as §0.6's own paragraph above prescribes:
[§6.5](../spec/06-evaluation.md) block-quotes item 0251 — whether an untaken
arm may be reduced — and [§9.2](../spec/09-prelude.md) block-quotes item 0266 —
whether the prelude gains an `Answer` constructor. §0.6 states the rule, two
sections follow it, and §0.6's last paragraph denies the result.

**§0.6 says only one thing is frozen.**

> Nothing here is frozen except where a section says so explicitly, and today
> only one thing is: **there is no `run`**.

[§7.1](../spec/07-ledger.md#71-canonical-encoding) opens with **Frozen.** in
bold, and [§7.3.1](../spec/07-ledger.md#731-node-encoding) with *Frozen*, on
the same terms. [§8.0](../spec/08-rites.md) makes the narrower claim that is
true — "the only frozen *requirement*" — and §0.6 generalised it into one that
is not.

**§10 defines the latent depth as the reading §90.2 rejected.**

> **Latent depth** — the depth a function reaches when applied, carried in its
> arrow type and written as a trailing `@n` on its signature.

Three things wrong in four lines. `dƒ` is not what a function *reaches*: it is
what a caller must already hold, and
[§90.2](../spec/90-rationale.md)'s *One number on an arrow* records "what it
reaches" as the third number that was considered and refused, because a trace
already lists every stratum reached and a number in a signature that no rule
consumes is a comment with a syntax. It is also no longer a depth — [§2.1](../spec/02-calculus.md#21-judgement-form)
makes it a set — and no longer only `@n`, since [§3.5](../spec/03-lexical.md#35-depth-annotations)
spells `@{3,5}`.

The first two of those are the churn's. The third was wrong before it.

## Constraints it inherits

§0.6's shape is worth keeping: it names what has been settled and points at
section 90 for what each cost. What it may not do is carry a count that has to
be revised every time an item opens, which is how it came to be wrong — so the
fix is a sentence that stays true rather than a corrected number.

## Delivery steps and dependencies

1. §0.6 says where open questions are, not how many there are.
2. §0.6's frozen sentence says what §8.0 says, and names §7.1 and §7.3.1.
3. §10's entry becomes the latent set, defined as what a caller must hold.

## Acceptance criteria

- [x] §0.6 agrees with §6.5 and §9.2 about whether a question is open
- [x] §0.6 agrees with §7.1, §7.3.1 and §8.0 about what is frozen
- [x] §10 defines `dƒ` as what a caller holds, not what the function reaches
- [x] Neither §0.6 nor §10 states a number that a new item would falsify

## 2026-09-18

§0.6 no longer counts anything. Which questions are open is a fact about the roadmap, so the section points at the filter and at the block quotes in the sections themselves, which is what §0.6's own rule already prescribed. A count in the overview is a claim that has to be revised every time an item opens, and it was not.

## 2026-09-18

The glossary defined the latent depth as what a function reaches when applied, which is the third number §90.2's One number on an arrow considered and refused. It is what a caller must hold, it is a set since 0254, and it spells @{n,m} as well as @n.
