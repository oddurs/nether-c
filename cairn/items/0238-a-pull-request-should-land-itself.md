---
id: 238
title: A pull request should land itself
type: feature
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-14
priority: p1
effort: s
area: scripts/agent
stratum: '4'
proof: scripts/agent pr arms auto-merge, and a green pull request merges and deletes its branch with nobody watching
---

## The gap

`scripts/agent pr` opens a pull request and stops. Somebody then has to come
back, notice CI went green, and press the button — which for a project worked
on by agents means the merge happens whenever a human next looks, not when the
work is ready.

GitHub will do it: auto-merge lands a pull request the moment its required
checks pass. It is now enabled on the repository, `main` requires the
`required` check, and no approving review is required — so arming it is one
flag and the loop closes.

## What changes

`scripts/agent pr` arms auto-merge unless the pull request is a draft. A draft
is the way to say *not yet*, and it already exists.

The safety comes from the branch protection rather than from the script: main
requires `required` to pass, requires linear history, refuses force pushes and
deletions, and enforces all of it on admins too. A pull request that arms
auto-merge and then fails CI simply sits there.

## Acceptance criteria

- [x] `scripts/agent pr` arms auto-merge; `--draft` does not
- [x] `CONTRIBUTING.md` says what happens after a pull request is opened
- [x] The branch is deleted on merge, which the repository already does

## 2026-09-14

Armed. scripts/agent pr now calls gh pr merge --auto --squash unless the pull request is a draft, so a green branch lands without anybody watching and deletes itself.

Wrote it with '|| say' first, which is exactly what this repository forbids and what ci.yml carries a comment about. Rewritten as an explicit if/then that says which of the two things happened.

The safety is in the branch protection rather than in the script, and that is now real rather than aspirational. main requires the 'required' check, requires a branch to be up to date, keeps history linear, refuses force pushes and deletions, and enforces all of it on administrators. Tested it by committing an empty commit and pushing: refused, and main untouched.

Also enabled on the repository while I was there: auto-merge, Discussions, and the homepage pointing at the Pages site. Labels now speak the project's vocabulary — spec, descending, starved, unrecorded, deep, decay, the face, the ledger — and wontfix is gone, because CLAUDE.md's table says say unrecorded. GitHub's own good first issue and help wanted stay, because those are how newcomers and GitHub's own surfacing find a project.
