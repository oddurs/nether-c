---
section: "07"
title: The ledger
status: draft
---

# The ledger

The format is the contract. Everything else in this specification is
negotiable; an ambiguity here is a reproducibility bug that will be found
years from now by somebody who cannot fix it.

## 7.1 Canonical encoding

Every value has exactly one encoding, on every platform, forever.

```
value      := tag byte_content

tag        := 0x00  U0
            | 0x01  Bool
            | 0x02  I64
            | 0x03  Bytes
            | 0x04  Str
            | 0x05  Cairn
            | 0x06  Shade
            | 0x10  Struct
            | 0x11  Array
            | 0x20  Node
```

Rules an implementation MUST follow:

1. Integers are fixed-width big-endian. There is no variable-length integer
   encoding anywhere in the format, including lengths.
2. Lengths are `u64` big-endian, even where the value is small.
3. Struct fields are encoded in **declaration order**, not alphabetical order,
   and the struct's type name is encoded with it. Nominal typing means the
   name is part of the value.
4. There is no map type, and therefore no question of key ordering. An
   association is an array of pairs, and its order is the program's.
5. There is no floating-point type. See
   [§3.6](03-lexical.md#36-literals).
6. A decoder MUST reject any input that is not the canonical encoding of the
   value it decodes to. Lenient decoding of a canonical format is how two
   implementations start disagreeing.

## 7.2 Cairns

```
cairn(v) = blake3( DOMAIN || encode(v) )
DOMAIN   = b"netherc/cairn/v1\x00"
```

A cairn is 32 bytes. Its text form is lowercase hexadecimal. Tools MAY display
a prefix; the first 8 hexadecimal characters are the RECOMMENDED short form,
and any tool that accepts a short form MUST reject an ambiguous one rather
than picking a match.

The domain separator is versioned. Changing the encoding changes the domain,
which changes every cairn, which is the correct and honest consequence: values
encoded under different rules are not the same values.

## 7.3 Nodes

A **node** is the unit the ledger stores. Every node is content-addressed by
its cairn.

| Node | Contents |
| --- | --- |
| `Literal` | a value |
| `Apply` | a function cairn, argument cairns, and the resulting value cairn |
| `Hole` | the fields listed in [§6.3](06-evaluation.md#63-holes) |
| `Deposit` | a value cairn and the source span that deposited it |
| `Witness` | a stratum, a call, the answer, and the span that asked |
| `Trace` | the roots, the fuel spent, the depth reached, and the stratum-8 mark |

Nodes reference other nodes only by cairn. The graph is therefore acyclic by
construction: a node cannot name a node that does not yet exist, and a node
that exists cannot change.

## 7.4 Provenance

Provenance is not a separate index. It is the `Apply` and `Witness` nodes
themselves, read backwards: given a value's cairn, the ledger can find every
node that produced it, and from those, their inputs, down to literals and
holes.

An implementation MUST maintain a reverse index sufficient to answer *what
produced this cairn* in time proportional to the answer, not to the size of
the store. `nether lamp --provenance` is unusable otherwise, and it is the
rite that makes the language comprehensible.

## 7.5 The store

The store is a set of `(cairn, node)` pairs. Required operations:

```
put(node)    -> Cairn      idempotent
get(Cairn)   -> Node       fails if absent
has(Cairn)   -> Bool
resolve(hex_prefix) -> Cairn | Ambiguous | Absent
```

`put` MUST be atomic: a reader MUST never observe a partially written node. An
implementation SHOULD write to a temporary path and rename.

Because writing is idempotent and content-addressed, concurrent writers need
no coordination beyond atomicity of the individual write. Two burials that
compute the same value write the same bytes to the same place.

## 7.6 Garbage

There is none.

A content-addressed store of immutable values has no unreachable objects, only
objects nothing currently points at — and the entire premise of the language
is that something might point at them later. Deleting a node destroys a
provenance chain that another trace may depend on, and there is no way to know
that it does not without reading every trace that will ever exist.

An implementation MAY provide an explicit, operator-invoked prune with a
stated reachability root. It MUST NOT prune automatically, and it MUST NOT
prune `Witness` nodes at all: a witness is the only copy of something the
world said once and may never say again.

> This is a real cost, stated plainly: a Nether C store grows monotonically.
> Whether that is affordable is a question the roadmap item *The store:
> layout, index, and garbage* has to answer with measurements rather than
> opinions.

## 7.7 Versioning

The format carries no version field inside a node. The version is the domain
separator in §7.2.

An implementation encountering a cairn it cannot verify under its own domain
MUST report that the node was written under a different format version and
refuse it. It MUST NOT attempt to interpret it. There is no migration path
between format versions and there is not meant to be one: a value encoded
under different rules is a different value, and pretending otherwise would
make every reproducibility claim in this document conditional.
