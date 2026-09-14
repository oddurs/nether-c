---
id: 222
title: Mechanize the depth lattice
type: chore
status: unmarked
milestone: assay
created: 2026-09-13
updated: 2026-09-13
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
