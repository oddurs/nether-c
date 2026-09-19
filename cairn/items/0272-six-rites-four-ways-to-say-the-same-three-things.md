---
id: 272
title: Six rites, four ways to say the same three things
type: chore
status: buried
milestone: after
created: 2026-09-18
updated: 2026-09-18
priority: p2
effort: m
area: crates/nether-cli
stratum: '0'
proof: The core is smaller and one waiver is gone
---

Every rite opens the ledger, most of them resolve a name, and three of them
write nodes to the store. Each of those said so in six or seven lines, at
every site, in every file:

```rust
let store = match ledger() {
    Ok(store) => store,
    Err(e) => {
        eprintln!("nether: {e}");
        return FAILED;
    }
};
```

Twenty-four of those across six files. The shape of each rite was buried under
its own error handling, and three of the four `too_many_lines` waivers in the
tree were on rites whose length was mostly this.

## Four helpers, and one of them is a better sentence

`opened`, `named` and `kept` are the three things every rite does. `not_a_trace`
is the fourth, and it is not a deduplication — it is a **correction**. Three
rites read a trace before doing anything; `exhume` and `graft` said only

> `nether: 76c4b655 is not a trace`

which tells a reader they are wrong and not what they have. `strata` already
said which, and offered the rite that would show them:

> `nether: 76c4b655 is a witness, not a trace`
> `` `nether lamp 76c4b655` shows what is there ``

Unifying on the commonest version would have been a downgrade. That is the
sentence now, in all three.

## What it cost, and what it did not

`bury` and `exhume` needed `Result<(), ExitCode>` inner functions to use `?`.
The other four did not: two lines at the call site is already the win, and
changing four more signatures to save one line each is churn.

## Acceptance criteria

- [x] One way to open the ledger, resolve a name, keep a node, and refuse a
      non-trace
- [x] The core is smaller
- [x] No rite keeps a `too_many_lines` waiver it no longer needs

## 2026-09-18

The best outcome was one nobody asked for: bury's too_many_lines waiver is now unfulfilled, because inter shrank below the threshold once its error handling stopped being longer than its argument. The excuse went with the length.

## 2026-09-18

Unifying the not-a-trace sentence went the other way from the obvious one. Two rites said the short thing and one said the useful thing, so the useful one won -- deduplication that picks the commonest version rather than the best is how a codebase gets worse quietly.
