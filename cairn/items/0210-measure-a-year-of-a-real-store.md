---
id: 210
title: Measure a year of a real store
type: chore
status: unmarked
milestone: ossuary
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: l
area: ops/
stratum: '4'
proof: A year of a real store's size, object count and access distribution, published, with 7.6 rewritten to match
---

7.6 says there is no garbage collection and argues it is affordable. It also
admits the argument is a hope rather than a measurement, which has been true
for the whole life of the project.

A year of a store somebody actually uses settles it. The interesting number is
not the total — it is the shape: how much is never read again, and how soon.

## Delivery plan — 2026-09-15

### Starting point and scope

A real year needs an operator and stable measurement policy, not a synthetic capacity estimate.

### Steps

1. Choose a genuinely used store and owner; define privacy-safe size/count/access measurements.
2. Collect dated observations for a full year, recording outages, workload changes and gaps.
3. Publish methodology and data, then revise §7.6's affordability argument to match.

### Acceptance and evidence

- [ ] A measured year supports the published claims. Monthly snapshots are interim evidence; never backdate data or substitute extrapolation.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
