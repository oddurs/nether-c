---
id: 91
title: Publish the site
type: chore
status: buried
milestone: lamp
depends_on:
- 92
created: 2026-09-10
updated: 2026-09-10
priority: p1
effort: s
area: site/
proof: The published URL serves the current main and updates on merge
---

Static files with no build step, so anything serves it. GitHub Pages from
/site on main is the least machinery and therefore the right answer unless
there is a reason it is not.

## 2026-09-10

GitHub Pages, deployed by .github/workflows/pages.yml from site/ as committed. No build step in the workflow: the files are already there, and scripts/task site:check in CI is what guarantees they match spec/. This avoids renaming site/ to docs/ purely to satisfy the classic branch-folder option.
