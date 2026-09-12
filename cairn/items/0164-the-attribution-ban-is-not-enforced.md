---
id: 164
title: The attribution ban is not enforced
type: bug
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: .githooks
stratum: '0'
proof: Every shape attribution takes is refused by a check, in the message and in the tree
---

## Measured

`CLAUDE.md` bans attributing work to a tool, a model or an assistant, and says
the `commit-msg` hook rejects it. The hook matched a model name only on a
`Co-authored-by:` line, plus "generated with", a robot emoji and "as an AI".
Six of seven shapes went straight through:

```
caught  Co-Authored-By: Claude Opus 5 <noreply@anthropic.com>
PASSES  Claude-Session: https://claude.ai/code/session_01HH
PASSES  See https://claude.ai/code/session_01HH
PASSES  Assisted-by: Claude
PASSES  Generated-by: an assistant
PASSES  Signed-off-by: Anthropic
PASSES  Written with help from an AI model
```

The two that pass most easily are the two a tool adds by default: a session
trailer and a session link.

## And nothing checked what is already here

A hook runs on a message being written. Nothing looked at the tree, the items,
the specification, or the commits already on a branch — so the rule covered
what came next and not what was there.

The repository is in fact clean: a hundred and sixteen commits, every pull
request body, every comment, no releases, and the only occurrences of the word
are the filename `CLAUDE.md` being cited. That is worth keeping rather than
re-establishing by hand.

## Acceptance criteria

- [x] Every shape attribution takes is refused by the hook
- [x] A check refuses a tree or a branch that carries one, in `scripts/task`
- [x] `CLAUDE.md` stays usable as a filename
- [x] dependabot's own trailer still passes, because it is a bot under its own
      name and not this project's work handed away

## 2026-09-12

The repository was already clean: 116 commits, every PR body, every comment, no releases, and every occurrence of the word is the filename CLAUDE.md being cited. What was missing was anything that would keep it that way.
