---
id: 139
title: The Decay Rule can be nullified by one line
type: bug
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
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

- [x] A `#[cfg(test)]` above real code does not stop that code being counted
- [x] `tests/decay/run` covers a test module that is not last
- [x] The count over `crates/` does not change

## 2026-09-12

The counter matches braces on text with strings, character literals and comments stripped, so a brace inside a string does not unbalance it. The line predicate is unchanged on purpose: the cleaned text is for finding braces, not for deciding what a line is, so a line that is only the middle of a string literal still counts. The count over crates is identical to the old one, 5598, which is what says the change is about where tests are and nothing else.

## 2026-09-12

Took the unquoted find loop with it, since it was the loop being rewritten. Noted on 0148.
