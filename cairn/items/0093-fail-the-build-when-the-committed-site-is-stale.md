---
id: 93
title: Fail the build when the committed site is stale
type: chore
status: buried
milestone: lamp
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: s
area: scripts/task
proof: A PR that edits spec/ without re-baking fails CI
---

`scripts/task site:check` runs `site/bake --check`, which rebuilds into
memory and compares against what is committed. Documentation that lies fails
the build.
