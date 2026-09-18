---
id: 265
title: 'exhume: --root, and no default for it'
type: feature
status: unmarked
milestone: world
created: 2026-09-17
updated: 2026-09-17
priority: p1
effort: m
stratum: 4
area: crates/nether-cli
proof: Granting disk without --root is an error, and two roots answer read("main.nc") differently on purpose
part_of: [262]
---

## Problem

`spec/08-rites.md` §8.3.4 now says `--root` names the one directory `disk` and
`disk!` resolve every path against, that it is required whenever either is
granted, and that there is no default. `nether exhume` has no such flag.

What it has instead is `crates/nether-cli/src/exhume.rs`:

```rust
let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
```

That is the default §8.3.4 rejects, for the reason §8.3.1 gives about `--clock`
and `--target`: the only default available is the directory the rite happened
to be run in, and a rite whose answer depends on where it was run is what §6.7
exists to prevent.

## Proposal

`--root <path>`, given at most once. Granting `disk` or `disk!` without it is
an error; giving it without granting either is an error, the way `--reach`
without `net` already is. `crates/nether-world`'s `Disk` already takes a root
and already refuses a path that climbs out of it textually and through a
symbolic link, so this is the invocation reaching what the provider has.

## Starting point and non-goals

`Disk::reading` and `Disk::writing` need nothing. The work is in `exhume.rs`'s
flag parsing, the error when a grant and a root disagree, and every test and
transcript that grants `disk` today.

Not in scope: repeating the flag. §8.3.4 says once and §90.2 records why.

## Delivery steps and dependencies

1. Parse and validate the flag, with the two errors above.
2. Thread it to `world_from` in place of `current_dir`.
3. Fix the callers: `crates/nether-cli/tests/rites.rs`, and the `console`
   transcripts in §8.3 and §6.6 if they grant the disk.
4. A test that two roots answer the same `read` differently, which is the
   observable the flag exists for.

## Which stratum does this reach?

4. It decides where a `write` lands.

## Acceptance criteria

- [ ] `--grant disk` without `--root` is a usage error naming §8.3.4
- [ ] `--root` without `--grant disk` is a usage error
- [ ] Two roots answer one `read("main.nc")` with two different values
- [ ] A path that climbs out of the root is `denied` and not `absent`

## Evidence to close

Record the tested commit and the exact invocations. `scripts/task check` must
pass with every existing disk-granting test and transcript updated, and the
diff should show what each of them was implicitly rooted at before.
