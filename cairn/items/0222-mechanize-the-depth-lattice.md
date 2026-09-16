---
id: 222
title: Mechanize the depth lattice
type: chore
status: unmarked
milestone: assay
created: 2026-09-13
updated: 2026-09-15
priority: p0
effort: xl
area: proof/
stratum: '0'
proof: Monotonicity and ambient soundness are proved in a proof assistant, not sampled
---

2.4 states two properties an implementation MUST preserve, and 0052 tests them
generatively over ten million programs. Ten million is a lot of programs and it
is not all of them.

The rules are eleven lines. That is small enough to mechanize, and a language
that asks people to trust a depth is in a poor position to say the depth is
probably right.

## Delivery plan — 2026-09-15

### Starting point and scope

Generative calculus tests are not mechanization. A proof environment is a development dependency requiring a rationale decision.

### Steps

1. Compare small suitable proof environments, record rejected alternatives and pin the selected toolchain outside runtime.
2. Transcribe section 02's judgments and assumptions without strengthening premises for convenience.
3. Prove monotonicity and ambient soundness; add a scripts/task proof check rejecting admitted obligations.

### Acceptance and evidence

- [ ] Clean-environment theorem checking passes with explicit assumptions and model scope. No unacknowledged axioms, placeholders or substitution of sampled tests.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
