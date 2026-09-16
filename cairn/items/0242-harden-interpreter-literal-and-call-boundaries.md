---
id: 242
title: Harden interpreter literal and call boundaries
type: bug
status: buried
milestone: quickening
assignee: Oddur Sigurdsson
created: 2026-09-15
updated: 2026-09-15
priority: p2
stratum: '0'
area: lib
proof: Regression tests reject malformed calls and literals before effects, respect lexical call shadowing, and round-trip every signed decimal boundary without overflow.
---

## What happens

Empty decimal fields become zero and oversized fields wrap. A call can bypass
a same-named local value. Calling a global value collapses as malformed syntax
instead of reporting a semantic error. Trailing commas are accepted. Cursor
bounds add before checking and can overflow.

## What should happen

Reject malformed numeric fields before arithmetic can wrap; round-trip I64's
minimum without a positive intermediate. Respect lexical call resolution,
report non-callable values before effects, and enforce the existing list grammar.
Check cursor bounds by subtraction before constructing the end offset.

## Reproduction

The interpreter regression suite covers missing digits, both overflow edges,
shadowed globals and prelude names, trailing argument and parameter commas,
and cursor offsets at I64's maximum. Existing staging and fuel proofs remain
unchanged. This work reaches stratum 0 and adds no trusted-core code.

## Cost

Extra checks add about seven percent to the fixed sample host-step counts;
all remain inside the existing budgets. The alternative, trusting numeric
fields and overflowing end offsets, silently accepts malformed records.

## 2026-09-15

Thirty interpreter regressions pass, including staged residue and replay. Full scripts/task check passes: workflow tests, documentation fixtures, generated assets, WASM budget and unchanged 8017-line trusted core. Sample fuel budgets are unchanged; measured counts are recorded in lib/README.md.
