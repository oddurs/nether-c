---
id: 82
title: The third projection
type: feature
status: unmarked
milestone: futamura
depends_on:
- 81
created: 2026-09-10
updated: 2026-09-15
priority: p2
effort: xl
area: lib/
stratum: '0'
proof: A generated compiler beats the interpreter it came from, on that interpreter's own test suite
---

Specialise the burier to itself: a compiler generator. The stated end of the
road, and the only claim in this roadmap that might turn out to be false.
Recording it as an item means we have to either do it or close it as
`unrecorded`, in public.

## Delivery plan — 2026-09-15

### Starting point and scope

The third projection needs the representable specializer identified in 0081. Speed is an additional measurement, not evidence of compiler-generator correctness on its own.

### Steps

1. State the self-specialization equation and demonstrate that the specializer can consume its own representation.
2. Generate a compiler generator, then generate a compiler for the interpreter; retain reproducible input/output cairns.
3. Verify the interpreter corpus before timing generated versus interpreted programs with fixed inputs, fuel and machine details.

### Acceptance and evidence

- [ ] Both the compiler-generator derivation and the original performance proof hold. Report failures honestly; do not substitute a manually optimized interpreter or close a conjecture as proved.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
