---
id: 81
title: The second projection
type: feature
status: unmarked
milestone: futamura
depends_on:
- 80
created: 2026-09-10
updated: 2026-09-15
priority: p2
effort: l
area: lib/
stratum: '0'
proof: Burying the burier against the interpreter yields a working compiler
---

Specialise the burier to the interpreter: a compiler falls out.

## Delivery plan — 2026-09-15

### Starting point and scope

0080 is necessary but insufficient: the repository has no Nether C burier ready to specialize. This is research with a missing prerequisite, not a small invocation change.

### Steps

1. Define the burier/interpreter representations and the exact equation the generated compiler must satisfy.
2. Inventory missing language and self-hosted burier support; create bounded specification/implementation prerequisite items before attempting generation.
3. Once prerequisites land, specialize against the interpreter and compare generated results across multiple guest programs and answers.

### Acceptance and evidence

- [ ] A generated compiler, not a handwritten adapter, passes the agreed semantic corpus. Record generating inputs and cairns; keep this open if the required self-hosted specializer does not exist.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
