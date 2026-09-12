---
id: 117
title: 'Spec: Answer and Refusal have no tag in the frozen encoding'
type: spec
status: unmarked
milestone: rites
depends_on:
- 21
created: 2026-09-12
updated: 2026-09-12
priority: p0
effort: s
area: spec/07-ledger.md
proof: Every type in section 5.1 has a tag in the table in section 7.1, or a stated reason it is not a ledger value
---

## The hole

`spec/05-types.md` §5.1 gives both types a canonical encoding:

| `Answer⟨T⟩` | tag byte, then the `T` or the `Refusal` |
| `Refusal` | one byte |

`spec/07-ledger.md` §7.1 is the frozen tag table, and it has neither. The
free tags below `0x10` are `0x07` upward.

§7.1 wins — it says so itself — so as the document stands an `Answer` cannot
be sealed, deposited, or recorded as a witness. Every prelude function that
touches the world returns one.

Found while building the IR's type model: §5.1 and §7.1 disagree about how
many types there are.

## Why this is awkward

Adding a tag changes nothing that exists — no trace has ever held an `Answer`,
because nothing has ever been buried — but §7.1 says changing the section
changes the domain separator in §7.2, and therefore every cairn. Either that
sentence means "changing the meaning of an existing tag" and should say so, or
the domain goes to `v2` before anything is written under `v1`.

## Acceptance criteria

- [x] `Answer` and `Refusal` have tags, or §5.1 stops claiming they have encodings
- [x] §7.1.1 says what a decoder rejects for each
- [x] The domain separator question is answered in §7.2, in one sentence
- [x] `nether-ledger` implements whichever it is

## 2026-09-12

Settled. Answer is 0x07, Refusal is 0x08, both in the free range below 0x10. 7.1.1 rejects an Answer discriminant other than given/refused and a refusal code above 5. nether-ledger implements both, with the refusal set's discriminants asserted equal to the order 5.1.1 lists them — that order is the format, so a test holds it.

The domain separator question is answered, and it needed a narrower rule than 7.2 had. The old sentence said any change to the encoding bumps the domain, which would have renamed every value ever stored in order to assign a tag that had never meant anything. It now distinguishes changing what an existing tag MEANS (bumps: a byte string that decoded to one value now decodes to another, and calling both by one name is a lie) from assigning a previously-unknown tag (does not: nothing that decoded changes, and what was refused is merely refused less often). v1 stands. Recorded in 90.2 along with what the narrower rule costs — it can no longer be checked by looking at a diff.

Caught by the compiler on the way through: nether-cli's lamp matched Value exhaustively and had to learn to render both. A refusal reads as its word rather than its code, and an answer says which it is, so that 'absent' and a string that happens to be 'absent' do not read the same.
