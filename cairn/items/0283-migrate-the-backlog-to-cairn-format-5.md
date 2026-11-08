---
id: 283
uid: 73039a5f-fbdb-4967-992d-0181fa3262db
title: Migrate the backlog to cairn format 5
type: chore
status: buried
milestone: assay
assignee: Oddur Sigurdsson
created: 2026-11-07
updated: 2026-11-07
closed_at: 2026-11-07
priority: p1
effort: s
stratum: '0'
area: cairn
proof: cairn new, set and close write to this project, and cairn migrate reports every item kept its body and every other value
---

## Starting point and scope

## Steps and prerequisites

1.

## Acceptance and evidence

- [ ] State the measurable result, exact check and evidence to retain.
- [ ] Separate preparation from any human, external-service or elapsed-time gate.

## 2026-11-07

The installed cairn 1.0.0-alpha.1 writes only format 5, and this project was format 3, so cairn new, set and close refused. 0282 had to be written by hand. Format 5 adds a uid tag to each item; numbers, filenames, references and bodies are unchanged. cairn migrate verified 270 items and 151 references.
