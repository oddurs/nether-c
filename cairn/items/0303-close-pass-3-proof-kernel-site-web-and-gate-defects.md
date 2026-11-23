---
id: 303
uid: 51554ec2-ac4f-48f9-94df-658c2cf47427
title: Close Pass-3 proof kernel, site, web, and gate defects
type: feature
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p2
stratum: '0'
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

## 2026-11-23

Closed Pass-3 proof kernel, site, web/wasm, and gate defects: forbade subclassing and enforced exact runtime types and Formula/Term validation in proof/kernel.py; isolated lattice_decide from polymorphic FOL equality with dedicated eq_d; blocked mutually negative InductiveRelation premises; fixed SE_Descend, V_SeqCons, BadMark, and added Rust node codec fixture cross-checks; added missing Depth header in site/src/index.html; fixed site/bake inline() sentinel collision, code-in-href expansion, and blocked-link double-escaping; fixed site/gfx.py owed() EOF delimiter; aligned nether-wasm nodes count with deduplicated objects; hardened Necropolis app.js DOM rollback and graph.js span.start guard; fixed scripts/decay cfg(test) and block-comment counting (lowering .decay-ceiling to 12309); anchored attribution trailer regex and aligned case sensitivity across hook and attribution test; added bounds checks to tests/wasm/run.
