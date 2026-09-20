---
section: "02"
title: A name that holds
status: draft
---

# A name that holds

> One idea: **the name is the thing.**

`efed9b2a` was not allocated. Nothing handed it out and nothing is keeping a
counter. It is what the trace *is*, run through a hash — a **cairn**.

You can ask for one without burying anything:

```console
$ nether cairn hello.nc
83219e1bdbee3e617fa774b52c1cc0fccf0dd1ab87087e0e6641a20414c4b4f8   143 bytes   hello.nc
```

That is the name of those 143 bytes, and it is the name of those 143 bytes on
every machine that has ever existed and every machine that ever will. Change
one character of the file and it is a different name. There is no version of
`hello.nc` that has that name and different contents.

The short form is a prefix. `efed9b2a` is the first four bytes of the trace's
cairn, and every rite takes a prefix as long as it is unambiguous in your
ledger. Use the short one at a terminal and the long one anywhere it has to
survive.

## Asking whether a name still holds

```console
$ nether cairn --verify efed9b2a
efed9b2aa645c8105480b982c17c1cfca96c232987c1c7d89e60186ed3292cd8   holds   node, 124 bytes
```

*Holds* means the ledger has something under that name and that thing hashes
to that name. It is not a lookup. The ledger is not trusted to have stored the
right bytes; it is checked.

This is a small command and it is the load-bearing one. Everything else in the
language leans on being able to say "this is the thing I meant" and be
believed without anybody having to be trusted.

## Why the name is not a path

A path says where something is. A cairn says what it is. The difference shows
up the moment two people, or one person and last Tuesday, disagree:

- A path can point at different bytes on Monday and Friday. A cairn cannot.
- A path is only meaningful on the machine that has it. A cairn is meaningful
  anywhere.
- Two paths can name the same bytes and nothing notices. Two cairns that name
  the same bytes *are* the same cairn, and the ledger stores it once.

That last one is not an optimisation. It is why a build can tell you what it
reused: if two things have the same name, nothing had to make the second one.

## A note on the word

A cairn is a pile of stones that marks something. You put one where you have
been, so that somebody coming after you knows the way. The word is doing that
job here and the vocabulary is not decoration — [§10](../../spec/10-glossary.md)
lists the rest of it.

[Next](03-the-hole.md): the first program that cannot finish.
