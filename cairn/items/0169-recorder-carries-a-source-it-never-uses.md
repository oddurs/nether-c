---
id: 169
title: Recorder carries a source it never uses
type: chore
status: buried
milestone: world
created: 2026-09-13
updated: 2026-09-13
priority: p2
effort: s
area: crates/nether-world
stratum: '0'
proof: Every field on Recorder is read by something
---

## Dead, and dressed as a rule

`Recorder` holds a `source` and offers a `source()` for it. `record` takes the
span — which carries its own source, because that is what
[§7.3](../spec/07-ledger.md) makes a span — and never reads the field. Nothing
else calls the accessor.

The doc comment justifies the field this way:

> Holds the store the answer goes to and the cairn of the source the question
> was asked from, because a span names its source by cairn (§7.3).

Which is true of the span and not of the field. A comment that explains
something the code does not do is worse than no comment, because it answers the
question a reader would otherwise have asked.

`-D dead-code` says nothing: it is `pub` on a `pub` struct, so it is API rather
than dead code. Nothing being API is worse than nothing being code.

## Acceptance criteria

- [x] Every field on `Recorder` is read by something
- [x] `Recorder::new` takes what it uses

## 2026-09-13

The tests were already writing Recorder::new(&store, span.source) -- passing the field the span carries. That is the whole argument for the field not existing.
