---
id: 149
title: Burial does not deposit
type: bug
status: buried
milestone: rites
created: 2026-09-12
updated: 2026-09-12
priority: p1
effort: m
area: crates/nether-bury
stratum: '0'
proof: A program whose only statement is a bare expression buries to a trace with one deposit in it, and lamp shows it
---

## What happens

`hello.nc` is the first program in the specification and its whole body is a
bare string. §4.7 says an expression statement whose value is not `U0`
**deposits** that value into the trace, and §6.8 says a program's deposits are
what `nether lamp` shows afterwards.

`nether-bury` produces no `Node::Deposit`. Burying `hello.nc` writes a trace
whose `deposits` list is empty, so `nether lamp` on it shows nothing, so the
two-command hello world on the front page does not work.

## What should happen

Burial produces one `Deposit` per bare expression statement it evaluates, in
source order, and `Node::Trace` names them.

## How it was found

Building `nether bury` (0060). `Node::Trace` gained a `deposits` field in 0128
and there was nothing to put in it. Shipped as an empty list rather than
pretending, because an empty list is true today.

## Note

This is the last thing between the implementation and the page's own first
example, which makes it the best available end-to-end proof.

## 2026-09-12

Fixed. A bare expression statement whose value is not U0 now produces a Node::Deposit, named and in source order, and Node::Trace carries them.

Six tests: one statement one deposit, U0 deposits nothing, the same value twice is two deposits (they differ by span, because the program did say it twice), source order, the span covers exactly what was written, and an undemanded function deposits nothing — which is 6.2's guarantee that work nothing needs is never done.

It found a bigger bug immediately. The specification's own first program is 'demand greet;', which demands the FUNCTION: 4.5 requires parentheses on a call, so the body never runs and nothing is deposited. 'demand greet();' prints 'Hello from the nether'. Filed as 0151, p0 — it is the first program anybody reads and it is on the front page.
