---
id: 75
title: The trace graph browser
type: feature
status: starved
milestone: necropolis
assignee: Oddur Sigurdsson
claimed: 2026-09-15
depends_on:
- 74
created: 2026-09-10
updated: 2026-09-15
priority: p0
effort: xl
area: web/necropolis
stratum: '0'
proof: A stranger can navigate a real trace without instruction
---

The DolDoc inversion. Terry's IDE showed you living code; this shows you the
dead. Click any value, see the graph around it.

## 2026-09-15

Delivery in three PRs: canonical graph export (0243), navigation and inspector, then layout and browser verification. Keep the unfamiliar-person navigation proof open until directly observed; automated browser checks are separate evidence.

## 2026-09-15

Implementation delivered through canonical graph export (PR 164), navigation and the actual WASM thread-boundary fix (PR 165), and responsive layout with required Chrome regression checks (0245). The default page opens a real trace with holes and deposits, exact values and byte-accurate source spans. No application dependencies were added. The core is 7996 lines versus 8017 before this work; the WASM module is 146840 compressed bytes, below its unchanged 153600-byte budget. Starved only on the original human proof: no unfamiliar participant has been observed navigating unaided. The procedure is recorded in web/necropolis/README.md. Do not replace this proof with passing automated tests or close the item without that observation.

## Delivery plan — 2026-09-15

### Starting point and scope

The browser is implemented; only the unfamiliar-person observation remains. See web/necropolis/README.md and items 0243–0245. Do not rebuild it or substitute automation for this proof.

### Steps

1. Keep the existing page and navigation task fixed for the observation.
2. Ask an unfamiliar participant to find a hole, inspect its question, follow an argument and return, without coaching.
3. Record the tested commit, device, unaided result and hesitation points here; file concrete failures separately.

### Acceptance and evidence

- [ ] An actual observation satisfies the README procedure. Remain starved until a participant is available; 0078 is a separate study.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
