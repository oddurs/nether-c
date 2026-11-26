---
id: 235
uid: 1d947add-3320-433f-b5b7-d158ac87505b
title: Ship the face as something somebody else can use
type: chore
status: buried
milestone: face
depends_on:
- 231
- 232
created: 2026-09-14
updated: 2026-11-26
closed_at: 2026-11-26
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

- [x] A published release has usable font files and an explicit licence link. Local packaging alone is not completion; publication must follow the repository release authority.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## 2026-11-26

Proved on release tag v0.3.0 (commit c48d163fd8dfc5ed16cbbd45d8077f0527550b8f, https://github.com/oddurs/nether-c/releases/tag/v0.3.0). Downloaded nether-v0.3.0.woff, nether-bold-v0.3.0.woff, LICENSE, and SHA256SUMS from the published release and compared them byte-for-byte with the tagged source tree: nether-v0.3.0.woff and site/nether.woff match (SHA-256 e1186ecf06dd6ca7dbe0add031e3ae0cb2e231f0e3309fb336e72bcb5c9b3343), nether-bold-v0.3.0.woff and site/nether-bold.woff match (SHA-256 4bbb242477ea49cc91a8704f8e9e09ab4d24ea4b6b23752290d217b532bfaed2), and LICENSE matches the repository MIT licence (SHA-256 08c2d9fa3561617521f5d3c258d7d03d439f908f47eecda4b5f3800aac442c5a). The specimen page (site/specimen.html) documents both weights, MIT licence, @font-face usage, and links to the releases page. Remaining limit: WOFF only; WOFF2 is not packaged.
