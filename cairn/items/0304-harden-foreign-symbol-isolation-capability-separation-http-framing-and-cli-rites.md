---
id: 304
uid: a9384a4a-6d34-4009-aca2-b87f2a18e325
title: Harden foreign symbol isolation, capability separation, HTTP framing, and CLI rites
type: feature
status: buried
milestone: warden
assignee: Oddur Sigurdsson
created: 2026-11-23
updated: 2026-11-23
closed_at: 2026-11-23
priority: p2
stratum: '4'
---

## Problem

## Proposal

## Starting point and non-goals

Name existing implementation/tests; distinguish missing work from work already delivered.

## Delivery steps and dependencies

1. Resolve specification changes first, in their own PR.
2. Implement the smallest slice; name the paths and prerequisite items.
3. Verify success, failure and boundary behavior.

## Which stratum does this reach?

## Acceptance criteria

- [ ]

## Evidence to close

Set `proof` to the observable gate. Record the tested commit, exact checks and
remaining limits here. Human or elapsed-time evidence cannot be replaced by CI.

## 2026-11-23

Closed Pass-3 world, foreign, and CLI findings: verified dlsym symbols are defined in the explicitly loaded shared object via dladdr/dlinfo (blocking transitive libc::system RCE); isolated Disk::writing to stratum 4 and Net::sending to stratum 6 per §9.1/§8.2; rejected NUL bytes in Disk::resolve as Malformed; hardened Net HTTP/1.1 framing (strict HTTP/1.0|1.1, 3-digit status, RFC 9110 tchar header names, digits-only Content-Length/ports, sole chunked Transfer-Encoding, rejecting combined TE+CL, and cumulative chunked framing budget); stopped Replay::merge from leaking base_witnesses into returned Replay.said; added return-type validation, act-hole prefix ordering, and deferred witness persistence to graft; added seen_locals cycle/DAG guard to lamp --at; fixed exit 65 MALFORMED codes in strata/not_a_trace/closing; verified trace completeness in Memo::recall and deferred source writes until memo miss.
