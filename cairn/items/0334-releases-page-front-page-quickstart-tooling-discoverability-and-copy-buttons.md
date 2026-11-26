---
id: 334
uid: 204500d8-25b9-44ff-ae56-d7d1150082b8
title: Releases page, front-page quickstart, tooling discoverability, and copy buttons
type: feature
status: buried
created: 2026-11-26
updated: 2026-11-26
closed_at: 2026-11-26
priority: p1
area: site,web/necropolis,tests
stratum: '0'
effort: m
proof: site/releases.html bakes deterministically from CHANGELOG.md, SECURITY.md, and release.yml; all links, font, css, contrast, notes, transcripts, attribution, and Chromium browser checks pass
---

## Problem

## Proposal

## Starting point and non-goals

Name existing implementation/tests; distinguish missing work from work already delivered.

## Delivery steps and dependencies

1. Resolve specification changes first, in their own PR.
2. Implement the smallest slice; name the paths and prerequisite items.
3. Verify success, failure and boundary behavior.

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.

## 2026-11-26

Fixed broken #834-declaring-a-root anchor in CHANGELOG.md, added root-level spec/repo href rewrites and deterministic Releases page baking (CHANGELOG.md + SECURITY.md drift check + release.yml targets) in site/bake, added {{VERSION}}/{{LATEST_RELEASE}} slots, compact nav with releases, 1:1 front-desk directory, manual install bridge and chapter pager, spec/specimen/404/Necropolis releases links, progressive-enhancement clipboard copy buttons in site/template.html, reordered front-page install/quickstart and #tooling section in site/src/index.html, and extended tests/notes/run, tests/transcripts/MANIFEST.tsv, and tests/necropolis/browser.mjs.
