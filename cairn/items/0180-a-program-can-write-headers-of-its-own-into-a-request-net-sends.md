---
id: 180
title: A program can write headers of its own into a request net sends
type: bug
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: crates/nether-world
stratum: '6'
proof: A URL with a carriage return in it is refused rather than sent
---

`crates/nether-world/src/net.rs` builds the request by interpolation:

```rust
format!("{verb} {} HTTP/1.1\r\nHost: {}\r\n…", at.path, at.host)
```

`at.path` is whatever the program put after the host, and
[§3.6](../spec/03-lexical.md) gives a string literal `\r` and `\n`. So a
program can end the request line early and write its own headers:

```c
descend net { get("http://allowed.example/x\r\nHost: internal-admin\r\n") };
```

[§8.3.2](../spec/08-rites.md)'s reach decides **which socket opens**. It does
not decide what goes down it. A reverse proxy in front of the allowed host
routes on `Host:`, so the program reaches a name the invocation never declared
— and a second request can be smuggled whole after a blank line.

## And the headers coming back are unbounded

`status_of` reads the status line and every header with `read_line` and no cap
at all. A server that sends one line and never a newline grows a `String`
until the thirty-second timeout. The body is capped at `MOST`; nothing above
it is.

## What a URL may contain

§9.6 says nothing about the shape of a URL, and it does not need to: this is a
fact about an implementation that speaks HTTP/1.1, in the same way the disk
root is a fact about one that opens files. A control character in a URL is not
a URL, and `malformed` is the refusal §5.1.1 has for it.

## Acceptance criteria

- [x] A URL carrying a control character is refused, before the socket
- [x] The host is a host, and not something with an `@` or a space in it
- [x] Response headers are bounded in bytes and in count
- [x] A test sends the smuggling attempt and watches it be refused

## 2026-09-13

Refused before the socket, on the whole URL, rather than escaped on the way into the request. Escaping would leave the question of what a URL is open in three places; refusing anything that is not printable ASCII closes it in one, and a control character in a URL is not a URL.

## 2026-09-13

The host now has to look like a host: letters, digits, - and . and nothing else. That is what stops example.com@evil.invalid, where the reach sees one party and the socket gets another.

## 2026-09-13

Take spends one budget across every read, so MOST_HEAD bounds the status line and all the headers together. When the budget runs out rather than the server, that is Exhausted and not a short read reported as a whole response.
