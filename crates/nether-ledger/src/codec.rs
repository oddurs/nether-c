//! The canonical encoding.
//!
//! Frozen in `spec/07-ledger.md` §7.1. One value, one byte string, on every
//! platform, forever. The property worth holding on to is stated there as the
//! round-trip law: for every byte string `b` that decodes to a value `v`,
//! `encode(v)` equals `b`.

use core::fmt;

use crate::cairn::Cairn;
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

// ── decoding ────────────────────────────────────────────────────────────────

/// Why a byte string was not the canonical encoding of anything.
///
/// Every variant corresponds to a clause of `spec/07-ledger.md` §7.1.1.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DecodeError {
    /// A tag byte that is not in the table.
    UnknownTag(u8),
    /// A tag the format defines but this build cannot yet read.
    Unsupported(u8),
    /// A `Bool` payload other than `0x00` or `0x01`.
    BadBool(u8),
    /// A `Str` or struct name that was not well-formed UTF-8.
    NotUtf8,
    /// A shade claiming to come from below stratum 8.
    StratumTooDeep(u8),
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
}

impl fmt::Display for DecodeError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownTag(t) => write!(f, "unknown tag {t:#04x}"),
            Self::Unsupported(t) => write!(f, "tag {t:#04x} is defined but not read by this build"),
            Self::BadBool(b) => write!(f, "a Bool payload is 0x00 or 0x01, this is {b:#04x}"),
            Self::NotUtf8 => f.write_str("not well-formed UTF-8"),
            Self::StratumTooDeep(s) => write!(f, "no stratum {s}; the deepest is {MAX_STRATUM}"),
            Self::EmptyStructName => {
                f.write_str("a struct's name is part of its value and cannot be empty")
            }
            Self::Truncated { needed, had } => write!(f, "needed {needed} more bytes, had {had}"),
            Self::LengthOverflow(n) => write!(f, "length {n} does not fit in this address space"),
            Self::TrailingBytes(n) => write!(f, "a complete value, and then {n} more bytes"),
            Self::TooDeep => write!(f, "nested deeper than {MAX_DEPTH}"),
        }
    }
}

impl core::error::Error for DecodeError {}

/// Decodes exactly one value from `bytes`.
///
/// # Errors
///
/// [`DecodeError`] for anything that is not the canonical encoding of a value,
/// including trailing bytes after an otherwise complete one. Rejection is
/// specified rather than left to judgement: see `spec/07-ledger.md` §7.1.1.
pub fn decode(bytes: &[u8]) -> Result<Value, DecodeError> {
    let mut reader = Reader { bytes, at: 0 };
    let value = reader.value(0)?;
    let left = reader.remaining();
    if left > 0 {
        return Err(DecodeError::TrailingBytes(left));
    }
    Ok(value)
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
                    return Err(DecodeError::StratumTooDeep(origin));
                }
                Ok(Value::Shade { origin, value: self.cairn()? })
            }
            tag::STRUCT => {
                let name = self.text()?;
                if name.is_empty() {
                    return Err(DecodeError::EmptyStructName);
                }
                let count = self.len()?;
                let mut fields = Vec::new();
                for _ in 0..count {
                    fields.push(self.value(depth + 1)?);
                }
                Ok(Value::Struct { name, fields })
            }
            tag::ARRAY => {
                let count = self.len()?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            tag::NODE => Err(DecodeError::Unsupported(tag::NODE)),
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

    #[test]
    fn rejects_the_node_tag_as_unsupported_not_unknown() {
        assert_eq!(decode(&[tag::NODE]), Err(DecodeError::Unsupported(tag::NODE)));
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
        assert_eq!(decode(&bytes), Err(DecodeError::StratumTooDeep(9)));
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
