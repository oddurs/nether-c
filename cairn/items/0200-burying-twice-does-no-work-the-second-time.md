---
id: 200
title: Burying twice does no work the second time
type: feature
status: buried
milestone: quickening
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-09-13
updated: 2026-10-30
priority: p0
effort: l
area: crates/nether-bury
stratum: '1'
proof: Burying an unchanged program twice against one store evaluates nothing the second time, reports the same cairn and the same steps and that it remembered, and a --fuel below those steps runs out where a cold burial does
---

## Why this is the whole point

Every value already exists at its content address. A burial that has been done
once has named everything it produced, and a second burial of the same source
should find all of it and evaluate nothing.

That is not an optimisation. It is the thing content addressing was for, and
0044 measured 99.93% node reuse for a one-line change — which is the same
property seen from the other side.

## What it needs

Burial to consult the store before evaluating, which means burial holds
stratum 1, which means `bury` grants it by default and says so.

## Watch out for

Fuel. A burial that evaluates nothing spends no fuel, and 8.2 reports what was
spent — so the number becomes evidence rather than trivia.

## Delivery plan — 2026-09-15

### Starting point and scope

Content addressing alone does not supply a sound burial cache key. Zero warm-cache fuel must be reconciled with deterministic accounting in §6.4 before implementation.

### Steps

1. Specify keys including source, semantics/version, grants, recorded answers and relevant limits; separate lookup from evaluation fuel.
2. Define behavior for smaller budgets, changed answers and missing/corrupt closure objects.
3. Implement bounded verified lookup and invalidation tests; retain fresh world queries unless replay is requested.

### Acceptance and evidence

- [x] A supported unchanged burial spends zero evaluation fuel and returns the same cairn; changed inputs cannot reuse stale results. Resolve the fuel/spec conflict in a separate specification PR first.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-10-30

Specification: §8.2.1. The rite keeps a memo outside the ledger, keyed on the implementation's identity and the source's cairn, naming the trace and the steps it spent. A hit is honoured only if the trace is present with that source and no witnesses, and only if --fuel covers the recorded steps; the rite reports the recorded steps, which are a fresh burial's, and says it remembered. Burial holds no capability and the trace format does not move. §90.2 records burial holding stratum 1 (the original proposal) and reporting zero steps as the rejected alternatives. The proof is restated to match: the second burial evaluates nothing, and fuel still means the same thing on every implementation. Implementation is the next pull request: the memo file, the identity as the cairn of the executable, the four rules, and tests for a hit, a stale entry, a budget below the recorded steps, and a deleted memo.

## 2026-10-30

Buried. crates/nether-cli/src/memo.rs keeps one file per burial under <store>/memo, named by the cairn of (the cairn of the running executable, the source cairn), holding the trace, the steps and the node count, written beside and renamed over. bury consults it after checking and before burying; an entry is honoured only if the trace is a Trace with that source and no witnesses and its steps fit --fuel. Fresh and remembered burials are reported by one path that reads the trace back. The summary says remembered, and fuel_spent is the recorded steps. Observed: build.nc buried twice gives 76c4b655 both times, 11 steps both times, remembered:false then true; --fuel 10 then runs out after 10 steps, exit 75; deleting <store>/memo, or spoiling an entry to name an absent trace, buries afresh to the same cairn. Tests: burying_twice_remembers_and_reports_the_same_burial, a_budget_below_the_remembered_steps_runs_out_where_a_cold_burial_does, a_memo_that_is_gone_or_wrong_costs_time_and_nothing_else. §8.2.1 now shows the second burial as an executed transcript; the transcript runner drops the memo after pre-burying fixtures, because the documents show a reader's first burial. Limits: exhume and graft do not remember, and the browser core has no store to keep one in. Core +59, to 8545.
