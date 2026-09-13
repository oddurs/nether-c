---
id: 74
title: WASM build of the core
type: chore
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
depends_on:
- 46
created: 2026-09-10
updated: 2026-09-13
priority: p1
effort: l
area: crates/nether-core
stratum: '0'
proof: A trace buries in the browser with no server round trip
---

nether-core and nether-ledger to wasm32, small enough to ship to a browser
without apology.

## 2026-09-13

358 KB on disk, 147 KB compressed, which is what a browser downloads and so what .wasm-ceiling holds. A separate [profile.wasm] rather than a smaller release: the binary is built for speed and the module for the length of a download.

## 2026-09-13

No bindings generator. Four exports over linear memory, and every returned block is a little-endian u32 length and then that many bytes -- the same bargain §9.8.1 strikes at the other boundary, for the same reason.

## 2026-09-13

nether-world is absent on purpose. A browser has no world to grant, so every world-touching expression becomes a hole, which is §8.2's default anyway.

## 2026-09-13

The proof is that the two burials agree. scripts/wasm reads the module's export section to check it loads; the_browser_buries_what_the_rite_buries checks it agrees with nether bury on the same source, cairn for cairn and hole for hole. A second burial giving a different cairn would make the Necropolis a picture of a different program.

## 2026-09-13

The module is built, not committed. The first version committed it and compared bytes in wasm:check, which failed on CI: a Rust release build is not byte-reproducible across machines, so a committed copy can only be checked by rebuilding it -- and then the committed copy is doing nothing. What is checked is what the build produces.

## 2026-09-13

.wasm-ceiling is a budget, not a high-water mark. The Decay Rule can be pinned to the line because a line count is the same everywhere; the same source compresses to 147512 bytes here and 147529 on CI, so a ceiling set to the last measurement says the module grew when nothing changed. 150 KiB, chosen, with the headroom printed every time.
