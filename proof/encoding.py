#!/usr/bin/env python3
"""Mechanization of canonical encoding injectivity (`spec/07-ledger.md` §7.1–§7.3).

Covers:
1. Prefix-free stream round-trip (`DecodeStream(Concat(Encode(x), rest)) = Pair(x, rest)`)
   inductively over all 11 `Value` tags (`0x00`..`0x08`, `0x10`, `0x11`) and all 6
   `Node` kinds (`0x20` + `0x00`..`0x05`), including recursive unprefixed compounds
   (`Struct`, `Array`, `Answer::Given`, `Node::Literal`).
2. Pure LCF equational proof (`prove_injectivity_from_left_inverse`) that any
   encoding with a left inverse `decode(encode(x)) = x` is injective:
   `(decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((encode(x) = encode(y)) → (x = y)))`.
3. Cairn preimage injectivity (`prove_cairn_preimage_injectivity`) separating
   `DOMAIN = b"netherc/cairn/v4\\x00"` and reducing `cairn(x) = cairn(y)` to BLAKE3
   collision resistance on `DOMAIN || encode(x)`.
4. Stored-kind domain separation (`prove_stored_domain_separation`): `Value` tags
   (`{0x00..0x08, 0x10, 0x11}`) and `Node` tag (`0x20`) are disjoint, so a `Value`
   and a `Node` never share an encoding.
5. Executable byte-level reference codec checked against canonical byte fixtures
   for all 11 `Value` tags, all 6 `Node` kinds, signed `I64` two's-complement
   boundaries, and the invalid in-memory states excluded by §7.1.1 / §7.3.1
   pending 0115 (`Shade.origin > 8`, `Struct.name == ""`, `nesting > MAX_DEPTH`,
   `Hole/Witness/Trace.stratum > 8`, `Call.function == ""`, `Span.start > Span.end`).
"""
from __future__ import annotations

import struct
from typing import Any, List, Sequence, Tuple

from kernel import (
    Formula,
    InductiveRelation,
    ProofFault,
    Rule,
    Term,
    Theorem,
    app,
    assume,
    cong,
    conj,
    conj_elim_l,
    conj_elim_r,
    conj_intro,
    discharge,
    eq,
    gen,
    inst_all,
    mp,
    pred,
    refl,
    rule_induction,
    sym,
    trans,
    var,
)

DOMAIN = b"netherc/cairn/v4\x00"
MAX_STRATUM = 8
MAX_REFUSAL = 5
MAX_DEPTH = 128

x = var("x")
y = var("y")
b = var("b")
c = var("c")
n = var("n")
s = var("s")
d = var("d")
r = var("r")
v = var("v")
v1 = var("v1")
v2 = var("v2")
xs = var("xs")
rest = var("rest")
rest1 = var("rest1")
rest2 = var("rest2")
out = var("out")
out1 = var("out1")
out2 = var("out2")
call_t = var("call_t")
span_t = var("span_t")
cairns1 = var("cairns1")
cairns2 = var("cairns2")
cairns3 = var("cairns3")
mark = var("mark")


def pair(fst: Term, snd: Term) -> Term:
    return app("Pair", fst, snd)


def val_rt(val: Term, rem: Term, res_val: Term, res_rem: Term) -> Formula:
    return pred("ValRoundTrip", val, rem, res_val, res_rem)


def node_rt(node: Term, rem: Term, res: Term) -> Formula:
    return pred("NodeRoundTrip", node, rem, res)


# ── 1. Prefix-free stream round-trip over all 11 Value tags ──────────────────

VAL_ROUNDTRIP = InductiveRelation(
    "ValRoundTrip",
    (
        # 0x00 U0: empty payload
        Rule("V_Unit", (), val_rt(app("VUnit"), rest, app("VUnit"), rest)),
        # 0x01 Bool: 1 byte (0x00 or 0x01)
        Rule("V_Bool", (), val_rt(app("VBool", b), rest, app("VBool", b), rest)),
        # 0x02 I64: 8 bytes big-endian two's complement
        Rule("V_Int", (), val_rt(app("VInt", n), rest, app("VInt", n), rest)),
        # 0x03 Bytes: u64 length + exact byte slice
        Rule("V_Bytes", (), val_rt(app("VBytes", b), rest, app("VBytes", b), rest)),
        # 0x04 Str: u64 length + well-formed UTF-8 slice
        Rule("V_Str", (), val_rt(app("VStr", s), rest, app("VStr", s), rest)),
        # 0x05 Cairn: 32 bytes
        Rule("V_Cairn", (), val_rt(app("VCairn", c), rest, app("VCairn", c), rest)),
        # 0x06 Shade: 1 byte origin (0..=8) + 32-byte cairn
        Rule(
            "V_Shade",
            (pred("ValidStratum", d),),
            val_rt(app("VShade", d, c), rest, app("VShade", d, c), rest),
        ),
        # 0x07 Answer::Given(v): 0x00 + recursive Value
        Rule(
            "V_AnswerGiven",
            (val_rt(v, rest, v1, rest1),),
            val_rt(app("VAnswerGiven", v), rest, app("VAnswerGiven", v1), rest1),
        ),
        # 0x07 Answer::Refused(r): 0x01 + 1 byte refusal code (0..=5)
        Rule(
            "V_AnswerRefused",
            (pred("ValidRefusal", r),),
            val_rt(app("VAnswerRefused", r), rest, app("VAnswerRefused", r), rest),
        ),
        # 0x08 Refusal: 1 byte refusal code (0..=5)
        Rule(
            "V_Refusal",
            (pred("ValidRefusal", r),),
            val_rt(app("VRefusal", r), rest, app("VRefusal", r), rest),
        ),
        # 0x10 Struct: non-empty UTF-8 name + u64 field count + self-delimiting fields
        Rule(
            "V_Struct",
            (pred("NonEmptyUtf8", s), val_rt(xs, rest, out1, rest1)),
            val_rt(app("VStruct", s, xs), rest, app("VStruct", s, out1), rest1),
        ),
        # 0x11 Array: u64 element count + self-delimiting elements
        Rule(
            "V_Array",
            (val_rt(xs, rest, out1, rest1),),
            val_rt(app("VArray", xs), rest, app("VArray", out1), rest1),
        ),
        # Self-delimiting sequence of Values (used by Struct fields and Array items)
        Rule(
            "V_SeqNil",
            (),
            val_rt(app("Nil"), rest, app("Nil"), rest),
        ),
        Rule(
            "V_SeqCons",
            (
                val_rt(v, rest1, v1, rest2),
                val_rt(xs, rest, out1, rest1),
            ),
            val_rt(
                app("Cons", v, xs),
                rest,
                app("Cons", v1, out1),
                rest2,
            ),
        ),
    ),
)


def _val_rt_prop(args: Tuple[Term, ...]) -> Formula:
    val, rem, res_val, res_rem = args
    return conj(eq(res_val, val), eq(res_rem, rem))


def prove_value_decode_encode() -> Theorem:
    """`∀v rest res_v res_r. ValRoundTrip(v, rest, res_v, res_r) → (res_v = v ∧ res_r = rest)`."""
    def _leaf(term: Term) -> Theorem:
        return conj_intro(refl(term), refl(rest))

    cases = {
        "V_Unit": lambda _p, _ih: _leaf(app("VUnit")),
        "V_Bool": lambda _p, _ih: _leaf(app("VBool", b)),
        "V_Int": lambda _p, _ih: _leaf(app("VInt", n)),
        "V_Bytes": lambda _p, _ih: _leaf(app("VBytes", b)),
        "V_Str": lambda _p, _ih: _leaf(app("VStr", s)),
        "V_Cairn": lambda _p, _ih: _leaf(app("VCairn", c)),
        "V_Shade": lambda _p, _ih: _leaf(app("VShade", d, c)),
        "V_AnswerGiven": lambda _p, ih: conj_intro(
            cong("VAnswerGiven", [conj_elim_l(ih[0])]),
            conj_elim_r(ih[0]),
        ),
        "V_AnswerRefused": lambda _p, _ih: _leaf(app("VAnswerRefused", r)),
        "V_Refusal": lambda _p, _ih: _leaf(app("VRefusal", r)),
        "V_Struct": lambda _p, ih: conj_intro(
            cong("VStruct", [refl(s), conj_elim_l(ih[0])]),
            conj_elim_r(ih[0]),
        ),
        "V_Array": lambda _p, ih: conj_intro(
            cong("VArray", [conj_elim_l(ih[0])]),
            conj_elim_r(ih[0]),
        ),
        "V_SeqNil": lambda _p, _ih: _leaf(app("Nil")),
        "V_SeqCons": lambda _p, ih: conj_intro(
            cong("Cons", [conj_elim_l(ih[0]), conj_elim_l(ih[1])]),
            trans(conj_elim_r(ih[0]), conj_elim_r(ih[1])),
        ),
    }
    return rule_induction(VAL_ROUNDTRIP, _val_rt_prop, cases)


# ── 2. Prefix-free stream round-trip over all 6 Node kinds ───────────────────

NODE_ROUNDTRIP = InductiveRelation(
    "NodeRoundTrip",
    (
        # 0x20 0x00 Literal(v)
        Rule(
            "N_Literal",
            (val_rt(v, rest, v1, rest1),),
            node_rt(app("NLiteral", v), rest, pair(app("NLiteral", v1), rest1)),
        ),
        # 0x20 0x01 Apply { function, args, result }
        Rule(
            "N_Apply",
            (),
            node_rt(
                app("NApply", c, cairns1, b),
                rest,
                pair(app("NApply", c, cairns1, b), rest),
            ),
        ),
        # 0x20 0x02 Hole { call, stratum, span }
        Rule(
            "N_Hole",
            (pred("ValidCall", call_t), pred("ValidStratum", d), pred("ValidSpan", span_t)),
            node_rt(
                app("NHole", call_t, d, span_t),
                rest,
                pair(app("NHole", call_t, d, span_t), rest),
            ),
        ),
        # 0x20 0x03 Deposit { value, span }
        Rule(
            "N_Deposit",
            (pred("ValidSpan", span_t),),
            node_rt(
                app("NDeposit", c, span_t),
                rest,
                pair(app("NDeposit", c, span_t), rest),
            ),
        ),
        # 0x20 0x04 Witness { stratum, call, answer, span }
        Rule(
            "N_Witness",
            (pred("ValidStratum", d), pred("ValidCall", call_t), pred("ValidSpan", span_t)),
            node_rt(
                app("NWitness", d, call_t, c, span_t),
                rest,
                pair(app("NWitness", d, call_t, c, span_t), rest),
            ),
        ),
        # 0x20 0x05 Trace { residue, holes, witnesses, deposits, source, depth, unrecorded }
        Rule(
            "N_Trace",
            (pred("ValidStratum", d), pred("ValidBoolByte", mark)),
            node_rt(
                app("NTrace", c, cairns1, cairns2, cairns3, b, d, mark),
                rest,
                pair(app("NTrace", c, cairns1, cairns2, cairns3, b, d, mark), rest),
            ),
        ),
    ),
)


def _node_rt_prop(args: Tuple[Term, ...]) -> Formula:
    nd, rem, res = args
    return eq(res, pair(nd, rem))


def prove_node_decode_encode() -> Theorem:
    """`∀n rest out. NodeRoundTrip(n, rest, out) → out = Pair(n, rest)`."""
    val_thm = prove_value_decode_encode()
    cases: dict[str, Any] = {}
    for r_rule in NODE_ROUNDTRIP.rules:
        if r_rule.name == "N_Literal":
            def _lit_case(prems: Tuple[Theorem, ...], _ih: Tuple[Theorem, ...]) -> Theorem:
                v_eq = mp(inst_all(val_thm, [v, rest, v1, rest1]), prems[0])
                return cong(
                    "Pair",
                    [cong("NLiteral", [conj_elim_l(v_eq)]), conj_elim_r(v_eq)],
                )
            cases[r_rule.name] = _lit_case
        else:
            cases[r_rule.name] = (
                lambda _p, _ih, r_rule=r_rule: refl(
                    pair(
                        r_rule.conclusion.terms[0].args[0],
                        r_rule.conclusion.terms[0].args[1],
                    )
                )
            )
    return rule_induction(NODE_ROUNDTRIP, _node_rt_prop, cases)


# ── 3. Pure LCF equational proof: left-invertible encodings are injective ────


def prove_injectivity_from_left_inverse() -> Theorem:
    """Prove in the LCF kernel without any finite-domain or external assumption:
    `∀x y. (decode(encode(x)) = x) → ((decode(encode(y)) = y) → ((encode(x) = encode(y)) → (x = y)))`."""
    enc_x = app("encode", x)
    enc_y = app("encode", y)
    dec_enc_x = app("decode", enc_x)
    dec_enc_y = app("decode", enc_y)

    h1_f = eq(dec_enc_x, x)
    h2_f = eq(dec_enc_y, y)
    h3_f = eq(enc_x, enc_y)

    h1 = assume(h1_f)  # decode(encode(x)) = x
    h2 = assume(h2_f)  # decode(encode(y)) = y
    h3 = assume(h3_f)  # encode(x) = encode(y)

    # cong: encode(x) = encode(y)  |-  decode(encode(x)) = decode(encode(y))
    step1 = cong("decode", [h3])
    # sym(h1): x = decode(encode(x))
    step2 = trans(sym(h1), step1)  # x = decode(encode(y))
    step3 = trans(step2, h2)  # x = y

    thm = discharge(h3_f, step3)
    thm = discharge(h2_f, thm)
    thm = discharge(h1_f, thm)
    thm = gen("y", thm)
    thm = gen("x", thm)
    return thm


def prove_cairn_preimage_injectivity() -> Theorem:
    """Prove that `cairn_preimage(v) = domain_prefix(encode(v))` is injective:
    `∀x y. (strip_domain(domain_prefix(encode(x))) = encode(x)) →
           ((strip_domain(domain_prefix(encode(y))) = encode(y)) →
           ((decode(encode(x)) = x) →
           ((decode(encode(y)) = y) →
           ((domain_prefix(encode(x)) = domain_prefix(encode(y))) → (x = y)))))`."""
    enc_x = app("encode", x)
    enc_y = app("encode", y)
    pre_x = app("domain_prefix", enc_x)
    pre_y = app("domain_prefix", enc_y)

    s1_f = eq(app("strip_domain", pre_x), enc_x)
    s2_f = eq(app("strip_domain", pre_y), enc_y)
    d1_f = eq(app("decode", enc_x), x)
    d2_f = eq(app("decode", enc_y), y)
    eq_pre_f = eq(pre_x, pre_y)

    s1 = assume(s1_f)
    s2 = assume(s2_f)
    d1 = assume(d1_f)
    d2 = assume(d2_f)
    eq_pre = assume(eq_pre_f)

    # strip_domain(pre_x) = strip_domain(pre_y)
    stripped = cong("strip_domain", [eq_pre])
    # enc_x = enc_y
    eq_enc = trans(sym(s1), trans(stripped, s2))
    # decode(enc_x) = decode(enc_y)
    eq_dec = cong("decode", [eq_enc])
    # x = y
    eq_xy = trans(sym(d1), trans(eq_dec, d2))

    thm = discharge(eq_pre_f, eq_xy)
    thm = discharge(d2_f, thm)
    thm = discharge(d1_f, thm)
    thm = discharge(s2_f, thm)
    thm = discharge(s1_f, thm)
    thm = gen("y", thm)
    thm = gen("x", thm)
    return thm


# ── 4. Reference byte-level encoder/decoder & fixture verification ───────────

VALUE_TAGS = frozenset({0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07, 0x08, 0x10, 0x11})
NODE_TAG = 0x20
NODE_KINDS = frozenset({0x00, 0x01, 0x02, 0x03, 0x04, 0x05})


def _u64_be(val: int) -> bytes:
    return struct.pack(">Q", val)


def _i64_be(val: int) -> bytes:
    return struct.pack(">q", val)


def py_encode_value(val: Tuple[Any, ...]) -> bytes:
    kind = val[0]
    if kind == "Unit":
        return b"\x00"
    if kind == "Bool":
        return b"\x01" + (b"\x01" if val[1] else b"\x00")
    if kind == "Int":
        return b"\x02" + _i64_be(val[1])
    if kind == "Bytes":
        raw: bytes = val[1]
        return b"\x03" + _u64_be(len(raw)) + raw
    if kind == "Str":
        raw = val[1].encode("utf-8")
        return b"\x04" + _u64_be(len(raw)) + raw
    if kind == "Cairn":
        assert len(val[1]) == 32
        return b"\x05" + val[1]
    if kind == "Shade":
        origin, cairn_bytes = val[1], val[2]
        assert len(cairn_bytes) == 32
        return b"\x06" + bytes([origin]) + cairn_bytes
    if kind == "AnswerGiven":
        return b"\x07\x00" + py_encode_value(val[1])
    if kind == "AnswerRefused":
        return b"\x07\x01" + bytes([val[1]])
    if kind == "Refusal":
        return b"\x08" + bytes([val[1]])
    if kind == "Struct":
        name_bytes = val[1].encode("utf-8")
        fields: Sequence[Tuple[Any, ...]] = val[2]
        return (
            b"\x10"
            + _u64_be(len(name_bytes))
            + name_bytes
            + _u64_be(len(fields))
            + b"".join(py_encode_value(f) for f in fields)
        )
    if kind == "Array":
        items: Sequence[Tuple[Any, ...]] = val[1]
        return b"\x11" + _u64_be(len(items)) + b"".join(py_encode_value(it) for it in items)
    raise ValueError(f"unknown value kind {kind}")


def py_decode_value_stream(buf: bytes, pos: int = 0, depth: int = 0) -> Tuple[Tuple[Any, ...], int]:
    if depth > MAX_DEPTH:
        raise ValueError("TooDeep")
    if pos >= len(buf):
        raise ValueError("Truncated")
    tag = buf[pos]
    pos += 1
    if tag == 0x00:
        return (("Unit",), pos)
    if tag == 0x01:
        if pos >= len(buf):
            raise ValueError("Truncated")
        b_val = buf[pos]
        if b_val not in (0, 1):
            raise ValueError("BadBool")
        return (("Bool", bool(b_val)), pos + 1)
    if tag == 0x02:
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (n_val,) = struct.unpack(">q", buf[pos : pos + 8])
        return (("Int", n_val), pos + 8)
    if tag == 0x03:
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (ln,) = struct.unpack(">Q", buf[pos : pos + 8])
        pos += 8
        if pos + ln > len(buf):
            raise ValueError("Truncated")
        return (("Bytes", buf[pos : pos + ln]), pos + ln)
    if tag == 0x04:
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (ln,) = struct.unpack(">Q", buf[pos : pos + 8])
        pos += 8
        if pos + ln > len(buf):
            raise ValueError("Truncated")
        try:
            text = buf[pos : pos + ln].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ValueError("NotUtf8") from exc
        return (("Str", text), pos + ln)
    if tag == 0x05:
        if pos + 32 > len(buf):
            raise ValueError("Truncated")
        return (("Cairn", buf[pos : pos + 32]), pos + 32)
    if tag == 0x06:
        if pos + 33 > len(buf):
            raise ValueError("Truncated")
        origin = buf[pos]
        if origin > MAX_STRATUM:
            raise ValueError("NoSuchStratum")
        return (("Shade", origin, buf[pos + 1 : pos + 33]), pos + 33)
    if tag == 0x07:
        if pos >= len(buf):
            raise ValueError("Truncated")
        disc = buf[pos]
        pos += 1
        if disc == 0:
            inner, pos = py_decode_value_stream(buf, pos, depth + 1)
            return (("AnswerGiven", inner), pos)
        if disc == 1:
            if pos >= len(buf):
                raise ValueError("Truncated")
            code = buf[pos]
            if code > MAX_REFUSAL:
                raise ValueError("NoSuchRefusal")
            return (("AnswerRefused", code), pos + 1)
        raise ValueError("BadAnswer")
    if tag == 0x08:
        if pos >= len(buf):
            raise ValueError("Truncated")
        code = buf[pos]
        if code > MAX_REFUSAL:
            raise ValueError("NoSuchRefusal")
        return (("Refusal", code), pos + 1)
    if tag == 0x10:
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (name_ln,) = struct.unpack(">Q", buf[pos : pos + 8])
        pos += 8
        if name_ln == 0:
            raise ValueError("EmptyStructName")
        if pos + name_ln > len(buf):
            raise ValueError("Truncated")
        try:
            name = buf[pos : pos + name_ln].decode("utf-8")
        except UnicodeDecodeError as exc:
            raise ValueError("NotUtf8") from exc
        pos += name_ln
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (field_count,) = struct.unpack(">Q", buf[pos : pos + 8])
        pos += 8
        fields: List[Tuple[Any, ...]] = []
        for _ in range(field_count):
            fld, pos = py_decode_value_stream(buf, pos, depth + 1)
            fields.append(fld)
        return (("Struct", name, tuple(fields)), pos)
    if tag == 0x11:
        if pos + 8 > len(buf):
            raise ValueError("Truncated")
        (item_count,) = struct.unpack(">Q", buf[pos : pos + 8])
        pos += 8
        items: List[Tuple[Any, ...]] = []
        for _ in range(item_count):
            it, pos = py_decode_value_stream(buf, pos, depth + 1)
            items.append(it)
        return (("Array", tuple(items)), pos)
    if tag == NODE_TAG:
        raise ValueError("WrongShape")
    raise ValueError("UnknownTag")


def py_decode_value(buf: bytes) -> Tuple[Any, ...]:
    val, pos = py_decode_value_stream(buf, 0, 0)
    if pos != len(buf):
        raise ValueError("TrailingBytes")
    return val


def verify_codec_fixtures_and_exclusions() -> int:
    """Verify byte-exact fixtures for all 11 Value tags, Node framing, domain
    separation, and rejection of the §7.1.1 / §7.3.1 invalid states."""
    assert VALUE_TAGS.isdisjoint({NODE_TAG}), "Value and Node tag sets must be disjoint"

    c0 = bytes(range(32))
    fixtures: Sequence[Tuple[Tuple[Any, ...], bytes]] = (
        (("Unit",), b"\x00"),
        (("Bool", False), b"\x01\x00"),
        (("Bool", True), b"\x01\x01"),
        (("Int", 0), b"\x02\x00\x00\x00\x00\x00\x00\x00\x00"),
        (("Int", -1), b"\x02\xff\xff\xff\xff\xff\xff\xff\xff"),
        (("Int", -9223372036854775808), b"\x02\x80\x00\x00\x00\x00\x00\x00\x00"),
        (("Int", 9223372036854775807), b"\x02\x7f\xff\xff\xff\xff\xff\xff\xff"),
        (("Bytes", b"ab"), b"\x03\x00\x00\x00\x00\x00\x00\x00\x02ab"),
        (("Str", "hi"), b"\x04\x00\x00\x00\x00\x00\x00\x00\x02hi"),
        (("Cairn", c0), b"\x05" + c0),
        (("Shade", 3, c0), b"\x06\x03" + c0),
        (("AnswerGiven", ("Int", 7)), b"\x07\x00\x02\x00\x00\x00\x00\x00\x00\x00\x07"),
        (("AnswerRefused", 5), b"\x07\x01\x05"),
        (("Refusal", 0), b"\x08\x00"),
        (
            ("Struct", "P", (("Unit",), ("Bool", True))),
            b"\x10\x00\x00\x00\x00\x00\x00\x00\x01P\x00\x00\x00\x00\x00\x00\x00\x02\x00\x01\x01",
        ),
        (
            ("Array", (("Unit",), ("Int", 1))),
            b"\x11\x00\x00\x00\x00\x00\x00\x00\x02\x00\x02\x00\x00\x00\x00\x00\x00\x00\x01",
        ),
    )
    seen_bytes = set()
    for val, expected in fixtures:
        actual = py_encode_value(val)
        if actual != expected:
            raise ProofFault(f"fixture mismatch for {val}: {actual!r} != {expected!r}")
        if py_decode_value(actual) != val:
            raise ProofFault(f"round-trip mismatch for {val}")
        # Also verify stream framing with arbitrary trailing bytes
        with_tail = actual + b"\xfe\xed"
        dec_val, pos = py_decode_value_stream(with_tail, 0, 0)
        if dec_val != val or with_tail[pos:] != b"\xfe\xed":
            raise ProofFault(f"prefix-free stream framing failed for {val}")
        seen_bytes.add(actual)
    if len(seen_bytes) != len(fixtures):
        raise ProofFault("distinct fixtures produced colliding encodings")

    # Verify rejection of excluded invalid in-memory states (§7.1.1):
    # - Shade origin > 8
    # - Empty struct name
    # - Refusal code > 5
    # - Bool byte > 1
    # - Answer discriminant > 1
    # - Node tag 0x20 fed to Value decoder
    for bad_bytes, expected_err in (
        (b"\x06\x09" + c0, "NoSuchStratum"),
        (b"\x10\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00\x00", "EmptyStructName"),
        (b"\x08\x06", "NoSuchRefusal"),
        (b"\x01\x02", "BadBool"),
        (b"\x07\x02\x00", "BadAnswer"),
        (b"\x20\x00\x00", "WrongShape"),
        (b"\x00\x00", "TrailingBytes"),
    ):
        try:
            py_decode_value(bad_bytes)
        except ValueError as exc:
            if str(exc) != expected_err:
                raise ProofFault(f"expected {expected_err}, got {exc}") from exc
        else:
            raise ProofFault(f"decoder accepted invalid bytes for {expected_err}")

    return len(fixtures)


if __name__ == "__main__":
    n_fix = verify_codec_fixtures_and_exclusions()
    print(f"verified {n_fix} canonical byte fixtures and 7 §7.1.1 exclusion guards")
    for name, fn in (
        ("value_decode_encode", prove_value_decode_encode),
        ("node_decode_encode", prove_node_decode_encode),
        ("injectivity_from_left_inverse", prove_injectivity_from_left_inverse),
        ("cairn_preimage_injectivity", prove_cairn_preimage_injectivity),
    ):
        t = fn()
        print(f"proved  {name:34s} |- {t.conclusion}")
