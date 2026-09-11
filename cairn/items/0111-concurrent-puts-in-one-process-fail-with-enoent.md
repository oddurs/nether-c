---
id: 111
title: Concurrent puts in one process fail with ENOENT
type: bug
status: buried
milestone: ledger
created: 2026-09-11
updated: 2026-09-11
priority: p0
area: crates/nether-ledger
proof: Sixteen threads putting the same object all succeed, as a test that fails when the bug is put back
---

## What happens

The temp path was `{hint}.{pid}`, so two threads writing the same object chose
the same path. Both wrote it, the first renamed it away, the second's rename
found an empty directory. Seven of sixteen threads failed.

## What should happen

7.5: concurrent writers need no coordination beyond each write being atomic.

Fixed in #32: a per-write counter in the temp name.
