---
id: 171
title: exists answers where the prelude gives it a plain Bool
type: bug
status: buried
milestone: world
assignee: Oddur Sigurdsson
created: 2026-09-13
updated: 2026-09-13
priority: p0
effort: s
area: crates/nether-world
stratum: '3'
proof: A branch on exists folds and the deposit inside it happens
---

## Measured

```console
$ cat p.nc
Bool@3 e = descend disk { exists("m.nc") };
U0 note()
{
  if (e) { b"it is there"; } else { b"it is not"; }
}
demand note();

$ nether exhume <trace> --grant disk
sealed   1c4f2f90 + 5f2a22ab → b262ae25   depth 3   holes 0

$ nether lamp <sealed>
nothing was deposited
```

The file is there. The answer is recorded, correctly, as `given true` — and
that is the bug. [§9.5](../spec/09-prelude.md) types `exists` as

```
Bool exists(Str path)                  @3;
```

not `Answer<Bool>`, because "is it there" has an answer either way and there is
no refusal to distinguish. `crates/nether-world/src/disk.rs` wraps it in
`Given` anyway, so the value substituted back is an `Answer` where the program
declared a `Bool`, the branch cannot fold, and the deposit inside it never
happens. Nothing says a word: the trace seals, the depth is right, and the
program silently did nothing.

The existing test asserts the wrong shape and says so in its own comment —
"§9.5 gives `exists` a `Bool` and no refusals" — directly above
`AnswerOf::Given(Value::Bool(true))`.

## Every prelude function this applies to

`Bool has_node`, `I64 clock`, `Str target` and `Bytes draw` are the rest of
them, and none is built yet. Whatever this item settles is the rule the
providers in 0068, 0070 and 0071 are written to.

## Acceptance criteria

- [x] A prelude function whose §09 signature is not an `Answer` records a plain value
- [x] A branch on `exists` folds, and a deposit inside it happens
- [x] The rule is stated where the next provider will read it

## 2026-09-13

Two halves, and the first alone fixed nothing. nether-world had to stop wrapping, and nether-bury had to learn to fold what is not wrapped: Val::answered returned stuck for anything that was not a Value::Answer, so a plain Bool came back a hole and the branch stayed residual either way.

## 2026-09-13

The rule lives on Prim::refusable, in nether-core, so nether-world and nether-bury read the same sentence rather than each deciding. The five that cannot be refused are has_node, clock, target, exists and draw.
