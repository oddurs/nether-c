---
id: 207
uid: 8a40025a-b159-4413-9eb1-8a5fe6a0f809
title: Sixteen processes, one store
type: chore
status: buried
milestone: cortege
created: 2026-09-13
updated: 2026-11-07
priority: p1
effort: m
area: crates/nether-ledger
stratum: '4'
proof: Sixteen processes burying and exhuming against one store for a minute leave it consistent, checked by fsck
---

The store's concurrency tests use threads. Processes have separate address
spaces but still share the OS page cache. The rename-based atomicity story
must survive independent processes and interrupted writers too.

A minute of real contention, then a consistency check. This is the test that
would have caught the two ordering bugs review found in the store, and it
should exist before anybody runs a shared one.

## Delivery plan — 2026-09-15

### Starting point and scope

Store tests already use threads. Separate processes also share the OS page cache; the missing proof is process contention and interruption.

### Steps

1. Start sixteen child processes against one disposable store for at least sixty measured seconds.
2. Mix overlapping puts, reads and reverse-edge queries; capture seeds, exits and completed operations.
3. Use 0214's read-only checker or a specified equivalent after completion; separately exercise terminated writers.

### Acceptance and evidence

- [x] Committed objects and reverse edges remain consistent after real contention. Timeouts, killed children and partial work cannot silently count as success.
- [x] Record the tested commit, exact checks or observation, and any remaining limits here before closing.

### Evidence — 2026-11-07

- Base commit: `8eb5520` (`crates/nether-ledger/tests/proofs.rs`).
- Tests added:
  - `sixteen_processes_contend_on_one_store` (runs on every commit, 16 child processes contending on a shared disposable `Store` with overlapping and worker-distinct trace subgraphs, `get` reads across periodic `Store::open` re-opens, and `referrers` queries, followed by `Store::audit` and full verification of every committed object, edge and trace).
  - `terminated_writers_leave_committed_objects_and_audit_clean` (spawns 8 endless writer child processes that are killed mid-write via `SIGKILL` alongside 8 live reader/writer child processes, then verifies `store.audit().unwrap().is_clean()` and that every trace committed before `SIGKILL` is in `audit.healthy_traces`).
  - `sixteen_processes_one_store_for_a_minute` (`#[ignore]`, run via `scripts/task proofs`): 16 child processes ran for `60.24s` measured wall time (`359,412` puts, `37,820` gets, `37,820` `referrers` queries; `135,622` unique objects, `300,896` unique reverse edges, `18,804` healthy traces; `Store::audit` clean, `tmp/` empty):
    - `w00` `exit=0` `seed=0x9c3079b97f4a7c15` `puts=22465` `gets=2363` `refs=2363` `traces=1181`
    - `w01` `exit=0` `seed=0x3e69f372fe94f82a` `puts=22526` `gets=2372` `refs=2372` `traces=1210`
    - `w02` `exit=0` `seed=0xd8a16d2c7ddf743f` `puts=22233` `gets=2342` `refs=2342` `traces=1203`
    - `w03` `exit=0` `seed=0x7adae6e5fd29f054` `puts=22461` `gets=2360` `refs=2360` `traces=1155`
    - `w04` `exit=0` `seed=0x1512609f7c746c69` `puts=22492` `gets=2369` `refs=2369` `traces=1214`
    - `w05` `exit=0` `seed=0xb74bda58fbbee87e` `puts=22381` `gets=2356` `refs=2356` `traces=1195`
    - `w06` `exit=0` `seed=0x518354127b096493` `puts=22153` `gets=2332` `refs=2332` `traces=1183`
    - `w07` `exit=0` `seed=0xf3bccdcbfa53e0a8` `puts=22723` `gets=2387` `refs=2387` `traces=1163`
    - `w08` `exit=0` `seed=0x8df44785799e5cbd` `puts=22695` `gets=2386` `refs=2386` `traces=1181`
    - `w09` `exit=0` `seed=0x2c2dc13ef8e8d8d2` `puts=22094` `gets=2324` `refs=2324` `traces=1162`
    - `w10` `exit=0` `seed=0xce653af8783354e7` `puts=22486` `gets=2368` `refs=2368` `traces=1210`
    - `w11` `exit=0` `seed=0x689eb4b1f77dd0fc` `puts=22258` `gets=2346` `refs=2346` `traces=1218`
    - `w12` `exit=0` `seed=0x0ad62e6b76c84d11` `puts=22675` `gets=2386` `refs=2386` `traces=1201`
    - `w13` `exit=0` `seed=0xa50fa824f612c926` `puts=22504` `gets=2370` `refs=2370` `traces=1212`
    - `w14` `exit=0` `seed=0x474721de755d453b` `puts=22568` `gets=2373` `refs=2373` `traces=1178`
    - `w15` `exit=0` `seed=0xe1709b97f4a7c150` `puts=22698` `gets=2386` `refs=2386` `traces=1178`
- Remaining limits: local POSIX/APFS filesystem semantics; network filesystems that do not provide atomic same-directory `rename` or coherent `O_APPEND` for 32-byte records are outside §7.5.1's assumptions, and a writer killed mid-`put` can leave an unreferenced partial file in `tmp/` or a dangling 32-byte record in `refs/` (ignored by `referrers` and `audit`).
