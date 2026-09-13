---
id: 183
title: Depth is the site's colour, and it is computed
type: feature
status: buried
milestone: necropolis
created: 2026-09-13
updated: 2026-09-13
priority: p1
effort: s
area: site
stratum: '0'
proof: Every spec page carries a depth nobody typed
---

The ramp from sulphur to magenta is the one piece of colour this project
invented rather than inherited from VGA. It appeared only inside code samples.

Everything with a depth should be stained by it, and the depth should be
**computed** rather than written into a table somebody has to remember to
update. `paint_nc` already stains every `@n` it finds, so the deepest stain in
a rendered page is the deepest stratum that page describes — which makes a
section's depth the join of its parts, the same rule
[§7.3.2](../spec/07-ledger.md) gives a trace.

```
03-lexical    depth 0
01-strata     depth 3
04-grammar    depth 5
09-prelude    depth 8
```

Nobody typed those.

## Acceptance criteria

- [x] A section's depth is derived from the section
- [x] Both tables of contents show it, stained
- [x] Every specification page says how far down it goes, before it is read

## 2026-09-13

The depth is the join of the page's parts, which is §7.3.2 applied to prose. It is also the reason it cannot go stale: a section that grows a deeper example gets a deeper gauge without anybody noticing it needed one.

## 2026-09-13

The memorial was tightened in the same commit -- the defensive footnote arguing that the project does not need him is gone, because a memorial does not argue.
