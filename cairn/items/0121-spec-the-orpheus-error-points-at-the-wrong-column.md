---
id: 121
title: 'Spec: the Orpheus error points at the wrong column'
type: spec
status: buried
milestone: calculus
depends_on:
- 14
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: s
area: spec/01-strata.md
proof: The location line and the caret row in section 1.6 name the same column
---

## The mistake

`spec/01-strata.md` §1.6 prints the Orpheus error:

```
  --> stamp.nc:14:11
   |
14 |   I64   n       = look(reply).len;
   |                   ^^^^^^^^^^^ this shade came from `fetch` at stratum 5
```

The caret row is right: eleven carets, the width of `look(reply)`, starting
under the `l` at column 19 of the source line. The location line says column
11. The two disagree, and 11 is the caret *count* rather than the caret
*position*.

`spec/06-evaluation.md` §6.4 prints the same shape correctly, so this is one
number and not a format question.

Found while building 0051, whose proof is that the Orpheus rule behaves as the
codex decided including the error text — which means the error text has to be
right to reproduce.

## Acceptance criteria

- [x] §1.6's location line and caret row name the same column
- [x] The site is rebaked

## 2026-09-12

Reopened once, for the last time: the error block was depicting a stamp.nc whose statements are indented by two, and section 1.6's own sample has them at unit level. The block now depicts a file that exists — tests/programs/stamp.nc in 0059 — so the three numbers in it are checked against a real file rather than against an imagined one.
