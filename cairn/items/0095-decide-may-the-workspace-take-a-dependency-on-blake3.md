---
id: 95
title: 'Decide: may the workspace take a dependency on blake3?'
type: spec
status: buried
milestone: ledger
created: 2026-09-10
updated: 2026-09-10
priority: p0
effort: s
area: Cargo.toml
proof: The rule is written down in spec/90-rationale.md and in CLAUDE.md, and names what it permits and what it does not
---

## The question

The repository has no third-party dependencies anywhere, and that has been a
stated principle rather than an accident. Section 7.2 specifies blake3.

## The decision

Permitted. The rule is narrower than "no dependencies":

> Write your own encoders, parsers, renderers and formats. Do not write your
> own cryptography.

An encoder that is subtly wrong produces a file somebody notices. A hash
function that is subtly wrong produces cairns that collide, and the failure
surfaces years later underneath every reproducibility claim in the
specification.

Recorded in spec/90-rationale.md. `blake3` is expected to be the only
dependency for a long time; every addition after it needs its own entry there.
