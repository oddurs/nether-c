---
id: 235
title: Ship the face as something somebody else can use
type: chore
status: unmarked
milestone: face
depends_on:
- 231
- 232
created: 2026-09-14
updated: 2026-09-15
priority: p2
effort: s
area: .github/workflows
stratum: '0'
proof: A release carries nether.woff and a stated licence for it, and the specimen page links to it
---

The face is 3.2kB, drawn from nothing, and covers a monospace ASCII range plus
the mathematical and box-drawing characters the specification needs. People
will want it.

A release should carry it as an asset with its licence stated plainly —
`LICENSE` covers the repository and a font is the kind of thing people need to
be told about separately before they will put it in their own page.

## Delivery plan — 2026-09-15

### Starting point and scope

Release packaging is separate from drawing the face. Use the existing licence and generated assets; do not introduce a font-service dependency.

### Steps

1. After 0231 and 0232, document the face names, weights, version, licence and minimal CSS usage.
2. Extend the existing release path to attach deterministic WOFFs with checksums, and link the specimen to the versioned assets.
3. Download an actual release asset and compare its checksum with the tagged source build.

### Acceptance and evidence

- [ ] A published release has usable font files and an explicit licence link. Local packaging alone is not completion; publication must follow the repository release authority.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
