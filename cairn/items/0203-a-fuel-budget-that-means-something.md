---
id: 203
title: A fuel budget that means something
type: spec
status: unmarked
milestone: quickening
depends_on:
- 199
created: 2026-09-13
updated: 2026-09-15
priority: p2
effort: s
area: spec/06-evaluation.md
stratum: '0'
proof: The default budget is justified by a measurement rather than chosen
---

`DEFAULT_FUEL` is 1,000,000 because it looked about right. 6.4 requires the
budget to be deterministic and finite and says nothing about what it should be.

Once burial is profiled, pick it from evidence: large enough that no reasonable
program hits it, small enough that a runaway stops in a second rather than a
minute. Then say which it is in 6.4.

## Delivery plan — 2026-09-15

### Starting point and scope

The one-million default appears in CLI and WASM paths. Fuel counts expression starts, not seconds; the interpreter already needs explicit larger budgets.

### Steps

1. Use 0199 to measure ordinary and runaway programs under existing §6.4 accounting.
2. Justify defaults and overrides, including differences in browser/native resource limits.
3. Specify policy changes first, then align call sites and pin exhaustion-span regressions.

### Acceptance and evidence

- [ ] Defaults are backed by named workloads and machines. Do not promise a universal one-second timeout or redefine fuel steps for speed.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
