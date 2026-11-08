---
id: 292
uid: bbb09140-64e6-4a9b-afbc-d0977d2e168e
title: Push without holding GitHub's connection open through the checks
type: bug
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-08
updated: 2026-11-08
closed_at: 2026-11-08
priority: p1
effort: s
area: scripts
proof: scripts/agent pr pushes a commit whose checks take longer than GitHub keeps an idle SSH connection, and a push that fails says so and opens no pull request
---

## What happens

`scripts/agent pr` sometimes stopped after the checks passed, with no pull
request and no message (#265, #266). Running it again worked.

`git push` opens its connection, reads the remote's refs, and then runs
`pre-push`, which ran `scripts/task check`: five to ten minutes on a loaded
machine. GitHub closes an SSH connection that is idle that long. A probe with a
`pre-push` that only sleeps reproduced it: 30 s pushed, and 480 s gave
`Connection to github.com closed by remote host.` and exit 141 (SIGPIPE).
`scripts/agent` runs under `set -e`, so it ended at the push with nothing of
its own to say. A retry worked because the second run's checks were faster.

## Fix boundary and regression proof

`scripts/agent pr` runs the checks before it pushes, confirms the tree is
clean and `HEAD` did not move, and stamps that commit in the worktree's git
directory. `pre-push` skips the checks only for a stamp that names the commit
being pushed, and removes it. A failed push dies with a message.

`tests/workflow/run`: a push by hand still runs the checks, a stamp for
another commit does not skip them, and a failed push is said and opens nothing.
The last fails on the old scripts.
