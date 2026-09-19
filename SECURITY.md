# Security

## Supported versions

`v0.2.0` is the current release and the only one. Nothing is supported for
backports yet: the specification is a draft that changes without notice, the
crates are not published, and the tag is the version. When that changes it will
be said here and in `CHANGELOG.md`, and not before.

## Reporting a vulnerability

Report privately through GitHub Security Advisories:

**<https://github.com/oddurs/nether-c/security/advisories/new>**

Do not open a public issue for a vulnerability.

Expect an acknowledgement within seven days and an assessment within thirty. If
a report goes unanswered for two weeks, escalate by opening a public issue that
says only that you are waiting on a private report — no details.

## Who is trusted

Eight crates, a wasm build, a store meant to be shared between people who do
not trust each other, and foreign code at stratum 8. Seven boundaries, and for
each one: what crosses it, what is checked, what is assumed, and what is not
promised.

### 1. The author of a program

A `.nc` source you did not write, which you bury.

**Not trusted.** A program is a thing that asks; what it may reach is decided
by the invocation and not by the program.

| | |
| --- | --- |
| Crosses | source text, and the arguments it computes for prelude calls |
| Checked | the depth checker, before burial. `δ` is a set of strata ([§2](spec/02-calculus.md)), so a grant is not implied by any other grant — `descend net { write(…) }` is rejected, and `spec/mod.rs` holds that sample as `Illegal` |
| Checked | burial is bounded by fuel ([§6.4](spec/06-evaluation.md)) and by a nesting limit, so a program cannot run the burial out of stack or time |
| Checked | every string a program hands to `net` must be printable ASCII, and a host must look like a host, so a program cannot write its own HTTP headers or reach a party the invocation did not name |
| Assumed | nothing |
| Not promised | that a program which was *granted* a capability will use it well. That is what the grant meant |

### 2. The author of a trace

Bytes in a store, read by `exhume`, `lamp`, `strata` or `graft`.

**Not trusted, and this is the hostile surface.** A trace is the one input that
arrives as bytes and is parsed before anything else knows what it is.

| | |
| --- | --- |
| Crosses | the canonical encoding ([§7.1](spec/07-ledger.md)): values, nodes, spans, calls |
| Checked | every object is verified against the cairn it was fetched by, so a store that serves altered bytes is detected rather than believed |
| Checked | the decoder rejects non-canonical forms, over-long strata, malformed spans and trailing bytes; a decoder that accepted two byte strings as one value would break every reproducibility claim in the specification |
| Checked | a trace naming a hole this ledger does not hold is reported rather than skipped |
| Assumed | that `blake3` is collision-resistant. It is the only dependency in the tree and the only place this assumption is made |
| Not promised | that a trace is *true*. It says what some machine recorded. Whether that machine was honest is boundary 3 |

### 3. The machine a burial runs on

The disk, the network, the clock, and the source of entropy.

**Trusted for integrity, bounded for availability.** A provider is the thing
that answers, and the invocation says which providers exist.

| | |
| --- | --- |
| Crosses | file contents, HTTP responses, a pinned clock and target, drawn bytes |
| Checked | `disk` resolves every path under a root and refuses to climb out, textually and then through the filesystem, so a symlink already inside the root cannot point out of it |
| Checked | `net` opens a socket only to a host named by `--reach`, decided **before** the socket; responses are capped at 64 MiB of body and 64 KiB of headers, with a 30-second patience |
| Checked | `entropy` refuses a draw past 16 MiB and collapses on a negative one |
| Checked | every answer is written to the ledger **before** it reaches the program ([§1.4](spec/01-strata.md)), and the type of `Recorder::record` is the only way to make the proof a provider must return |
| Assumed | the filesystem, the clock and `/dev/urandom` are what they claim. A burial cannot check its own machine |
| Not promised | protection against a path that appears between the check and the act. The disk root is a check before an act, and a link created in that window wins. It stops a link that was already there, which is the one a program being buried can arrange for itself |

### 4. The author of a shared object

Whatever `--load` names, called at stratum 8 through `dlopen`.

**Not trusted, and not defended against.** This is the stratum that exists
because a language that cannot call C proves nothing, and
[§1.7](spec/01-strata.md) marks any trace that reached it, permanently.

| | |
| --- | --- |
| Crosses | a symbol name, an argument buffer, and whatever the callee writes back |
| Checked | a symbol is looked for only in objects `--load` named, in the order it named them |
| Checked | the witness is written **before** the call, so a callee that does not return still leaves a record of what was attempted |
| Checked | the trace is marked, `strata` reports `replayable: no` without being asked, and `exhume --replay` refuses it |
| Assumed | that the callee honours [§9.8.1](spec/09-prelude.md): that it writes no more than it was given room for, keeps no pointer after it returns, and returns |
| Not promised | anything at all. A callee that violates the protocol has the process. There is no sandbox and there is not going to be one |

### 5. Whoever else writes to the store

A store shared between people who do not trust each other.

| | |
| --- | --- |
| Crosses | objects, and the reverse index under `refs/` |
| Checked | the store is append-only and content-addressed, so a writer can add but cannot revise; everything read is verified against the name it was asked for |
| Assumed | that the filesystem gives each writer a whole write. Concurrency across processes is not yet settled — that is 0207 |
| Not promised | availability. Anyone who can write to a shared store can fill it |

### 6. The page a browser loads

`nether-wasm`, the Necropolis, and the module a page fetches.

| | |
| --- | --- |
| Crosses | source text in, a length-prefixed JSON block out |
| Checked | the module holds no capabilities. A browser has no world to grant, so every world-touching expression becomes a hole — §8.2's default, enforced by there being no provider to reach |
| Checked | the front end makes no external requests, and a test says so |
| Assumed | the page embedding the module calls it correctly. The exports take raw pointers and lengths |
| Not promised | that a hostile *page* cannot misuse its own module. It runs in that page's origin and can reach nothing else |

### 7. The build

| | |
| --- | --- |
| Crosses | one dependency: `blake3` |
| Checked | nothing is downloaded at build time. There is no image library, no font library, no markdown library and no framework; the encoders, the face, the HTTP client and the loader are in the tree |
| Checked | `unsafe` is `forbid` for the workspace and allowed in exactly two crates — `nether-foreign`, where Nether C calls out, and `nether-wasm`, where something else calls in |
| Assumed | the toolchain and `blake3` are what they claim |

## What counts

- **Memory unsafety, panics or unbounded allocation in the decoder.** It parses
  bytes from a store that may be shared between people who do not trust each
  other.
- **Non-canonical acceptance.** A decoder that accepts two different byte
  strings as the same value breaks every reproducibility claim in the
  specification. Treat it as a vulnerability, not a bug.
- **Capability escape.** Any way to reach a stratum without a `descend` that
  granted it, or to lower a value's depth other than through `seal` and
  `shade`.
- **Witness omission.** A path where a world-derived value reaches the program
  before its witness is written to the ledger.
- **Escaping a declared bound.** Reading outside the disk root, opening a socket
  to a host `--reach` did not name, or loading an object `--load` did not name.

## What does not count

- **A program you granted a capability doing what that capability permits.**
  `descend disk! { remove(p) }` removes a file. That is the grant.
- **Stratum 8 behaving badly.** That is what stratum 8 *is*, it is marked in the
  trace, and §1.7 says so.
- **Availability of a shared store.** Anyone who may write to it may fill it.
  Integrity is defended; availability is not.

## Known gaps

Named because a threat model that lists only defences is the security version
of a design document that lists only advantages.

- **`nether-foreign` allocates whatever a callee asks for.** On return code `1`
  the caller allocates `*out_len` bytes with no cap, so a hostile or buggy
  callee can abort the process by asking for more memory than exists.
  `net` and `entropy` both bound their answers; this one does not.
- **`nether-wasm` frees with an assumed capacity.** `nether_free` reconstructs
  a `Vec` with `capacity == len`, which holds for `Vec::with_capacity` today
  but is not guaranteed by its contract. Freeing with a layout other than the
  one allocated is undefined.
- **`nether-wasm` trusts its caller's lengths.** `nether_bury` builds a slice
  from a pointer and length the page supplies. A page that lies has its own
  module's memory, and nothing else.
- **`unrecorded` records a refusal it has not received.** The pre-call witness
  §9.8.1 asks for is written as `Refusal::Unreachable`, so a foreign call that
  then *succeeds* leaves a permanent witness saying the world said unreachable
  — content-identical to a genuine one. The ledger is append-only, so it stays.
- **Concurrent writers to one store are unsettled.** 0207.

## Where the boundaries are tested

| Boundary | Tests |
| --- | --- |
| 1 — program | `crates/nether-core/tests/calculus.rs`, `crates/nether-syntax/tests/spec/`, `crates/nether-world/tests/net.rs` |
| 2 — trace | `crates/nether-ledger/` encoding and store tests, `crates/nether-cli/tests/rites.rs` |
| 3 — machine | `crates/nether-world/tests/{disk,net,env,entropy}.rs` |
| 4 — foreign | `crates/nether-world/tests/unrecorded.rs`, `tests/foreign/said.c` |
| 5 — store | `crates/nether-ledger/src/store.rs` tests |
| 6 — browser | `crates/nether-cli/tests/rites.rs`, the browser tests in `scripts/task check` |
| 7 — build | `tests/attribution/run`, `scripts/decay`, `scripts/wasm` |
