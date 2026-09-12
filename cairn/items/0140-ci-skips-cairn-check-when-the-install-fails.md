---
id: 140
title: CI skips cairn check when the install fails
type: bug
status: unmarked
milestone: calculus
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: .github/workflows
stratum: '0'
proof: A CI run where cairn cannot be installed fails
---

## The hole

`.github/workflows/ci.yml`:

```yaml
run: cargo install cairn --locked || echo "cairn unavailable; scripts/task skips it"
```

paired with `scripts/task check`:

```sh
if command -v cairn >/dev/null 2>&1; then cairn check; fi
```

If the install breaks — a yanked version, a registry outage, a toolchain bump —
the roadmap check silently stops running and CI stays green. That is `|| true`
with extra steps, which `CLAUDE.md` lists under NEVER.

## What to do

Let the install fail. The guard in `scripts/task` can stay, because a
contributor without `cairn` should still be able to run `scripts/task check`
locally; what must not happen is CI quietly deciding the same thing.

## Acceptance criteria

- [ ] A CI run where `cairn` cannot be installed fails
- [ ] `scripts/task check` still works locally without `cairn`
