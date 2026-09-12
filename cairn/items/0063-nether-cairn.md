---
id: 63
title: nether cairn
type: feature
status: buried
milestone: rites
assignee: Oddur Sigurdsson
depends_on:
- 22
created: 2026-09-10
updated: 2026-09-12
priority: p1
effort: s
area: crates/nether-cli
stratum: '1'
proof: Verifying a tampered object fails loudly and names what changed
---

Name a thing by its content; verify that a name still holds.

## 2026-09-12

The first rite that does something. nether cairn names a file by its contents as the Bytes they are — reading a file as a Str would mean deciding what to do about one that is not UTF-8, and there is nothing to decide: the name of a thing is the name of what it holds.

## 2026-09-12

Verification reports which of the three things happened rather than that something did. Bytes that hash to something else are answered with the hash they do have; bytes that hash right and do not decode are section 7.7's case and say so; a prefix that names two objects is refused rather than resolved. The store already made those distinctions and this is the rite that says them out loud.

## 2026-09-12

The ledger is NETHER_STORE, or .nether beside the work. Section 07 does not say where a store sits and does not need to: where it sits changes nothing about what is in it, and a trace means the same thing wherever it was written.
