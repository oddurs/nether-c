---
id: 84
title: 'site/bake: render the specification to HTML'
type: feature
status: buried
milestone: lamp
depends_on:
- 23
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: l
area: site/bake
proof: Editing a file in spec/ and running site/bake changes the site; no spec content is duplicated anywhere
---

One file of standard library Python, no dependencies, no framework, no
lockfile. Reads `spec/*.md`, renders a markdown subset, writes
`site/spec/*.html`.

The output is committed. Anybody who wants to read the site can read the site,
and the whole toolchain is one file they can also read.

`site/bake --check` fails if what is committed is stale, which is how the
documentation is stopped from drifting.
