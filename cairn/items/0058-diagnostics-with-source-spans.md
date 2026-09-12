---
id: 58
title: Diagnostics with source spans
type: feature
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 57
created: 2026-09-10
updated: 2026-09-12
priority: p1
effort: l
area: crates/nether-syntax
stratum: '0'
proof: Every diagnostic names a span, and the depth errors name the line that caused the descent
---

Depth errors are the ones people will hit constantly and they are the hardest
to phrase. 'This is @3 because line 14 descended to disk' is the shape to aim
for — always name the cause, never just the symptom.

## 2026-09-12

The shape the item asked for needed a second snippet rather than a longer sentence. A diagnostic now carries an optional cause — a span somewhere else and what to say about it — and an assertion that disagrees with inference points at the line that descended rather than only at the line that claimed otherwise.

## 2026-09-12

Blame prefers what somebody wrote over where they said they were going: a descend disk containing a read blames the read, and blames the descent only when nothing inside it reaches that stratum. The Orpheus error keeps its blame on the caret row, because it is the one depth error whose cause and symptom are the same expression and a second snippet would be the same line twice.

## 2026-09-12

The bar that closes a snippet is a separator rather than a border — present when something follows and gone when nothing does — which is what keeps section 1.6's error rendering byte for byte the same as before.
