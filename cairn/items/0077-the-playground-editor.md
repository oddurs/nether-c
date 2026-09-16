---
id: 77
title: The playground editor
type: feature
status: unmarked
milestone: necropolis
depends_on:
- 60
- 75
created: 2026-09-10
updated: 2026-09-15
priority: p1
effort: l
area: web/necropolis
stratum: '0'
proof: Write, bury, and watch holes fill as strata are granted, entirely client-side
---

Edit, bury, see the trace. Grant a stratum and watch the holes fill. This is
the demo, and it needs to work on a phone.

## Delivery plan — 2026-09-15

### Starting point and scope

Source editing, worker burial, cancellation and mobile navigation already exist. The missing portion is capability/answer interaction; WASM currently has no world provider. Scope this to the existing editor, not a new editor framework.

### Steps

1. Audit the existing source disclosure and tests/necropolis/browser.mjs before changing controls.
2. Specify how client-side supplied answers are labelled, granted and recorded, including refusal and revocation; a grant alone cannot manufacture an answer.
3. Implement the smallest specified flow with stale-worker protection and phone/keyboard tests.

### Acceptance and evidence

- [ ] Edit a program, bury it, explicitly supply permitted answers and see its holes change entirely client-side. No silent server calls, simulated witnesses presented as real, or browser capability escalation.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
