---
id: 102
title: 'Editor support: spans, depth and provenance'
type: feature
status: buried
milestone: after
assignee: Oddur Sigurdsson
claimed: 2026-11-07
depends_on:
- 58
created: 2026-09-11
updated: 2026-11-07
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

- [x] A real editor demonstrates the stated hover and provenance task on a real trace. No reimplemented depth checker, invented source offsets or implied complete LSP coverage.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

## Evidence — 2026-11-07

- Specified `nether lamp <cairn> --at <byte> [--source <path>] [--depth <n>] [--json]` in `spec/08-rites.md` §8.4.1 and recorded the rejected stateful language server alternative in `spec/90-rationale.md` §90.2 (`a28e0a6`).
- Implemented `--at <byte>` and `--source <path>` in `crates/nether-cli/src/lamp.rs` and the thin Vim/Neovim adapter `tools/vim/nether.vim` (`:NetherTrace`, `:NetherHover`/`K`, `:NetherOrigin`/`gd`, `:NetherProvenance`/`gp`).
- Verified in `crates/nether-cli/tests/rites.rs` (`lamp_at_reports_depth_origin_provenance_and_refuses_stale_buffers` and `real_vim_editor_shows_hover_depth_jumps_to_origin_and_walks_provenance`) driving real `vim` on non-ASCII UTF-8 source (`// Βίος — Þórður`): hover depth (`depth 3   disk   Str`), `descend disk` and `read("saga.txt")` origin navigation, witness and deposit provenance walks in `__nether_lamp__`, stale buffer refusal (`does not match trace source`), repeated edits re-buried at shifted offsets, and absent store object refusal (`is in this store`).
- Remaining limits: inspection operates on an already buried trace whose `trace.source` is in the store; unsaved edits are refused until buried again rather than shifted heuristically.
