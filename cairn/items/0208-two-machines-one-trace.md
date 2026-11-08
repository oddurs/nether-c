---
id: 208
uid: c316d8e9-74e8-4682-b987-2c6d0f0cca3a
title: Two machines, one trace
type: feature
status: buried
milestone: cortege
assignee: Oddur Sigurdsson
claimed: 2026-11-07
closed_at: 2026-11-07
depends_on:
- 215
created: 2026-09-13
updated: 2026-11-07
priority: p2
effort: xl
area: crates/nether-world
stratum: '6'
proof: Two machines each answer half a trace's holes, and the sealed result is byte-identical to one machine answering all of them
---

## The idea

A trace is a set of questions and a residue. Nothing says one machine has to
answer all of them.

Content addressing verifies the objects exchanged; it does not make merging
answers trivial. Two machines can observe different answers to the same
question. Assignment, conflicting witnesses and deterministic merge order
must be specified before distribution.

## Why it is xl anyway

Moving objects between stores is a protocol, and a protocol is a format, and
this project freezes formats carefully. It should not be invented in a hurry.

## Delivery plan — 2026-09-15

### Starting point and scope

Machines can answer the same question differently. Content addressing names disagreement; it does not resolve it.

### Steps

1. On top of 0215, specify assignment, answer identity, duplicate delivery, conflicts and merge order in a separate PR.
2. Use two isolated workers with deterministic recorded answers and reject conflicting witnesses.
3. Exercise retries, missing objects and interrupted transfer; replay the merge without world capabilities.

### Acceptance and evidence

- [x] The two-machine result matches serial for the same recorded answers. No promise of equality for independently sampled worlds or exactly-once external effects.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-07

Specified hole-slot assignment (`0..holes.len()`), witness identity verification (`(stratum, call, span)` against `holes[slot]` plus readable answer value), idempotent duplicate delivery, conflict rejection, and capability-free ascending-slot merge order in `spec/06-evaluation.md` §6.6 and `spec/90-rationale.md` §90.2 (PR #247, `fefc8b2`). Implemented `World::ask_slots` and `Replay::merge` (`MergeError`) in `crates/nether-world` (stratum 6).

Verified with `cargo test -p nether-world --test replay` and `cargo test -p nether-cli --test rites two_machines_each_answer_half_a_traces_holes_and_seal_byte_identically_to_one`: two isolated worker stores imported via `Store::import` each answer half of a trace's holes across strata 3 and 6 (`read` and `post`), export portable bundles (`Store::export`), and merge in a capability-free coordinator store to produce a sealed trace byte-identical to a single serial `nether exhume`, which then passes `nether exhume --replay` (`identical.`). Retried duplicate deliveries, reversed delivery orders, interrupted transfers, missing objects, mismatched witnesses, open earlier duplicate-call act slots, and conflicting witnesses are all exercised. Core line count rose from 9202 to 9314 (+112 lines for `World::ask_slots`, `Replay::merge`, and `MergeError`). Remaining limits: no equality is promised when workers independently sample nondeterministic or mutable worlds with different answers, and content addressing does not prevent a retried act-stratum worker from repeating an external effect before its witness reaches the coordinator.
