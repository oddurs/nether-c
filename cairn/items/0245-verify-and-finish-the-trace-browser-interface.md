---
id: 245
title: Verify and finish the trace browser interface
type: feature
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
part_of:
- 75
stratum: '0'
area: web/necropolis
proof: Headless Chrome verifies real worker burial, graph navigation, keyboard use, source highlighting, mobile layout, hostile text, cancellation and recovery; screenshots are visually inspected.
---

## Problem

Native parity tests cannot prove that a worker can bury in a browser, nor that
the graph is navigable on a phone or by keyboard.

## Proposal

Keep controls in HTML and draw deterministic SVG connections between them.
Use the same relationships in a stacked mobile view. Require a real Chrome
test through the existing task seam, without a driver dependency.

## Which stratum does this reach?

Stratum 0. Source stays in the page; world questions remain holes.

## Acceptance criteria

- [x] Desktop and 390px layouts visually inspected.
- [x] Real worker/WASM graph traversal and keyboard navigation pass in Chrome.
- [x] Hostile text, cancellation, restart and load-error recovery are covered.
- [x] Large neighbourhoods expand incrementally and retain keyboard focus.
- [x] Full repository checks pass, including the unchanged WASM budget.
- [x] Parent 75's unfamiliar-person proof remains explicitly unclaimed.

## 2026-09-15

Nine decoder/navigation/memory tests and three real-WASM tests pass. Chrome exercises automatic worker burial, hole/argument/deposit/source navigation, keyboard Enter history, a 390px layout with no horizontal overflow, hostile text, paged relationships with retained focus, cancellation/restart, empty traces, module-load failure and recovery. Desktop and mobile screenshots were visually inspected; inherited heading decorations were removed where they crowded this view.
