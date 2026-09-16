---
id: 244
title: Navigate trace objects and inspect their recorded contents
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
proof: Decoder and navigation tests preserve canonical values and labelled references; a worker-backed page opens real trace objects without interpreting source as HTML.
---

## Problem

## Proposal

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## 2026-09-15

Seven deterministic decoder/navigation tests pass. The first real-WASM check
exposed a pre-existing blocker: burial attempts to spawn a native thread on
wasm32-unknown-unknown and returns a NoStack diagnostic. Native parity tests
cannot catch this. Fix the target-specific stack boundary and require a real
module burial before considering the navigation layer complete.

## 2026-09-15

Fixed the actual WASM failure with the configured 8 MiB module stack and a 128-frame diagnostic limit; native burial retains its dedicated stack. Three real-module tests now pass, including sample graphs, recursion/fuel diagnostics, exact integers and repeated allocations. The required target branch costs three core lines (7993 to 7996); this is still 21 fewer than before 75. WASM shrank to 146840 compressed bytes, within the unchanged budget.
