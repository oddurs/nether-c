---
id: 262
title: 'Spec: the disk is the one bounded capability the invocation cannot bound'
type: spec
status: unmarked
milestone: codex
created: 2026-09-16
updated: 2026-09-16
priority: p1
effort: m
stratum: '0'
area: spec/08-rites.md
proof: An operator can state where a burial may read, or §8.3 says why they may not
---

## The asymmetry

`spec/08-rites.md` gives `net` a subsection of its own (§8.3.2, `--reach`) and
`unrecorded` another (§8.3.3, `--load`). Both make the same argument: a
capability says *which stratum*, and a second flag says *how far*.

§8.3.2 makes that argument by pointing at the disk:

> §09 says nothing about where a path is rooted, and an implementation that
> rooted it nowhere would be one nobody could grant a capability to; a socket
> is the same argument with a longer reach.

That concedes the disk needs a bound. §8.3 does not give it one. 0069 records
that this implementation has "a root nothing may reach out of, resolved
textually so a symbolic link cannot climb out of it" — so the bound exists, in
one implementation, unstated and undeclarable.

Two consequences:

- An operator cannot say where a burial may read. `--grant disk` is the whole
  of what the invocation can express, and it means whatever the build decided.
- Two implementations answer `read("main.nc")` differently while the hole's
  `call` — "the prelude function and its fully-evaluated arguments" — is
  byte-identical. The reproducibility claim touches a filesystem exactly here.

## And the port

§8.3.2:

> Without a port it matches whatever port the URL asks for, because the port is
> a detail of the service and the host is the party being trusted.

So `--reach example.com` permits 22, 25, 6379 and 11211 on that host. The
document's standard everywhere else is that a grant states its bound; this is
the one place it argues the bound does not matter, and the argument is weaker
than the section it sits in.

## What to write

§8.3.4, `--root`, on §8.3.2's own reasoning: repeatable, resolved textually, and
a path outside every root refused `denied` rather than `absent` — the §9.6
distinction, for the §9.6 reason, since a root is this build saying no.

Then settle the port: keep the wide default and say what it costs in the
sentence that permits it, or make a bare host mean the scheme's default port.

## Acceptance criteria

- [ ] §8.3 says how the disk is bounded, or says it is the implementation's and why `net` is not
- [ ] §9.5 links whichever it is
- [ ] §8.3.2 states what a portless reach permits, where it permits it
- [ ] §90.2 records the alternative
