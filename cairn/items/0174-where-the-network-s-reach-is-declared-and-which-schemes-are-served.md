---
id: 174
title: Where the network's reach is declared, and which schemes are served
type: spec
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: spec
stratum: '6'
proof: Fetching is a documented invocation, and a scheme this build will not serve says so
---

Two things [§9.6](../spec/09-prelude.md) does not say, and 0070 cannot be
built without deciding both.

## How far `net` reaches

`Disk` takes a root nothing may reach out of, and says why in its own words: a
burial that can read `/etc/shadow` because a program asked it to is a burial
nobody can grant a capability to. The same argument applies to a socket and
nothing in §09 makes it.

`--grant net` should not be a blank cheque. §8.3 needs a `--reach`, the same
shape as §8.3.1's declaration, and a host nobody named is `denied`.

## Which schemes an implementation serves

`https` is TLS, and `CLAUDE.md` has exactly one dependency exception:

> Write your own encoders, parsers, renderers and formats. Do not write your
> own cryptography.

So an implementation either takes a TLS dependency or serves `http` alone.
§9.6 should say that the choice is the implementation's, that it MUST say
which it serves, and that a scheme it does not serve is `denied` rather than
`unreachable` — the difference between "this build will not" and "the world
did not answer".

## Acceptance criteria

- [x] §8.3.2 says how a reach is declared
- [x] §9.6 says an implementation states which schemes it serves
- [x] The rejected alternatives are in `spec/90-rationale.md`

## 2026-09-13

A bare host matches any port, because the port is a detail of the service and the host is the party being trusted; and it matches that host exactly, because a subdomain is a different party and a reach that spread to one would be a reach nobody declared.

## 2026-09-13

§9.6 does not fix the scheme set. What it fixes is that an implementation says which it serves, and that one it will not serve comes back denied rather than unreachable -- otherwise a trace records 'the host was down' for a build that never dialled.
