---
id: 27
title: Extract every spec code sample into tests/transcripts
type: chore
status: buried
milestone: codex
depends_on:
- 11
- 20
- 22
- 84
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: m
area: tests/transcripts/
proof: CI fails if a sample in the spec disagrees with the tool's actual output
---

Every code block and every shell transcript in `spec/` and on the site is a
test fixture. The documentation cannot drift from the implementation if
drifting fails the build.

This is written before the implementation exists, so the fixtures are the
specification of the behaviour rather than a recording of it.

## 2026-09-10

Built tests/transcripts/run: extracts every sample from README.md, site/src/index.html and spec/*.md, executes the ones nether can answer today, pins the rest by sha256 in MANIFEST.tsv. 59 samples, 3 executed, 56 pinned; both counts printed every run so nothing is skipped quietly. It caught two real documentation lies on its first run — the nether run excerpt in the README and on the front page, neither of which admitted to being an excerpt. Added an explicit [...] elision marker rather than loosening the match.
