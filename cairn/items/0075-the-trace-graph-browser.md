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
