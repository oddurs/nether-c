---
id: 241
title: Make the PR path fast and enforce its boundaries
type: chore
status: buried
milestone: quickening
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
area: scripts/
stratum: '0'
proof: Workflow regressions prove main and primary-checkout commits are refused, PR publishing is repeatable and checks once, dirty or unmerged worktrees cannot be removed, and live server policy requires checked PR merges.
---

Keep the server as the merge authority. Audit its existing rules, add missing local guards, make PR creation repeatable, wait for a verified merge, and refuse destructive cleanup. Exercise failure paths with isolated Git repositories and a stubbed GitHub client.

## 2026-09-15

Audited live GitHub policy: PR-only squash merges, required Actions check, strict up-to-date branches, admin enforcement, conversation resolution, no force pushes/deletions, auto-merge and branch deletion are already configured. Added a read-only fail-closed policy audit to publishing. Local commit/push guards now reject main, primary checkout and detached HEAD; pushes verify the exact checked-out branch/commit and detect changes during checks. Publishing checks once through pre-push, reuses open PRs, respects drafts, and reports API or merge failures. Added wait to distinguish green checks from an actual merge and safe done that verifies the exact merged head, refuses dirty worktrees and ledger data, and preserves dirty primary checkouts. Removed two tracked Python caches and ignore generated caches. Eighteen real-Git/offline-GitHub regressions and scripts/task check pass. CI remains one check plus its required gate, with bounded job timeouts; no new dependency, bot, service, approval requirement or trusted-core growth.
