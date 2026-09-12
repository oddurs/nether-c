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

**Frozen.** Every value has exactly one encoding, on every platform, forever.
Changing anything in this section changes the domain separator in
[§7.2](#72-cairns), which changes every cairn that has ever existed. Treat it
as immovable and mean it.

A value is a tag byte followed by a payload. There is no length prefix on the
value as a whole and no terminator: every payload's extent is determined by
the tag and by lengths inside it.

| Tag | Type | Payload |
| --- | --- | --- |
| `0x00` | `U0` | empty |
| `0x01` | `Bool` | one byte, `0x00` or `0x01`, and nothing else |
| `0x02` | `I64` | eight bytes, two's complement, big-endian |
| `0x03` | `Bytes` | `u64` length, then exactly that many bytes |
| `0x04` | `Str` | `u64` length, then exactly that many bytes of well-formed UTF-8 |
| `0x05` | `Cairn` | thirty-two bytes |
| `0x06` | `Shade` | one byte origin stratum, then the thirty-two byte cairn of the value |
| `0x07` | `Answer` | one byte, `0x00` given or `0x01` refused, then the value or the refusal |
| `0x08` | `Refusal` | one byte, `0x00`..`0x05`, in the order [§5.1.1](05-types.md#511-answers-and-refusals) lists them |
| `0x10` | `Struct` | `u64` name length, the name in UTF-8, `u64` field count, then the fields |
| `0x11` | `Array` | `u64` element count, then the elements |
| `0x20` | `Node` | see [§7.3](#73-nodes) |

Rules an implementation MUST follow:

1. Integers are fixed-width big-endian. There is no variable-length integer
   encoding anywhere in the format, including lengths.
2. Lengths and counts are `u64` big-endian, even where the value is small.
   Eight bytes to say `0` is the price of there being one encoding.
3. Struct fields are encoded in **declaration order**, not alphabetical order,
   and the struct's type name is encoded with it. Nominal typing means the
   name is part of the value.
4. There is no map type, and therefore no question of key ordering. An
   association is an array of pairs, and its order is the program's.
5. There is no floating-point type. See
   [§3.6](03-lexical.md#36-literals).
6. The encoder does not transform what it is given. It does not normalise
   text, reorder anything, or trim anything. A store that silently alters the
   bytes you handed it is not content-addressed; it is content-addressed to
   something else. See §7.1.1.

### 7.1.1 What a decoder MUST reject

Lenient decoding of a canonical format is how two implementations start
disagreeing, so rejection is specified rather than left to judgement. A
decoder MUST reject:

- a tag byte not in the table above;
- a `Bool` payload other than `0x00` or `0x01`;
- a `Str` payload that is not well-formed UTF-8;
- a `Shade` origin byte greater than `8`;
- an `Answer` discriminant other than `0x00` or `0x01`;
- a `Refusal` code greater than `0x05`;
- a `Struct` whose name is empty, or whose name is not well-formed UTF-8;
- any length or count that exceeds the bytes remaining;
- trailing bytes after a complete value.

A decoder MUST NOT accept an input and then re-encode it differently. For
every byte string `b` that decodes to a value `v`, `encode(v)` MUST equal `b`.
That round trip is the definition of canonical and is the property worth
testing first.

## 7.2 Cairns

```
cairn(v) = blake3( DOMAIN || encode(v) )
DOMAIN   = b"netherc/cairn/v3\x00"
```

A cairn is 32 bytes. Its text form is lowercase hexadecimal. Tools MAY display
a prefix; the first 8 hexadecimal characters are the RECOMMENDED short form,
and any tool that accepts a short form MUST reject an ambiguous one rather
than picking a match.

The domain separator is versioned. **Changing what an existing tag means
changes the domain**, which changes every cairn, which is the correct and
honest consequence: values encoded under different rules are not the same
values.

Adding a tag that was previously unassigned does not. A decoder under `v1`
rejected `0x07` as unknown and still does not accept anything it accepted
before; every byte string that had a meaning keeps exactly the meaning it had.
Nothing is reinterpreted, so nothing is renamed.

> This is the narrower rule, and it is narrower because the broad one made the
> format unable to grow without discarding everything ever written under it.
> [§90.2](90-rationale.md#902-rejected-alternatives) records what was given up
> to narrow it.

The domain is at `v3`. It has moved twice, both times because `Trace` changed
what it holds ([§7.3.1](#731-node-encoding)) — to `v2` when a residue stopped
being nodes, and to `v3` when a trace began naming its witnesses. Both are the
bumping case and not the exempt one: a byte string that decoded to a trace
under one domain decodes to a different trace under the next, and the two are
not the same value however similar they look. Nothing had been buried either
time, so the bumps cost nothing. That is the only reason they were affordable,
and it is why the rule is worth having before it is not.

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
| `Trace` | the residue, the holes, the witnesses, the deposits, the source, the fuel spent, the depth, and the stratum-8 mark |

Nodes reference other nodes only by cairn. The graph is therefore acyclic by
construction: a node cannot name a node that does not yet exist, and a node
that exists cannot change.

### 7.3.1 Node encoding

**Frozen**, on the same terms as [§7.1](#71-canonical-encoding). A node is the
tag `0x20`, then a kind byte, then the kind's payload.

| Kind | Node | Payload |
| --- | --- | --- |
| `0x00` | `Literal` | one `value` |
| `0x01` | `Apply` | `cairn` of the function, `cairn-list` of arguments, `cairn` of the result |
| `0x02` | `Hole` | `call`, one byte stratum, `span` |
| `0x03` | `Deposit` | `cairn` of the value, `span` |
| `0x04` | `Witness` | one byte stratum, `call`, `cairn` of the answer, `span` |
| `0x05` | `Trace` | `cairn` of the residue source, `cairn-list` of holes, `cairn-list` of witnesses, `cairn-list` of deposits, `cairn` of the source buried, `u64` fuel spent, one byte depth, one byte stratum-8 mark |

Three shapes appear inside more than one of them:

| Shape | Encoding |
| --- | --- |
| `cairn-list` | `u64` count, then that many thirty-two byte cairns |
| `span` | `cairn` of the source, `u64` start, `u64` end |
| `call` | `u64` name length, the name in UTF-8, `cairn-list` of arguments |

A decoder MUST reject, in addition to the clauses in
[§7.1.1](#711-what-a-decoder-must-reject):

- a kind byte not in the table above;
- a stratum or depth greater than `8`;
- a stratum-8 mark other than `0x00` or `0x01`;
- a `call` with an empty name;
- a `span` whose end is before its start.

A span names its source by cairn rather than by path. A path is a fact about
one machine at one moment; the trace has to mean the same thing on a machine
that has never seen that filesystem.

### 7.3.2 What a decoder cannot check

A `Trace`'s `depth` is how deep it goes, counting both directions at once: the
join of the depth of its residue and the greatest stratum of any `Witness` it
names.

Both halves are needed and neither alone is enough. A stage-one burial has
answered nothing, so its witnesses say 0 and its residue says how deep the
program still reaches — which is why
[§8.2](08-rites.md#82-bury)'s transcript reads `depth 3` over a program with one
unanswered `read`. A sealed trace has no holes left, so its residue says 0 and
its witnesses say how deep it went — which is why
[§6.6](06-evaluation.md#66-exhumation)'s reads `depth 3   holes 0`. One rule,
both transcripts.

`opaque` is the case that rules out counting holes instead of the residue: a
world-question inside an `opaque` barrier is never evaluated and never becomes a
hole, and the expression is still at its stratum.

The stratum-8 mark is the other half of §1.7 and is **not** the same test.
It is set exactly when a `Witness` the trace names is at stratum 8 — *reached*,
not owed. A trace whose only stratum-8 call is still a hole has depth 8 and is
not marked, because nothing has happened off the record yet.

Neither claim is checkable on `put`. A decoder sees one node, and both are
facts about a graph that may not be wholly present. An implementation MUST NOT
be required to enforce them there.
[§8.6](08-rites.md#86-strata) is where the witness half is caught, because
`strata` is the rite that has the whole graph in hand. The residue half needs
lowering the residue, which is burying it, and that is `bury`'s job rather than
`strata`'s.

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

The store is a set of `(cairn, bytes)` pairs, where the bytes are the canonical
encoding of a **value or a node**. Both have cairns and both are addressed the
same way: a `Deposit` names the value it deposited directly, rather than naming
a `Literal` node that wraps it.

```
put(stored)  -> Cairn      idempotent
get(Cairn)   -> Stored     fails if absent
has(Cairn)   -> Bool
resolve(hex_prefix) -> Cairn | Ambiguous | Absent
```

`put` MUST be atomic: a reader MUST never observe a partially written node. An
implementation SHOULD write to a temporary path and rename.

Because writing is idempotent and content-addressed, concurrent writers need
no coordination beyond atomicity of the individual write. Two burials that
compute the same value write the same bytes to the same place.

### 7.5.1 What survives a crash

A store is **crash-consistent and not durable**. A `put` that returned may not
be on the disk after a power loss, and an implementation MAY `fsync` but is not
required to.

What a store MUST NOT do is serve bytes that are not what the name says. That
is what makes the weaker promise enough. After a crash an object is in one of
two states, and both are safe:

- **absent**, which is indistinguishable from never having been written, and
  which a later `put` repairs by writing it again;
- **present and wrong**, which `get` catches, because the name is the content
  and checking costs one hash of bytes it has already read.

So a crash can lose work and cannot manufacture a fact. That is the property
the reproducibility claims in this document rest on, and it is weaker than
durability on purpose: a build tool that paid for an `fsync` per node would pay
it thousands of times for a single burial, to protect work it can simply do
again.

An implementation MUST NOT skip the check in `get` on the grounds that it
`fsync`s. The check is against bit rot and a bad disk as much as against a
crash, and those do not announce themselves.

The reverse index of [§7.4](#74-provenance) is a set of fixed-size records. An
implementation MUST detect a partial one rather than read past it — a record
half written is a record that would otherwise be read as a different cairn, and
a wrong edge in a provenance walk is worse than a missing one.

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
