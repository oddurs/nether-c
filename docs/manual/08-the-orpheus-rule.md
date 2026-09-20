---
section: "08"
title: The Orpheus rule
status: draft
---

# The Orpheus rule

> One idea: **you may carry a value up; you must go back down to look at it.**

Nine strata, and depth composes upwards: a thing built from parts is exactly as
deep as its deepest part. That is
[§1.1](../../spec/01-strata.md), and on its own it would make deep values
useless — one network call and everything downstream of it is at stratum 5
forever, including the integer you got by counting its bytes.

So there is a way out, and there is a price. A **shade** is a deep value
carried up to the surface. Sealing one, naming it, passing it around, storing
it: all fine, all at depth 0. Looking at what is inside it is not.

Write `stamp.nc`:

```c
// stamp.nc — the Orpheus rule, and the error spec/01-strata.md §1.6 prints.
//
// A shade may be carried up out of any stratum. It may only be looked at by
// going back down, and this does not: the look is at the top level of a file,
// where the ambient depth is 0 and the value came from stratum 5.
//
// The five lines below are §1.6's sample, unchanged, so that the file the
// error names is a file that exists and the line and column it prints are
// checked against it.
Shade<Bytes> reply = descend net { shade must(get("https://example.invalid/index.json")) };

Cairn witness = seal reply;       // legal: Cairn@0
// ILLEGAL here — see below
I64   n       = len(look(reply));
```

```console
$ nether bury stamp.nc
error: cannot look at a shade from stratum 5 at depth 0
  --> stamp.nc:14:21
   |
14 | I64   n       = len(look(reply));
   |                     ^^^^^^^^^^^ this shade came from `get` at stratum 5
   |
   = the value is here, but you are not. Wrap the look in `descend net { … }`.
```

*The value is here, but you are not.*

`seal reply` on the line above is fine. You may name the thing you brought back
without going down for it, and the name is at depth 0 like any other name. It
is the `look` that is refused, and it is refused where it stands rather than
where the value came from.

## Why not just let it through

Because the alternative is a language where a `Bytes` might have come from the
network and might not, and nothing in the type says which. Then every function
that takes `Bytes` is a function that might be handling something a stranger
sent, and knowing which ones is a code review rather than a compile.

Here, looking inside a shade is a syntactic act at a known depth. Every place
in a program where deep data becomes ordinary data is a `look` inside a
`descend`, and you can find all of them with a grep.

## The name

Orpheus was allowed to bring her up. He was not allowed to turn around and
check. He did, at the threshold, and that was that.

The rule is the same shape and it is meant as a joke you can also use: carrying
is free, looking has a place where it is legal, and the place is further down
than where you want to be standing.

## Where it bites in practice

It will bite on a length. You have a response, you want its size, and `len` is
pure — so you reach for it at the top level and get this error. The fix is
always the same and always one line: do the looking inside a `descend` and
carry the *result* up.

```c
I64@5 n = descend net { len(look(reply)) };
```

Which is a stratum-5 integer, and honest, because the only reason you know that
number is that somebody answered a network call.

[Next](09-where-to-go-from-here.md): the map.
