---
id: 59
title: 'Proof: the three sample programs'
type: chore
status: buried
milestone: surface
assignee: Oddur Sigurdsson
depends_on:
- 57
created: 2026-09-10
updated: 2026-09-12
priority: p0
effort: s
area: tests/programs
stratum: '3'
proof: hello.nc, stamp.nc and build.nc compile verbatim from the spec
---

The stage 2 proof. The programs are lifted unchanged from the spec, so if
they need editing to compile, the spec was wrong and gets fixed first.

## 2026-09-12

The three programs are real files under tests/programs now, not fragments in a harness. Each one contains its specification fence as a contiguous substring, checked first, so lifted unchanged is a claim the test makes rather than one the commit message makes. Where a file has more in it — a header comment, or the compile that section 9.2 says a program supplies for itself — the extra is above the excerpt.

## 2026-09-12

stamp.nc is the interesting one: it compiles and is refused, and the refusal is section 1.6's error block character for character, rendered from the real file. That made the specification wrong twice, which is what the item said would happen. The block was depicting a file indented by two that section 1.6's own sample is not, and it was drawing a source line without the comment a diagnostic would print. Both fixed first, in their own pull requests.

## 2026-09-12

Area moved from tests/transcripts to tests/programs. The transcripts harness is Python and pins documentation samples by hash; it cannot compile Nether C, and nether bury does not exist yet. A Rust test that lifts the fence and runs the real pipeline proves the same thing today.
