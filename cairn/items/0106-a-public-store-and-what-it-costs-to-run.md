---
id: 106
title: A public store, and what it costs to run
type: chore
status: unmarked
milestone: after
depends_on:
- 210
created: 2026-09-11
updated: 2026-09-15
priority: p2
effort: l
area: ops/
stratum: '6'
proof: A year of the real store's storage and egress, measured, published, and paid for by somebody
---

Section 7.6 says there is no garbage collection, argues that this is
affordable, and admits that the argument is a hope rather than a measurement.

A public store is where that hope meets a bill. Run one, measure it for a year,
publish the numbers, and either 7.6 stands or it gets rewritten by somebody
holding an invoice.

Includes the unpleasant questions a public content-addressed store has to
answer: what happens when somebody puts something illegal in it, and what
'immutable' means the day after that.

## Delivery plan — 2026-09-15

### Starting point and scope

This requires a willing operator, funding and a year of measurements. 0210 supplies the storage study; this item adds public-service egress, abuse handling and actual invoices.

### Steps

1. Before deployment, agree ownership, spending limits, retention/removal policy, privacy and incident response; use 0216's threat model.
2. Specify the minimal public read service using 0215, with transfer limits and integrity checks; secure explicit deployment authority.
3. Collect dated storage/egress/cost records and publish a full year's results and implications for §7.6.

### Acceptance and evidence

- [ ] A named operator has actually paid and published the measured year. No extrapolation, free-tier estimate or deployment from a planning task counts; external approval remains a gate.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
