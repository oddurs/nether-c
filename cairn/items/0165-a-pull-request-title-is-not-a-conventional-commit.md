---
id: 165
title: A pull request title is not a Conventional Commit
type: bug
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p2
effort: s
area: scripts/agent
stratum: '0'
proof: What a squash lands on main has a subject the commit-msg hook would accept
---

## What landed

    4eec7af fix/0164 the attribution ban (#118)
    2599afd feat(cli): nether graft, and two routes to one name (#117)

One of a hundred and eighteen. `scripts/agent pr` opens with `gh pr create
--fill`, which takes the title from the single commit when there is one and
from the **branch name** when there is more than one. A squash merge takes its
subject from the title, so a branch that happened to carry two commits put its
own name on `main`.

The `commit-msg` hook cannot catch it: the server does the squash, and no hook
of ours runs there.

## What to do

`--fill-first`. It takes the first commit's subject and body whatever the
commit count, so the title is a Conventional Commit because the commit was.

Rewriting `main` to fix the one that landed is not on the table — the hook
refuses it and so does the server, and working around either is a bug in the
approach. It stays as the record of this.

## Acceptance criteria

- [x] `scripts/agent pr` titles a pull request from its first commit
- [x] The reason is written where the flag is, because the two flags differ by
      one word and by everything
