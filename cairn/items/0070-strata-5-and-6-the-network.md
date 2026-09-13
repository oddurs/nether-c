---
id: 70
title: 'Strata 5 and 6: the network'
type: feature
status: buried
milestone: world
assignee: Oddur Sigurdsson
depends_on:
- 67
- 174
created: 2026-09-10
updated: 2026-09-13
priority: p1
effort: l
area: crates/nether-world
stratum: '6'
proof: A program that fetched over the network replays on a machine with no network
---

The stage 4 proof lives here. Every response sealed on arrival with its
witness; replay serves from the record and refuses to open a socket at all.

## 2026-09-13

http only. §9.6 does not fix the scheme set but requires an implementation to say which it serves, and https is TLS -- the one thing this repository does not write for itself. A scheme this build will not serve is denied, not unreachable: one is this build saying no and the other is the world not answering.

## 2026-09-13

The reach is checked before the socket, not after. A reach verified by whether the connection failed is not a reach; the test proves it by leaving a server up on a port nobody declared and asserting it is never asked.

## 2026-09-13

No redirects. A redirect followed silently is a trace whose witness records a URL the program never asked for; Location is in the response for a program that wants to ask again.

## 2026-09-13

Headers are not recorded. §1.1's witness obligation is what the program was told, the program is told the body, and a Date header would make two identical fetches two different witnesses.

## 2026-09-13

The provider writes the request in one write_all rather than two. Two writes is two packets, and a reader that took the first for the whole request would be right about the bytes and wrong about the request -- which is exactly how the post test failed once before the server learned to read to the end.
