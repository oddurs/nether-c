//! The canonical encoding.
//!
//! Frozen in `spec/07-ledger.md` §7.1. One value, one byte string, on every
//! platform, forever. The property worth holding on to is stated there as the
//! round-trip law: for every byte string `b` that decodes to a value `v`,
//! `encode(v)` equals `b`.

use core::fmt;

use crate::cairn::Cairn;
use crate::node::{Call, Node, Span, kind};
use crate::value::{MAX_STRATUM, Value, tag};

/// How deeply values may nest before a decoder gives up.
///
/// The decoder reads bytes from a store that may be shared between people who
/// do not trust each other, and it recurses. Without a bound, a few hundred
/// bytes of nested arrays are a stack overflow.
pub const MAX_DEPTH: usize = 128;

// ── encoding ────────────────────────────────────────────────────────────────

impl Value {
    /// The canonical encoding of this value.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.encode_into(&mut out);
        out
    }

    fn encode_into(&self, out: &mut Vec<u8>) {
        out.push(self.tag());
        match self {
            Self::Unit => {}
            Self::Bool(b) => out.push(u8::from(*b)),
            Self::Int(n) => out.extend_from_slice(&n.to_be_bytes()),
            Self::Bytes(b) => {
                put_len(out, b.len());
                out.extend_from_slice(b);
            }
            Self::Str(s) => {
                put_len(out, s.len());
                out.extend_from_slice(s.as_bytes());
            }
            Self::Cairn(c) => out.extend_from_slice(c.as_bytes()),
            Self::Shade { origin, value } => {
                out.push(*origin);
                out.extend_from_slice(value.as_bytes());
            }
            Self::Struct { name, fields } => {
                put_len(out, name.len());
                out.extend_from_slice(name.as_bytes());
                put_len(out, fields.len());
                for field in fields {
                    field.encode_into(out);
                }
            }
            Self::Array(items) => {
                put_len(out, items.len());
                for item in items {
                    item.encode_into(out);
                }
            }
        }
    }
}

/// Lengths and counts are `u64` big-endian, however small they are.
fn put_len(out: &mut Vec<u8>, n: usize) {
    out.extend_from_slice(&(n as u64).to_be_bytes());
}

// ── nodes ───────────────────────────────────────────────────────────────────

impl Node {
    /// The canonical encoding of this node.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut out = vec![tag::NODE, self.kind()];
        match self {
            Self::Literal(value) => value.encode_into(&mut out),
            Self::Apply { function, args, result } => {
                out.extend_from_slice(function.as_bytes());
                put_cairns(&mut out, args);
                out.extend_from_slice(result.as_bytes());
            }
            Self::Hole { call, stratum, span, depends } => {
                put_call(&mut out, call);
                out.push(*stratum);
                put_span(&mut out, span);
                put_cairns(&mut out, depends);
            }
            Self::Deposit { value, span } => {
                out.extend_from_slice(value.as_bytes());
                put_span(&mut out, span);
            }
            Self::Witness { stratum, call, answer, span } => {
                out.push(*stratum);
                put_call(&mut out, call);
                out.extend_from_slice(answer.as_bytes());
                put_span(&mut out, span);
            }
            Self::Trace { roots, fuel_spent, depth, unrecorded } => {
                put_cairns(&mut out, roots);
                out.extend_from_slice(&fuel_spent.to_be_bytes());
                out.push(*depth);
                out.push(u8::from(*unrecorded));
            }
        }
        out
    }
}

fn put_cairns(out: &mut Vec<u8>, cairns: &[Cairn]) {
    put_len(out, cairns.len());
    for cairn in cairns {
        out.extend_from_slice(cairn.as_bytes());
    }
}

fn put_span(out: &mut Vec<u8>, span: &Span) {
    out.extend_from_slice(span.source.as_bytes());
    out.extend_from_slice(&span.start.to_be_bytes());
    out.extend_from_slice(&span.end.to_be_bytes());
}

fn put_call(out: &mut Vec<u8>, call: &Call) {
    put_len(out, call.function.len());
    out.extend_from_slice(call.function.as_bytes());
    put_cairns(out, &call.args);
}

// ── decoding ────────────────────────────────────────────────────────────────

/// Why a byte string was not the canonical encoding of anything.
///
/// Every variant corresponds to a clause of `spec/07-ledger.md` §7.1.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A tag byte that is not in the table.
    UnknownTag(u8),
    /// A `Bool` payload other than `0x00` or `0x01`.
    BadBool(u8),
    /// A `Str` or struct name that was not well-formed UTF-8.
    NotUtf8,
    /// A struct with no name. A nominal type without a name is not one.
    EmptyStructName,
    /// A length or count ran past the end of the input.
    Truncated {
        /// How many bytes the payload said it needed.
        needed: usize,
        /// How many were left.
        had: usize,
    },
    /// A length that does not fit in this platform's address space.
    LengthOverflow(u64),
    /// A complete value, and then more bytes.
    TrailingBytes(usize),
    /// Nesting beyond [`MAX_DEPTH`].
    TooDeep,
    /// A node kind byte that is not in the table.
    UnknownKind(u8),
    /// A stratum or depth below the bottom of the lattice.
    ///
    /// Shades carry an origin, holes and witnesses carry the stratum they
    /// reached, and traces carry the deepest they got. All four are the same
    /// claim about the same lattice, so they are rejected the same way.
    NoSuchStratum(u8),
    /// A stratum-8 mark other than `0x00` or `0x01`.
    BadMark(u8),
    /// A call with no function name.
    EmptyCallName,
    /// A span that ends before it starts.
    BackwardsSpan {
        /// Where it claimed to start.
        start: u64,
        /// Where it claimed to end.
        end: u64,
    },
    /// A node where a value was expected, or the reverse.
    WrongShape,
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTag(t) => write!(f, "unknown tag {t:#04x}"),
            Self::BadBool(b) => write!(f, "a Bool payload is 0x00 or 0x01, this is {b:#04x}"),
            Self::NotUtf8 => f.write_str("not well-formed UTF-8"),
            Self::EmptyStructName => {
                f.write_str("a struct's name is part of its value and cannot be empty")
            }
            Self::Truncated { needed, had } => write!(f, "needed {needed} more bytes, had {had}"),
            Self::LengthOverflow(n) => write!(f, "length {n} does not fit in this address space"),
            Self::TrailingBytes(n) => write!(f, "a complete value, and then {n} more bytes"),
            Self::TooDeep => write!(f, "nested deeper than {MAX_DEPTH}"),
            Self::UnknownKind(k) => write!(f, "unknown node kind {k:#04x}"),
            Self::NoSuchStratum(s) => write!(f, "no stratum {s}; the deepest is {MAX_STRATUM}"),
            Self::BadMark(b) => write!(f, "a stratum-8 mark is 0x00 or 0x01, this is {b:#04x}"),
            Self::EmptyCallName => {
                f.write_str("a call names a prelude function; the name was empty")
            }
            Self::BackwardsSpan { start, end } => {
                write!(f, "span ends at {end}, before it starts at {start}")
            }
            Self::WrongShape => f.write_str("a node where a value was expected, or the reverse"),
        }
    }
}

impl core::error::Error for DecodeError {}

/// A thing the ledger holds: a value, or a node about evaluation.
///
/// Both are addressed the same way, so the store reads back either.
/// `spec/07-ledger.md` §7.5.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stored {
    /// A value a program could hold.
    Value(Value),
    /// A record of something that happened.
    Node(Node),
}

impl Stored {
    /// The canonical encoding.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        match self {
            Self::Value(v) => v.encode(),
            Self::Node(n) => n.encode(),
        }
    }

    /// Its name.
    #[must_use]
    pub fn cairn(&self) -> Cairn {
        Cairn::of_encoded(&self.encode())
    }
}

/// Decodes one value or node from `bytes`.
///
/// # Errors
///
/// [`DecodeError`] for anything that is not a canonical encoding, including
/// trailing bytes after an otherwise complete one.
pub fn decode_stored(bytes: &[u8]) -> Result<Stored, DecodeError> {
    let mut reader = Reader { bytes, at: 0 };
    let stored = if reader.bytes.first() == Some(&tag::NODE) {
        reader.at = 1;
        Stored::Node(reader.node()?)
    } else {
        Stored::Value(reader.value(0)?)
    };
    let left = reader.remaining();
    if left > 0 {
        return Err(DecodeError::TrailingBytes(left));
    }
    Ok(stored)
}

/// Decodes exactly one node from `bytes`.
///
/// # Errors
///
/// [`DecodeError::WrongShape`] if the bytes are a value, otherwise as
/// [`decode_stored`].
pub fn decode_node(bytes: &[u8]) -> Result<Node, DecodeError> {
    match decode_stored(bytes)? {
        Stored::Node(n) => Ok(n),
        Stored::Value(_) => Err(DecodeError::WrongShape),
    }
}

/// Decodes exactly one value from `bytes`.
///
/// # Errors
///
/// [`DecodeError`] for anything that is not the canonical encoding of a value,
/// including trailing bytes after an otherwise complete one. Rejection is
/// specified rather than left to judgement: see `spec/07-ledger.md` §7.1.1.
pub fn decode(bytes: &[u8]) -> Result<Value, DecodeError> {
    match decode_stored(bytes)? {
        Stored::Value(v) => Ok(v),
        Stored::Node(_) => Err(DecodeError::WrongShape),
    }
}

struct Reader<'a> {
    bytes: &'a [u8],
    at: usize,
}

impl Reader<'_> {
    const fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, n: usize) -> Result<&[u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::Truncated { needed: n, had: self.remaining() });
        }
        let slice = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(slice)
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn len(&mut self) -> Result<usize, DecodeError> {
        let raw = u64::from_be_bytes(self.take(8)?.try_into().expect("took exactly eight"));
        usize::try_from(raw).map_err(|_| DecodeError::LengthOverflow(raw))
    }

    fn cairn(&mut self) -> Result<Cairn, DecodeError> {
        let bytes: [u8; 32] = self.take(32)?.try_into().expect("took exactly thirty-two");
        Ok(Cairn::from_bytes(bytes))
    }

    fn text(&mut self) -> Result<String, DecodeError> {
        let n = self.len()?;
        let raw = self.take(n)?;
        core::str::from_utf8(raw).map(ToOwned::to_owned).map_err(|_| DecodeError::NotUtf8)
    }

    /// A count is only believable if the input could carry that many things.
    ///
    /// Every element costs at least one byte, so a count past the bytes
    /// remaining is a lie, and believing it means reserving for it. Spec
    /// §7.1.1 requires rejecting a count that exceeds the bytes remaining.
    fn believable(&self, count: usize, each: usize) -> Result<(), DecodeError> {
        let needed = count.saturating_mul(each);
        if needed > self.remaining() {
            return Err(DecodeError::Truncated { needed, had: self.remaining() });
        }
        Ok(())
    }

    fn cairns(&mut self) -> Result<Vec<Cairn>, DecodeError> {
        let count = self.len()?;
        self.believable(count, 32)?;
        let mut out = Vec::with_capacity(count);
        for _ in 0..count {
            out.push(self.cairn()?);
        }
        Ok(out)
    }

    fn span(&mut self) -> Result<Span, DecodeError> {
        let source = self.cairn()?;
        let start = u64::from_be_bytes(self.take(8)?.try_into().expect("took exactly eight"));
        let end = u64::from_be_bytes(self.take(8)?.try_into().expect("took exactly eight"));
        if end < start {
            return Err(DecodeError::BackwardsSpan { start, end });
        }
        Ok(Span { source, start, end })
    }

    fn call(&mut self) -> Result<Call, DecodeError> {
        let function = self.text()?;
        if function.is_empty() {
            return Err(DecodeError::EmptyCallName);
        }
        Ok(Call { function, args: self.cairns()? })
    }

    fn stratum(&mut self) -> Result<u8, DecodeError> {
        let s = self.byte()?;
        if s > MAX_STRATUM {
            return Err(DecodeError::NoSuchStratum(s));
        }
        Ok(s)
    }

    fn node(&mut self) -> Result<Node, DecodeError> {
        match self.byte()? {
            kind::LITERAL => Ok(Node::Literal(self.value(0)?)),
            kind::APPLY => Ok(Node::Apply {
                function: self.cairn()?,
                args: self.cairns()?,
                result: self.cairn()?,
            }),
            kind::HOLE => Ok(Node::Hole {
                call: self.call()?,
                stratum: self.stratum()?,
                span: self.span()?,
                depends: self.cairns()?,
            }),
            kind::DEPOSIT => Ok(Node::Deposit { value: self.cairn()?, span: self.span()? }),
            kind::WITNESS => Ok(Node::Witness {
                stratum: self.stratum()?,
                call: self.call()?,
                answer: self.cairn()?,
                span: self.span()?,
            }),
            kind::TRACE => {
                let roots = self.cairns()?;
                let fuel_spent =
                    u64::from_be_bytes(self.take(8)?.try_into().expect("took exactly eight"));
                let depth = self.stratum()?;
                let unrecorded = match self.byte()? {
                    0x00 => false,
                    0x01 => true,
                    other => return Err(DecodeError::BadMark(other)),
                };
                Ok(Node::Trace { roots, fuel_spent, depth, unrecorded })
            }
            other => Err(DecodeError::UnknownKind(other)),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::TooDeep);
        }
        match self.byte()? {
            tag::UNIT => Ok(Value::Unit),
            tag::BOOL => match self.byte()? {
                0x00 => Ok(Value::Bool(false)),
                0x01 => Ok(Value::Bool(true)),
                other => Err(DecodeError::BadBool(other)),
            },
            tag::INT => {
                let raw: [u8; 8] = self.take(8)?.try_into().expect("took exactly eight");
                Ok(Value::Int(i64::from_be_bytes(raw)))
            }
            tag::BYTES => {
                let n = self.len()?;
                Ok(Value::Bytes(self.take(n)?.to_vec()))
            }
            tag::STR => Ok(Value::Str(self.text()?)),
            tag::CAIRN => Ok(Value::Cairn(self.cairn()?)),
            tag::SHADE => {
                let origin = self.byte()?;
                if origin > MAX_STRATUM {
                    return Err(DecodeError::NoSuchStratum(origin));
                }
                Ok(Value::Shade { origin, value: self.cairn()? })
            }
            tag::STRUCT => {
                let name = self.text()?;
                if name.is_empty() {
                    return Err(DecodeError::EmptyStructName);
                }
                let count = self.len()?;
                self.believable(count, 1)?;
                let mut fields = Vec::with_capacity(count);
                for _ in 0..count {
                    fields.push(self.value(depth + 1)?);
                }
                Ok(Value::Struct { name, fields })
            }
            tag::ARRAY => {
                let count = self.len()?;
                self.believable(count, 1)?;
                let mut items = Vec::with_capacity(count);
                for _ in 0..count {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            tag::NODE => Err(DecodeError::WrongShape),
            other => Err(DecodeError::UnknownTag(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, MAX_DEPTH, decode};
    use crate::cairn::Cairn;
    use crate::value::{Value, tag};

    fn sample() -> Vec<Value> {
        let c = Cairn::of_encoded(b"named");
        vec![
            Value::Unit,
            Value::Bool(false),
            Value::Bool(true),
            Value::Int(0),
            Value::Int(-1),
            Value::Int(i64::MIN),
            Value::Int(i64::MAX),
            Value::Bytes(vec![]),
            Value::Bytes(vec![0, 1, 2, 255]),
            Value::Str(String::new()),
            Value::Str("café — 深い".to_owned()),
            Value::Cairn(c),
            Value::Shade { origin: 0, value: c },
            Value::Shade { origin: 8, value: c },
            Value::Array(vec![]),
            Value::Array(vec![Value::Int(1), Value::Unit]),
            Value::Struct { name: "Header".to_owned(), fields: vec![] },
            Value::Struct {
                name: "Header".to_owned(),
                fields: vec![Value::Int(3), Value::Bytes(b"nc".to_vec())],
            },
            Value::Array(vec![Value::Array(vec![Value::Struct {
                name: "Deep".to_owned(),
                fields: vec![Value::Cairn(c)],
            }])]),
        ]
    }

    #[test]
    fn values_round_trip() {
        for value in sample() {
            let bytes = value.encode();
            assert_eq!(decode(&bytes).unwrap(), value, "decoding {value:?}");
        }
    }

    /// The law from §7.1.1: for every `b` that decodes to `v`, `encode(v) == b`.
    #[test]
    fn decoding_then_encoding_reproduces_the_bytes() {
        for value in sample() {
            let bytes = value.encode();
            assert_eq!(decode(&bytes).unwrap().encode(), bytes);
        }
    }

    #[test]
    fn distinct_values_have_distinct_names() {
        let mut seen = std::collections::HashSet::new();
        for value in sample() {
            assert!(seen.insert(value.cairn()), "{value:?} collided");
        }
    }

    /// A struct's name is part of its value, so two shapes are two values.
    #[test]
    fn nominal_typing_reaches_the_encoding() {
        let a = Value::Struct { name: "A".to_owned(), fields: vec![Value::Int(1)] };
        let b = Value::Struct { name: "B".to_owned(), fields: vec![Value::Int(1)] };
        assert_ne!(a.cairn(), b.cairn());
    }

    /// The ledger stores what it is handed. Normalisation is the lexer's job.
    #[test]
    fn text_is_not_normalised() {
        let composed = Value::Str("\u{e9}".to_owned()); // é
        let decomposed = Value::Str("e\u{301}".to_owned()); // e + combining acute
        assert_ne!(composed.encode(), decomposed.encode());
        assert_eq!(decode(&decomposed.encode()).unwrap(), decomposed);
    }

    #[test]
    fn lengths_are_eight_bytes_however_small() {
        let bytes = Value::Bytes(vec![7]).encode();
        assert_eq!(bytes, [&[tag::BYTES][..], &0u64.to_be_bytes()[..7], &[1, 7]].concat());
    }

    // ── rejection, clause by clause ─────────────────────────────────────────

    #[test]
    fn rejects_unknown_tags() {
        assert_eq!(decode(&[0xff]), Err(DecodeError::UnknownTag(0xff)));
        assert_eq!(decode(&[0x07]), Err(DecodeError::UnknownTag(0x07)));
    }

    /// `0x20` is dispatched as a node rather than rejected as a tag, so a bare
    /// one is an incomplete node and says so. Handing a *complete* node to the
    /// value decoder is the wrong shape, which `node_tests` covers.
    #[test]
    fn the_node_tag_is_dispatched_not_rejected() {
        assert!(matches!(decode(&[tag::NODE]), Err(DecodeError::Truncated { .. })));
    }

    /// §5.3 compares values by cairn, and a shade's origin is in its encoding.
    ///
    /// So the same bytes carried up out of two different strata are two values
    /// and not one. The alternative — dropping the origin byte — would mean a
    /// shade read back out of the ledger had lost the one thing that makes the
    /// Orpheus rule checkable. §90.2.
    #[test]
    fn a_shades_origin_is_part_of_its_name() {
        let inside = Cairn::of_encoded(b"the same bytes");
        let from_disk = Value::Shade { origin: 3, value: inside };
        let from_net = Value::Shade { origin: 5, value: inside };
        assert_ne!(from_disk.cairn(), from_net.cairn());
        // And neither is the name of what it holds: `seal` on a shade names
        // the shade. §1.5.
        assert_ne!(from_disk.cairn(), inside);
    }

    #[test]
    fn rejects_non_canonical_bools() {
        assert_eq!(decode(&[tag::BOOL, 0x02]), Err(DecodeError::BadBool(0x02)));
        assert_eq!(decode(&[tag::BOOL, 0xff]), Err(DecodeError::BadBool(0xff)));
    }

    #[test]
    fn rejects_bad_utf8() {
        let mut bytes = vec![tag::STR];
        bytes.extend_from_slice(&2u64.to_be_bytes());
        bytes.extend_from_slice(&[0xc3, 0x28]);
        assert_eq!(decode(&bytes), Err(DecodeError::NotUtf8));
    }

    #[test]
    fn rejects_strata_that_do_not_exist() {
        let mut bytes = vec![tag::SHADE, 9];
        bytes.extend_from_slice(Cairn::of_encoded(b"x").as_bytes());
        assert_eq!(decode(&bytes), Err(DecodeError::NoSuchStratum(9)));
    }

    #[test]
    fn rejects_nameless_structs() {
        let mut bytes = vec![tag::STRUCT];
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        assert_eq!(decode(&bytes), Err(DecodeError::EmptyStructName));
    }

    #[test]
    fn rejects_lengths_that_run_off_the_end() {
        let mut bytes = vec![tag::BYTES];
        bytes.extend_from_slice(&16u64.to_be_bytes());
        bytes.extend_from_slice(&[1, 2, 3]);
        assert_eq!(decode(&bytes), Err(DecodeError::Truncated { needed: 16, had: 3 }));
    }

    /// Found by review. `cairns()` guarded its count; structs and arrays did
    /// not, and a `Value` is larger than a cairn, so the amplification was
    /// worse. Eight megabytes of input reserved 396 MB before erroring.
    #[test]
    fn rejects_counts_larger_than_the_input_could_carry() {
        for tag_byte in [tag::ARRAY, tag::STRUCT] {
            let mut bytes = vec![tag_byte];
            if tag_byte == tag::STRUCT {
                bytes.extend_from_slice(&1u64.to_be_bytes());
                bytes.push(b'S');
            }
            bytes.extend_from_slice(&u64::MAX.to_be_bytes());
            bytes.extend_from_slice(&[tag::UNIT; 8]);
            assert!(
                matches!(decode(&bytes), Err(DecodeError::Truncated { .. })),
                "tag {tag_byte:#04x} believed a count of u64::MAX"
            );
        }
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = Value::Unit.encode();
        bytes.push(0x00);
        assert_eq!(decode(&bytes), Err(DecodeError::TrailingBytes(1)));
    }

    #[test]
    fn rejects_truncation_at_every_prefix() {
        for value in sample() {
            let bytes = value.encode();
            for cut in 0..bytes.len() {
                assert!(decode(&bytes[..cut]).is_err(), "{value:?} accepted a {cut}-byte prefix");
            }
        }
    }

    /// A few hundred bytes of nested arrays must not be a stack overflow.
    #[test]
    fn rejects_nesting_past_the_limit() {
        let mut bytes = Vec::new();
        for _ in 0..=MAX_DEPTH + 1 {
            bytes.push(tag::ARRAY);
            bytes.extend_from_slice(&1u64.to_be_bytes());
        }
        bytes.push(tag::UNIT);
        assert_eq!(decode(&bytes), Err(DecodeError::TooDeep));
    }

    #[test]
    fn accepts_nesting_up_to_the_limit() {
        let mut value = Value::Unit;
        for _ in 0..MAX_DEPTH {
            value = Value::Array(vec![value]);
        }
        assert_eq!(decode(&value.encode()).unwrap(), value);
    }
}

#[cfg(test)]
mod node_tests {
    use super::{DecodeError, Stored, decode, decode_node, decode_stored};
    use crate::cairn::Cairn;
    use crate::node::{Call, Node, Span, kind};
    use crate::value::{Value, tag};

    fn c(seed: &[u8]) -> Cairn {
        Cairn::of_encoded(seed)
    }

    fn span() -> Span {
        Span { source: c(b"hello.nc"), start: 12, end: 31 }
    }

    fn call() -> Call {
        Call { function: "read".to_owned(), args: vec![c(b"main.nc")] }
    }

    fn sample() -> Vec<Node> {
        vec![
            Node::Literal(Value::Int(7)),
            Node::Literal(Value::Array(vec![Value::Unit])),
            Node::Apply { function: c(b"f"), args: vec![], result: c(b"r") },
            Node::Apply { function: c(b"f"), args: vec![c(b"a"), c(b"b")], result: c(b"r") },
            Node::Hole { call: call(), stratum: 3, span: span(), depends: vec![c(b"d")] },
            Node::Hole {
                call: Call { function: "draw".to_owned(), args: vec![] },
                stratum: 7,
                span: span(),
                depends: vec![],
            },
            Node::Deposit { value: c(b"greeting"), span: span() },
            Node::Witness { stratum: 3, call: call(), answer: c(b"bytes"), span: span() },
            Node::Trace { roots: vec![c(b"obj")], fuel_spent: 903, depth: 3, unrecorded: false },
            Node::Trace { roots: vec![], fuel_spent: 0, depth: 8, unrecorded: true },
        ]
    }

    #[test]
    fn nodes_round_trip() {
        for node in sample() {
            assert_eq!(decode_node(&node.encode()).unwrap(), node, "decoding {node:?}");
        }
    }

    #[test]
    fn decoding_then_encoding_reproduces_the_bytes() {
        for node in sample() {
            let bytes = node.encode();
            assert_eq!(decode_node(&bytes).unwrap().encode(), bytes);
        }
    }

    #[test]
    fn every_node_begins_with_the_node_tag() {
        for node in sample() {
            assert_eq!(node.encode()[0], tag::NODE);
        }
    }

    #[test]
    fn distinct_nodes_have_distinct_names() {
        let mut seen = std::collections::HashSet::new();
        for node in sample() {
            assert!(seen.insert(node.cairn()), "{node:?} collided");
        }
    }

    #[test]
    fn values_and_nodes_share_one_address_space() {
        for node in sample() {
            assert_eq!(decode_stored(&node.encode()).unwrap(), Stored::Node(node));
        }
        let value = Value::Int(7);
        assert_eq!(decode_stored(&value.encode()).unwrap(), Stored::Value(value));
    }

    /// A `Literal(v)` is a different thing from `v`, and must have a different name.
    #[test]
    fn wrapping_a_value_in_a_node_renames_it() {
        let value = Value::Int(7);
        assert_ne!(Node::Literal(value.clone()).cairn(), value.cairn());
    }

    #[test]
    fn asking_for_the_wrong_shape_is_an_error() {
        assert_eq!(decode(&sample()[0].encode()), Err(DecodeError::WrongShape));
        assert_eq!(decode_node(&Value::Unit.encode()), Err(DecodeError::WrongShape));
    }

    /// The forward edge. Provenance is this, read backwards.
    #[test]
    fn references_names_every_cairn_the_node_holds() {
        let hole = Node::Hole { call: call(), stratum: 3, span: span(), depends: vec![c(b"d")] };
        assert_eq!(hole.references(), vec![c(b"main.nc"), c(b"hello.nc"), c(b"d")]);
        assert!(Node::Literal(Value::Unit).references().is_empty());

        // Nothing a node holds may be missing from `references`, or the reverse
        // index is incomplete and provenance silently stops early.
        for node in sample() {
            let encoded = node.encode();
            for cairn in node.references() {
                assert!(
                    encoded.windows(32).any(|w| w == cairn.as_bytes()),
                    "{node:?} claims {cairn:?} but does not encode it"
                );
            }
        }
    }

    // ── rejection ───────────────────────────────────────────────────────────

    #[test]
    fn rejects_unknown_kinds() {
        assert_eq!(decode_node(&[tag::NODE, 0x7f]), Err(DecodeError::UnknownKind(0x7f)));
    }

    #[test]
    fn rejects_strata_below_the_lattice() {
        let mut bytes = vec![tag::NODE, kind::WITNESS, 9];
        bytes.extend_from_slice(&0u64.to_be_bytes());
        assert_eq!(decode_node(&bytes), Err(DecodeError::NoSuchStratum(9)));
    }

    #[test]
    fn rejects_nameless_calls() {
        let mut bytes = vec![tag::NODE, kind::HOLE];
        bytes.extend_from_slice(&0u64.to_be_bytes());
        assert_eq!(decode_node(&bytes), Err(DecodeError::EmptyCallName));
    }

    #[test]
    fn rejects_backwards_spans() {
        let mut bytes = vec![tag::NODE, kind::DEPOSIT];
        bytes.extend_from_slice(c(b"v").as_bytes());
        bytes.extend_from_slice(c(b"s").as_bytes());
        bytes.extend_from_slice(&9u64.to_be_bytes());
        bytes.extend_from_slice(&4u64.to_be_bytes());
        assert_eq!(decode_node(&bytes), Err(DecodeError::BackwardsSpan { start: 9, end: 4 }));
    }

    #[test]
    fn rejects_marks_that_are_not_a_bit() {
        let mut bytes = vec![tag::NODE, kind::TRACE];
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.push(0);
        bytes.push(0x02);
        assert_eq!(decode_node(&bytes), Err(DecodeError::BadMark(0x02)));
    }

    /// A count of four billion cairns must not become a four-billion allocation.
    #[test]
    fn rejects_counts_the_input_cannot_carry() {
        let mut bytes = vec![tag::NODE, kind::TRACE];
        bytes.extend_from_slice(&u64::from(u32::MAX).to_be_bytes());
        assert!(matches!(decode_node(&bytes), Err(DecodeError::Truncated { .. })));
    }

    /// Found by `tests/fuzz.rs`. The guard on a cairn count saturated, and the
    /// error message built alongside it did not, so a count near `usize::MAX`
    /// passed the check and then panicked constructing the complaint about it.
    /// Pinned here because a fuzzer finding it again is luck, not coverage.
    #[test]
    fn a_huge_cairn_count_does_not_overflow_its_own_error() {
        for kind_byte in [kind::APPLY, kind::TRACE] {
            let mut bytes = vec![tag::NODE, kind_byte];
            if kind_byte == kind::APPLY {
                bytes.extend_from_slice(c(b"f").as_bytes());
            }
            bytes.extend_from_slice(&u64::MAX.to_be_bytes());
            assert!(matches!(decode_node(&bytes), Err(DecodeError::Truncated { .. })));
        }
    }

    #[test]
    fn rejects_truncation_at_every_prefix() {
        for node in sample() {
            let bytes = node.encode();
            for cut in 0..bytes.len() {
                assert!(decode_node(&bytes[..cut]).is_err(), "{node:?} accepted {cut} bytes");
            }
        }
    }

    #[test]
    fn rejects_trailing_bytes() {
        let mut bytes = sample()[0].encode();
        bytes.push(0);
        assert_eq!(decode_node(&bytes), Err(DecodeError::TrailingBytes(1)));
    }
}
