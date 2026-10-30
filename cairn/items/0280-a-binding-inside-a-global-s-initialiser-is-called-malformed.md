---
id: 280
title: A binding inside a global's initialiser is called malformed
type: bug
status: buried
assignee: Oddur Sigurdsson
claimed: 2026-10-30
created: 2026-10-30
updated: 2026-10-30
priority: p2
effort: m
stratum: 0
area: crates/nether-core
proof: Bytes g = { Bytes s = b"hi"; concat(s, s) }; buries, or is refused with a fault §04 or §05 names, and print of it names s
---

## What happens

§4.2 lets a global's initialiser be any expression, and a block or a `descend`
body may bind locals. Lowering accepts it. The checker then refuses it as
malformed IR:

```console
$ nether bury a.nc
error: this binding names no local
 --> a.nc:1:23
```

`GlobalDef` has no table of locals the way `FuncDef` does, so the checker cannot
find one, `print` writes the binding as `U0 «local 0»`, and the message is an
internal one: it describes the IR, not anything the programmer wrote.

## What should happen

Either the binding works — a global's initialiser is an expression, and an
expression may hold a block — or the specification says it may not and the
refusal says so in the programmer's terms.

## Reproduction

1. `Bytes g = { Bytes s = b"hi"; concat(s, s) };`, `U0 f() { g; }`, `demand f();`
2. `nether bury` exits 65 with `this binding names no local`.

The fuzzer found it (0218, 0279): a mutation of `owed.nc` that put a binding
inside its `descend` body lowered, and printed as source that does not lex.

## Cairn of the offending trace

None: it never becomes a trace, which is why no residue can hold one and why
0279's round-trip property is stated over units that check.

## Fix boundary and regression proof

The likely fix is a `locals` table on `GlobalDef`, filled by lowering the way a
function's is and read by the checker and `print`. Refusing it instead needs a
sentence in §4.2 first.

## 2026-10-30

Buried by making it work. Unit-level code has a frame now, as a function does: Unit::locals. Lowering keeps one frame for the globals and the demands, setting it aside while each function has its own; the checker gives unit-level code that frame before the globals and again before the demands, and looks a binding's assertion up in it; burial evaluates a global and a demand in an environment of its size; print names unit-level locals from it. One more thing it uncovered: a starved global's value used to be copied into whatever function read it, and a copy carrying unit-level locals into a function's frame names something else there. In a unit that binds at unit level, a starved global now residualises as its name; every other unit's residue is unchanged, which is why no transcript moved. Bytes g = { Bytes s = b"hi"; concat(s, s) }; buries and lamps hihi, and a starved global binding a local exhumes. Tests: a_binding_inside_a_global_buries, a_starved_global_that_binds_exhumes, and the fuzz corpus carries a sample. Core +24, to 8569.
