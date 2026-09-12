---
id: 139
title: The Decay Rule can be nullified by one line
type: bug
status: unmarked
milestone: calculus
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: scripts/decay
stratum: '0'
proof: A `#[cfg(test)]` above real code does not stop that code being counted
---

## The loophole

`scripts/decay` strips tests with

```sh
sed '/#\[cfg(test)\]/,$d'
```

which deletes from the **first** marker to the end of the file. The comment
above it assumes the trailing-module convention; nothing enforces it. One
attribute at the top of a file and the whole file stops counting:

```console
$ scripts/decay /tmp/decaytest/core /tmp/decaytest/ceil
decay: core is 3 lines below the ceiling (0/3)
```

That tree holds six functions. The Decay Rule is the rule this project says has
teeth, and it can be turned off by one line that also compiles.

## Why the harness did not catch it

`tests/decay/run` writes `BODY + TESTS` and checks that adding tests does not
grow the core. It never writes `TESTS + BODY`.

## What to do

Strip `#[cfg(test)]` modules wherever they are, rather than assuming they are
last — the cheap version is to find the marker, then the `{` that opens the
module, then match braces to its close. And add the case to `tests/decay/run`,
because a rule with teeth needs a test that bites.

## Acceptance criteria

- [ ] A `#[cfg(test)]` above real code does not stop that code being counted
- [ ] `tests/decay/run` covers a test module that is not last
- [ ] The count over `crates/` does not change
