---
id: 140
title: CI skips cairn check when the install fails
type: bug
status: buried
milestone: calculus
assignee: Oddur Sigurdsson
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

## And it was the wrong crate

Filing this turned up the other half. `cairn` on crates.io is an unrelated
package — "Build-gated version control for Rust projects" — and the binary this
project uses comes from `cairn-md`, which is not published there at all. So
even when the install succeeded it installed something else, and the `||` meant
nobody found out.

## What to do

Install from the repository it is published from, pinned to a tag, and let it
fail. The guard in `scripts/task` can stay, because a
contributor without `cairn` should still be able to run `scripts/task check`
locally; what must not happen is CI quietly deciding the same thing.

## Acceptance criteria

- [x] A CI run where `cairn` cannot be installed fails
- [x] `scripts/task check` still works locally without `cairn`

## 2026-09-12

Pinned to 0.2.0 as well, so a release of cairn cannot change what CI validates without a commit here. The guard in scripts/task stays, because a contributor who has not installed cairn should still be able to run the check; what must not happen is CI quietly deciding the same thing.

## 2026-09-12

Pinning the version is what turned up the other half: cairn on crates.io is an unrelated package, and the binary this project uses is cairn-md, which is not published there. So the step was installing the wrong program when it worked at all, and the || meant nobody found out. Now installed from the git repository, pinned to v0.2.0.
