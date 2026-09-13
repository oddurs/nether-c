---
id: 176
title: The stratum-8 proof builds one object into one path from five tests
type: bug
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: crates/nether-world
stratum: '8'
proof: The stratum-8 proof runs green under repetition
---

```
thread 'foreign_code_is_called_and_what_it_said_is_recorded' panicked at
crates/nether-world/tests/unrecorded.rs:46:
  the object loads: "/tmp/nether-foreign-proof/libsaid.so: file too short"
```

`object()` runs `tests/foreign/build` into `$TMPDIR/nether-foreign-proof`, and
five tests call it. Cargo runs them in parallel, so `cc` is writing the file
while another test's `dlopen` is reading it.

It passed on the machine it was written on and failed on CI, which is the
shape of every race: the bug was there both times.

## Acceptance criteria

- [x] Two tests that build the object cannot write the same path
- [x] The proof runs green under repetition

## 2026-09-13

A directory per test rather than a lock. Five cc runs is a second and a half and is correct by construction; a lock is a thing that can be got wrong, and this is a test.
