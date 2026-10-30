---
id: 277
title: Setup overwrites the roadmap schema
type: bug
status: buried
created: 2026-10-30
updated: 2026-10-30
priority: p1
effort: s
area: scripts/setup
proof: Running scripts/setup with cairn installed leaves cairn.toml and cairn/items unchanged, and registers the merge driver
---

## What happens

`scripts/setup` registers the merge driver with `cairn init --git --force`.
`--force` means *overwrite an existing cairn.toml*, so on any clone with cairn
installed it replaces this project's schema — the language's own statuses,
`stratum`, `proof`, the saved views — with cairn's standard preset, and writes
four starter items whose status (`backlog`) the real schema does not have. The
`|| true` after it hid nothing, because nothing failed: the overwrite succeeded.

## What should happen

The driver is registered and the tree is left alone.

## Reproduction

1. Install cairn 0.2.0 and run `scripts/setup` in a clean clone.
2. `git status` shows `cairn.toml` modified and `cairn/items/0001-first-usable-version.md`
   through `0004-adopt-cairn-for-the-roadmap.md` untracked.

## Cairn of the offending trace

None. This is the repository's tooling, not a burial.

## Fix boundary and regression proof

`cairn init --git` without `--force` is documented as safe on an existing
project, and is: in a scratch clone with the driver unset it printed
`registered merge driver cairn`, exited 0, and `git status` was empty. The
`|| true` goes too. A setup step that fails should say so, and `set -eu` will.
