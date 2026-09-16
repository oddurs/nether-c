---
id: 250
title: Turn the remaining descents into executable plans
type: docs
status: buried
milestone: after
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
stratum: '0'
area: cairn/items
proof: Every open item has a grounded scope, ordered steps and an observable acceptance gate; cairn check and generated-site checks pass
---

Document the remaining work without implementing it or treating plans as completed proofs.

## Scope and delivery

1. Audit all 52 unfinished non-milestone items against the present repository.
2. Add a starting point, bounded scope, ordered steps and acceptance evidence
   to each; explain the delivery order and exit gate for all nine open descents.
3. Correct stale font metrics, concurrency assumptions and constructor claims.
   Encode hard completion dependencies without claiming research is implemented.
4. Improve new-item templates and the generated roadmap's reading guide.
5. Validate Cairn, regenerate the site/counter, and pass the checked PR workflow.

## Non-goals

No compiler, font or browser behavior changes. No feature closure, new human
observation, year-long data, public deployment or proof-assistant adoption is
claimed. 0075 remains starved on its original observation.

## Acceptance

- [x] All 52 pre-existing unfinished items contain actionable delivery plans.
- [x] Nine open milestones have sequences, exit gates and proof fields.
- [x] Cairn validates 238 items without warnings; no unfinished item lacks a proof.
- [x] Site and graphics regeneration checks pass. The PR must pass the full
      repository gate before merge.

Hard completion edges added: 0232/0234 → 0231, 0235 → 0231/0232,
0225 → 0222, and 0227 → 0223/0224/0225 alongside its existing 0226 gate.
0209's proof now closes the specification independently; its implementation
conformance belongs to 0204–0206 rather than creating a circular gate.
