---
id: 102
title: 'Editor support: spans, depth and provenance'
type: feature
status: unmarked
milestone: after
depends_on:
- 58
created: 2026-09-11
updated: 2026-09-15
priority: p1
effort: l
area: tools/
stratum: '3'
proof: Hovering a value in an editor shows its depth and walks its provenance, in a real editor, on a real trace
---

Depth is the language's whole point and it is invisible in a plain text buffer.

An editor that shows the depth of the expression under the cursor, and the
`descend` responsible for it, turns the central idea from something you reason
about into something you can see. The same is true of provenance: `nether lamp
--provenance` in a terminal is good; the same walk inline in the editor is
what makes people understand it.

Language Server Protocol is the obvious vehicle. The spans are already
load-bearing throughout the compiler because every diagnostic and every hole
points at source, so most of what a server needs will already exist.

## Delivery plan — 2026-09-15

### Starting point and scope

Byte spans and CLI inspection exist; do not assume a full language server is required. Start with crates/nether-cli/src/lamp.rs, source-span tests and the browser's UTF-8 handling.

### Steps

1. Specify one editor adapter and a bounded inspection protocol for an already buried trace; define stale-buffer versus named-source behavior.
2. Implement hover depth, source origin and recorded-reference navigation using existing compiler output.
3. Exercise non-ASCII source, absent objects, stale buffers and repeated edits in the chosen editor.

### Acceptance and evidence

- [ ] A real editor demonstrates the stated hover and provenance task on a real trace. No reimplemented depth checker, invented source offsets or implied complete LSP coverage.
- [ ] Record the tested commit, exact checks or observation, and any remaining limits here before closing.
