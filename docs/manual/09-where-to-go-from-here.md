---
section: "09"
title: Where to go from here
status: draft
---

# Where to go from here

You have buried four programs, given one of them a disk, given another two
different environments, asked a trace whether the world had moved, and been
refused by the type system for looking at something from the wrong depth.

That is the short path. Here is the rest of the map.

## The specification, in a useful order

It is not long and it is not a reference manual you dip into. Read these four,
in this order, and you have the language:

| | | |
|---|---|---|
| [§0](../../spec/00-overview.md) | Overview | the argument, in two pages |
| [§1](../../spec/01-strata.md) | Strata | the nine, and why depth composes |
| [§2](../../spec/02-calculus.md) | Calculus | eleven rules, one page |
| [§6](../../spec/06-evaluation.md) | Evaluation | what burial actually does |

[§2](../../spec/02-calculus.md) is the one to sit with. Everything the language
does is a consequence of eleven rules that fit on a page, and the page being
one page is a constraint the project holds itself to rather than a thing that
happened to work out.

Then, as you need them:

- [§5](../../spec/05-types.md) — types, shades, refusals, and why a loop body
  comes after itself
- [§7](../../spec/07-ledger.md) — what a node is, what a trace holds, how
  provenance is read backwards
- [§8](../../spec/08-rites.md) — every rite, every flag, every exit code
- [§9](../../spec/09-prelude.md) — the prelude, function by function, with the
  stratum of each
- [§10](../../spec/10-glossary.md) — the vocabulary, which is not decoration
- [§90](../../spec/90-rationale.md) — the rejected alternatives, which is the
  only honest part of any design document

## Rites this manual did not use

`nether graft` substitutes an answer into a trace and re-buries only what
changed, which is the operation a build system wants and the reason content
addressing earns its keep. It is [§8.7](../../spec/08-rites.md#87-graft). It is
not in this manual because its behaviour when replacing an answer that was
already given is wrong today — see item 0273 — and a worked example would
teach the bug.

## The three questions people ask

**Where is `main`?** There isn't one. There is `demand`, and a program is the
set of things that were demanded and whatever those need. See
[§4](../../spec/04-grammar.md).

**How do I print something?** You do not. You deposit it and somebody lamps it
afterwards. If that feels like a missing feature, [chapter 1](01-the-first-burial.md)
is the argument for why it is the point.

**How do I run it?** You do not, and `nether run` exists solely to say so and
exit 64. It is the single frozen requirement in an otherwise entirely draft
specification — [§8.0](../../spec/08-rites.md). Everything else here may change
without a version bump. That one will not.

## If you want to work on it

[CONTRIBUTING.md](../../CONTRIBUTING.md) is the process and
[CLAUDE.md](../../CLAUDE.md) is the standard. The roadmap is generated from
`cairn/items/`, one Markdown file per item, and every item carries a *proof* —
the observable fact that settles whether it is finished. "The code exists" is
not a proof.

The specification comes before the compiler. If you are about to implement
something `spec/` does not describe, the section is the first pull request.

## One last thing

Everything you read in this manual was executed to produce it. Every `$` line
on these nine pages runs on every build of this project, in a scratch directory
with these programs in it, and the output underneath is compared to what came
back. Two bugs were found writing it, and they are 0273 and 0274.

Documentation that lies fails the build. That is not a policy; it is
`tests/transcripts/run`, and it is forty lines.
