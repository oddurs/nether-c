---
id: 167
title: Exhuming a trace loses what the program deposited
type: bug
status: unmarked
milestone: rites
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: crates/nether-cli
stratum: '1'
proof: A deposit recorded by a burial is still there after exhuming and after grafting
---

## Measured

```console
$ nether lamp <buried>
a deposit that happened

$ nether lamp <exhumed>
nothing was deposited
```

[§6.8](../spec/06-evaluation.md) makes depositing the only thing a program can
do with a value it wants kept, and
[§8.4](../spec/08-rites.md) makes a trace's deposits how its output is read.
Exhuming throws it away. Grafting does the same.

## Where

One line below the line that gets it right:

```rust
witnesses: recorded,                      // carried forward
deposits: residue.deposits.clone(),       // not
```

The residue is right not to re-deposit — that deposit already happened, and
doing it again would record it twice. What the new trace has to do is name what
the old one recorded, exactly as it does for witnesses.

## Acceptance criteria

- [ ] A deposit survives exhuming
- [ ] A deposit survives grafting
- [ ] A deposit made *by* the second burial is recorded once, not twice
