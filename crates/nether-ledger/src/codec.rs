//! The canonical encoding.
//!
//! Frozen in `spec/07-ledger.md` §7.1. One value, one byte string, on every
//! platform, forever. The property worth holding on to is stated there as the
//! round-trip law: for every byte string `b` that decodes to a value `v`,
//! `encode(v)` equals `b`.

use core::fmt;

use crate::cairn::Cairn;
use crate::node::{Call, Node, Span, kind};
use crate::value::{AnswerOf, MAX_REFUSAL, MAX_STRATUM, Refusal, Value, tag};

/// How deeply values may nest before a decoder gives up.
///
/// The decoder reads bytes from a store that may be shared between people who
/// do not trust each other, and it recurses. Without a bound, a few hundred
/// bytes of nested arrays are a stack overflow.
pub const MAX_DEPTH: usize = 128;

/// Maximum estimated in-memory weight of a single decoded value.
///
/// Bounds dense `Value::Array` / `Value::Struct` encodings (e.g., millions of
/// 1-byte `Value::Unit` elements) so a 64 MiB object cannot amplify into
/// gigabytes of heap memory on decode.
pub const MAX_DECODED_BYTES: usize = 256 * 1024 * 1024;

// ── encoding ────────────────────────────────────────────────────────────────

impl Value {
    /// Checks every invariant `decode` enforces without allocating bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if any nested value exceeds [`MAX_DEPTH`], has
    /// an out-of-range shade origin, has an empty struct name, or exceeds
    /// [`MAX_DECODED_BYTES`].
    pub fn validate(&self) -> Result<(), DecodeError> {
        let mut stack = vec![(self, 0usize)];
        let mut weight = 0usize;
        let val_sz = core::mem::size_of::<Self>();
        while let Some((cur, depth)) = stack.pop() {
            if depth > MAX_DEPTH {
                return Err(DecodeError::TooDeep);
            }
            let extra = match cur {
                Self::Bytes(b) => b.len(),
                Self::Str(s) => s.len(),
                Self::Struct { name, .. } => name.len(),
                _ => 0,
            };
            weight = weight.saturating_add(val_sz).saturating_add(extra);
            if weight > MAX_DECODED_BYTES {
                return Err(DecodeError::TooLarge { bytes: weight, limit: MAX_DECODED_BYTES });
            }
            match cur {
                Self::Shade { origin, .. } if *origin > MAX_STRATUM => {
                    return Err(DecodeError::NoSuchStratum(*origin));
                }
                Self::Struct { name, .. } if name.is_empty() => {
                    return Err(DecodeError::EmptyStructName);
                }
                Self::Struct { fields, .. } => {
                    for field in fields.iter().rev() {
                        stack.push((field, depth + 1));
                    }
                }
                Self::Array(items) => {
                    for item in items.iter().rev() {
                        stack.push((item, depth + 1));
                    }
                }
                Self::Answer(answer) => {
                    if let AnswerOf::Given(inner) = answer.as_ref() {
                        stack.push((inner, depth + 1));
                    }
                }
                _ => {}
            }
        }
        Ok(())
    }

    /// Validates invariants and returns the canonical encoding.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if [`Self::validate`] rejects this value.
    pub fn try_encode(&self) -> Result<Vec<u8>, DecodeError> {
        self.validate()?;
        Ok(self.encode())
    }

    /// The canonical encoding of this value.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut size = Count(0);
        self.encode_into(&mut size);
        let mut out = Vec::with_capacity(size.0);
        self.encode_into(&mut out);
        out
    }

    fn encode_into(&self, out: &mut impl Sink) {
        let mut stack = vec![self];
        while let Some(cur) = stack.pop() {
            out.push(cur.tag());
            match cur {
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
                    for field in fields.iter().rev() {
                        stack.push(field);
                    }
                }
                Self::Array(items) => {
                    put_len(out, items.len());
                    for item in items.iter().rev() {
                        stack.push(item);
                    }
                }
                Self::Answer(answer) => match answer.as_ref() {
                    AnswerOf::Given(value) => {
                        out.push(0x00);
                        stack.push(value);
                    }
                    AnswerOf::Refused(refusal) => {
                        out.push(0x01);
                        out.push(*refusal as u8);
                    }
                },
                Self::Refusal(refusal) => out.push(*refusal as u8),
            }
        }
    }
}

/// Where an encoding goes: into bytes, or only counted.
///
/// One traversal writes both, so the size an encoding asks for is the size it
/// writes by construction rather than by a second function kept in step with
/// the first. 0202.
trait Sink {
    fn push(&mut self, byte: u8);
    fn extend_from_slice(&mut self, bytes: &[u8]);
}

impl Sink for Vec<u8> {
    fn push(&mut self, byte: u8) {
        Vec::push(self, byte);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        Vec::extend_from_slice(self, bytes);
    }
}

/// A sink that keeps only how much it was given.
struct Count(usize);

impl Sink for Count {
    fn push(&mut self, _: u8) {
        self.0 = self.0.saturating_add(1);
    }
    fn extend_from_slice(&mut self, bytes: &[u8]) {
        self.0 = self.0.saturating_add(bytes.len());
    }
}

/// Lengths and counts are `u64` big-endian, however small they are.
fn put_len(out: &mut impl Sink, n: usize) {
    out.extend_from_slice(&(n as u64).to_be_bytes());
}

// ── nodes ───────────────────────────────────────────────────────────────────

impl Node {
    /// Checks every invariant `decode_node` enforces without allocating bytes.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if any field violates §7.3.1 (empty call name,
    /// backwards span, stratum past `MAX_STRATUM`, or invalid literal value).
    pub fn validate(&self) -> Result<(), DecodeError> {
        let check_span = |span: &Span| {
            if span.end < span.start {
                Err(DecodeError::BackwardsSpan { start: span.start, end: span.end })
            } else {
                Ok(())
            }
        };
        let check_call = |call: &Call| {
            if call.function.is_empty() { Err(DecodeError::EmptyCallName) } else { Ok(()) }
        };
        let check_stratum = |s: u8| {
            if s > MAX_STRATUM { Err(DecodeError::NoSuchStratum(s)) } else { Ok(()) }
        };
        match self {
            Self::Literal(v) => v.validate(),
            Self::Apply { .. } => Ok(()),
            Self::Hole { call, stratum, span } | Self::Witness { stratum, call, span, .. } => {
                check_stratum(*stratum)?;
                check_call(call)?;
                check_span(span)
            }
            Self::Deposit { span, .. } => check_span(span),
            Self::Trace { depth, .. } => check_stratum(*depth),
        }
    }

    /// Validates invariants and returns the canonical encoding.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if [`Self::validate`] rejects this node.
    pub fn try_encode(&self) -> Result<Vec<u8>, DecodeError> {
        self.validate()?;
        Ok(self.encode())
    }

    /// The canonical encoding of this node.
    #[must_use]
    pub fn encode(&self) -> Vec<u8> {
        let mut size = Count(0);
        self.encode_into(&mut size);
        let mut out = Vec::with_capacity(size.0);
        self.encode_into(&mut out);
        out
    }

    fn encode_into(&self, out: &mut impl Sink) {
        out.extend_from_slice(&[tag::NODE, self.kind()]);
        match self {
            Self::Literal(value) => value.encode_into(out),
            Self::Apply { function, args, result } => {
                out.extend_from_slice(function.as_bytes());
                put_cairns(out, args);
                out.extend_from_slice(result.as_bytes());
            }
            Self::Hole { call, stratum, span } => {
                put_call(out, call);
                out.push(*stratum);
                put_span(out, span);
            }
            Self::Deposit { value, span } => {
                out.extend_from_slice(value.as_bytes());
                put_span(out, span);
            }
            Self::Witness { stratum, call, answer, span } => {
                out.push(*stratum);
                put_call(out, call);
                out.extend_from_slice(answer.as_bytes());
                put_span(out, span);
            }
            Self::Trace { residue, holes, witnesses, deposits, source, depth, unrecorded } => {
                out.extend_from_slice(residue.as_bytes());
                put_cairns(out, holes);
                put_cairns(out, witnesses);
                put_cairns(out, deposits);
                out.extend_from_slice(source.as_bytes());
                out.push(*depth);
                out.push(u8::from(*unrecorded));
            }
        }
    }
}

fn put_cairns(out: &mut impl Sink, cairns: &[Cairn]) {
    put_len(out, cairns.len());
    for cairn in cairns {
        out.extend_from_slice(cairn.as_bytes());
    }
}

fn put_span(out: &mut impl Sink, span: &Span) {
    out.extend_from_slice(span.source.as_bytes());
    out.extend_from_slice(&span.start.to_be_bytes());
    out.extend_from_slice(&span.end.to_be_bytes());
}

fn put_call(out: &mut impl Sink, call: &Call) {
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
    /// An `Answer` discriminant other than `0x00` given or `0x01` refused.
    BadAnswer(u8),
    /// A refusal code outside the closed set of six.
    NoSuchRefusal(u8),
    /// An encoded object exceeding `MAX_OBJECT_BYTES`.
    TooLarge {
        /// Encoded byte length of the object.
        bytes: usize,
        /// Maximum permitted encoded byte length.
        limit: usize,
    },
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
            Self::BadAnswer(b) => {
                write!(f, "an Answer is given (0x00) or refused (0x01), this is {b:#04x}")
            }
            Self::NoSuchRefusal(c) => write!(f, "no refusal {c}; the highest is {MAX_REFUSAL}"),
            Self::TooLarge { bytes, limit } => {
                write!(f, "object is {bytes} bytes, past the {limit}-byte bound")
            }
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
    /// Validates invariants and returns the canonical encoding.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError`] if the value or node violates canonical encoding rules.
    pub fn try_encode(&self) -> Result<Vec<u8>, DecodeError> {
        match self {
            Self::Value(v) => v.try_encode(),
            Self::Node(n) => n.try_encode(),
        }
    }

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

/// Summary produced by [`scan_stored`] without materializing `Value` trees.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Scanned {
    pub is_node: bool,
    pub is_trace: bool,
    pub references: Vec<Cairn>,
    pub nodes: Vec<Cairn>,
}

/// Validates canonical encoding of one value or node and extracts node
/// references without allocating `Value` payloads.
pub(crate) fn scan_stored(bytes: &[u8]) -> Result<Scanned, DecodeError> {
    let mut reader = Reader { bytes, at: 0, weight: 0 };
    let scanned = if reader.bytes.first() == Some(&tag::NODE) {
        reader.at = 1;
        reader.scan_node()?
    } else {
        reader.skip_value(0)?;
        Scanned { is_node: false, is_trace: false, references: Vec::new(), nodes: Vec::new() }
    };
    let left = reader.remaining();
    if left > 0 {
        return Err(DecodeError::TrailingBytes(left));
    }
    Ok(scanned)
}

/// Decodes one value or node from `bytes`.
///
/// # Errors
///
/// [`DecodeError`] for anything that is not a canonical encoding, including
/// trailing bytes after an otherwise complete one.
pub fn decode_stored(bytes: &[u8]) -> Result<Stored, DecodeError> {
    let mut reader = Reader { bytes, at: 0, weight: 0 };
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
    weight: usize,
}

impl<'a> Reader<'a> {
    fn charge(&mut self, bytes: usize) -> Result<(), DecodeError> {
        self.weight = self.weight.saturating_add(bytes);
        if self.weight > MAX_DECODED_BYTES {
            return Err(DecodeError::TooLarge { bytes: self.weight, limit: MAX_DECODED_BYTES });
        }
        Ok(())
    }

    const fn remaining(&self) -> usize {
        self.bytes.len() - self.at
    }

    fn take(&mut self, n: usize) -> Result<&'a [u8], DecodeError> {
        if self.remaining() < n {
            return Err(DecodeError::Truncated { needed: n, had: self.remaining() });
        }
        let slice = &self.bytes[self.at..self.at + n];
        self.at += n;
        Ok(slice)
    }

    fn take_array<const N: usize>(&mut self) -> Result<[u8; N], DecodeError> {
        let slice = self.take(N)?;
        let mut out = [0u8; N];
        out.copy_from_slice(slice);
        Ok(out)
    }

    fn byte(&mut self) -> Result<u8, DecodeError> {
        Ok(self.take(1)?[0])
    }

    fn len(&mut self) -> Result<usize, DecodeError> {
        let raw = u64::from_be_bytes(self.take_array::<8>()?);
        usize::try_from(raw).map_err(|_| DecodeError::LengthOverflow(raw))
    }

    fn cairn(&mut self) -> Result<Cairn, DecodeError> {
        Ok(Cairn::from_bytes(self.take_array::<32>()?))
    }

    fn text_str(&mut self) -> Result<&'a str, DecodeError> {
        let n = self.len()?;
        let raw = self.take(n)?;
        core::str::from_utf8(raw).map_err(|_| DecodeError::NotUtf8)
    }

    fn text(&mut self) -> Result<String, DecodeError> {
        self.text_str().map(ToOwned::to_owned)
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
        let start = u64::from_be_bytes(self.take_array::<8>()?);
        let end = u64::from_be_bytes(self.take_array::<8>()?);
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

    fn refusal(&mut self) -> Result<Refusal, DecodeError> {
        let code = self.byte()?;
        Refusal::from_code(code).ok_or(DecodeError::NoSuchRefusal(code))
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
            kind::HOLE => {
                Ok(Node::Hole { call: self.call()?, stratum: self.stratum()?, span: self.span()? })
            }
            kind::DEPOSIT => Ok(Node::Deposit { value: self.cairn()?, span: self.span()? }),
            kind::WITNESS => Ok(Node::Witness {
                stratum: self.stratum()?,
                call: self.call()?,
                answer: self.cairn()?,
                span: self.span()?,
            }),
            kind::TRACE => {
                let residue = self.cairn()?;
                let holes = self.cairns()?;
                let witnesses = self.cairns()?;
                let deposits = self.cairns()?;
                let source = self.cairn()?;
                let depth = self.stratum()?;
                let unrecorded = match self.byte()? {
                    0x00 => false,
                    0x01 => true,
                    other => return Err(DecodeError::BadMark(other)),
                };
                Ok(Node::Trace { residue, holes, witnesses, deposits, source, depth, unrecorded })
            }
            other => Err(DecodeError::UnknownKind(other)),
        }
    }

    fn scan_node(&mut self) -> Result<Scanned, DecodeError> {
        match self.byte()? {
            kind::LITERAL => {
                self.skip_value(0)?;
                Ok(Scanned {
                    is_node: true,
                    is_trace: false,
                    references: Vec::new(),
                    nodes: Vec::new(),
                })
            }
            kind::APPLY => {
                let function = self.cairn()?;
                let args = self.cairns()?;
                let result = self.cairn()?;
                let mut references = Vec::with_capacity(args.len() + 2);
                references.push(function);
                references.extend_from_slice(&args);
                references.push(result);
                Ok(Scanned { is_node: true, is_trace: false, references, nodes: Vec::new() })
            }
            kind::HOLE => {
                let call = self.call()?;
                let _ = self.stratum()?;
                let span = self.span()?;
                let mut references = call.args;
                references.push(span.source);
                Ok(Scanned { is_node: true, is_trace: false, references, nodes: Vec::new() })
            }
            kind::DEPOSIT => {
                let value = self.cairn()?;
                let span = self.span()?;
                Ok(Scanned {
                    is_node: true,
                    is_trace: false,
                    references: vec![value, span.source],
                    nodes: Vec::new(),
                })
            }
            kind::WITNESS => {
                let _ = self.stratum()?;
                let call = self.call()?;
                let answer = self.cairn()?;
                let span = self.span()?;
                let mut references = call.args;
                references.push(answer);
                references.push(span.source);
                Ok(Scanned { is_node: true, is_trace: false, references, nodes: Vec::new() })
            }
            kind::TRACE => {
                let residue = self.cairn()?;
                let holes = self.cairns()?;
                let witnesses = self.cairns()?;
                let deposits = self.cairns()?;
                let source = self.cairn()?;
                let _ = self.stratum()?;
                match self.byte()? {
                    0x00 | 0x01 => {}
                    other => return Err(DecodeError::BadMark(other)),
                }
                let node_count = holes.len() + witnesses.len() + deposits.len();
                let mut nodes = Vec::with_capacity(node_count);
                nodes.extend_from_slice(&holes);
                nodes.extend_from_slice(&witnesses);
                nodes.extend_from_slice(&deposits);
                let mut references = Vec::with_capacity(node_count + 2);
                references.push(residue);
                references.extend_from_slice(&nodes);
                references.push(source);
                Ok(Scanned { is_node: true, is_trace: true, references, nodes })
            }
            other => Err(DecodeError::UnknownKind(other)),
        }
    }

    fn skip_value(&mut self, depth: usize) -> Result<(), DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::TooDeep);
        }
        self.charge(core::mem::size_of::<Value>())?;
        match self.byte()? {
            tag::UNIT => Ok(()),
            tag::BOOL => match self.byte()? {
                0x00 | 0x01 => Ok(()),
                other => Err(DecodeError::BadBool(other)),
            },
            tag::INT => {
                let _ = self.take(8)?;
                Ok(())
            }
            tag::BYTES => {
                let n = self.len()?;
                let _ = self.take(n)?;
                self.charge(n)
            }
            tag::STR => {
                let n = self.text_str()?.len();
                self.charge(n)
            }
            tag::CAIRN => {
                let _ = self.take(32)?;
                Ok(())
            }
            tag::SHADE => {
                let origin = self.byte()?;
                if origin > MAX_STRATUM {
                    return Err(DecodeError::NoSuchStratum(origin));
                }
                let _ = self.take(32)?;
                Ok(())
            }
            tag::STRUCT => {
                let name_len = self.text_str()?.len();
                if name_len == 0 {
                    return Err(DecodeError::EmptyStructName);
                }
                self.charge(name_len)?;
                let count = self.len()?;
                self.believable(count, 1)?;
                for _ in 0..count {
                    self.skip_value(depth + 1)?;
                }
                Ok(())
            }
            tag::ARRAY => {
                let count = self.len()?;
                self.believable(count, 1)?;
                for _ in 0..count {
                    self.skip_value(depth + 1)?;
                }
                Ok(())
            }
            tag::ANSWER => match self.byte()? {
                0x00 => self.skip_value(depth + 1),
                0x01 => {
                    let _ = self.refusal()?;
                    Ok(())
                }
                other => Err(DecodeError::BadAnswer(other)),
            },
            tag::REFUSAL => {
                let _ = self.refusal()?;
                Ok(())
            }
            tag::NODE => Err(DecodeError::WrongShape),
            other => Err(DecodeError::UnknownTag(other)),
        }
    }

    fn value(&mut self, depth: usize) -> Result<Value, DecodeError> {
        if depth > MAX_DEPTH {
            return Err(DecodeError::TooDeep);
        }
        self.charge(core::mem::size_of::<Value>())?;
        match self.byte()? {
            tag::UNIT => Ok(Value::Unit),
            tag::BOOL => match self.byte()? {
                0x00 => Ok(Value::Bool(false)),
                0x01 => Ok(Value::Bool(true)),
                other => Err(DecodeError::BadBool(other)),
            },
            tag::INT => Ok(Value::Int(i64::from_be_bytes(self.take_array::<8>()?))),
            tag::BYTES => {
                let n = self.len()?;
                let slice = self.take(n)?;
                self.charge(n)?;
                Ok(Value::Bytes(slice.to_vec()))
            }
            tag::STR => {
                let s = self.text_str()?;
                self.charge(s.len())?;
                Ok(Value::Str(s.to_owned()))
            }
            tag::CAIRN => Ok(Value::Cairn(self.cairn()?)),
            tag::SHADE => {
                let origin = self.byte()?;
                if origin > MAX_STRATUM {
                    return Err(DecodeError::NoSuchStratum(origin));
                }
                Ok(Value::Shade { origin, value: self.cairn()? })
            }
            tag::STRUCT => {
                let name_str = self.text_str()?;
                if name_str.is_empty() {
                    return Err(DecodeError::EmptyStructName);
                }
                self.charge(name_str.len())?;
                let name = name_str.to_owned();
                let count = self.len()?;
                self.believable(count, 1)?;
                // Grown, not reserved: a count is believable against the bytes
                // left, and every enclosing level is believed against the same
                // bytes, so reserving it nests into gigabytes. 0219.
                let mut fields = Vec::new();
                for _ in 0..count {
                    fields.push(self.value(depth + 1)?);
                }
                Ok(Value::Struct { name, fields })
            }
            tag::ARRAY => {
                let count = self.len()?;
                self.believable(count, 1)?;
                let mut items = Vec::new();
                for _ in 0..count {
                    items.push(self.value(depth + 1)?);
                }
                Ok(Value::Array(items))
            }
            tag::ANSWER => match self.byte()? {
                0x00 => Ok(Value::Answer(Box::new(AnswerOf::Given(self.value(depth + 1)?)))),
                0x01 => Ok(Value::Answer(Box::new(AnswerOf::Refused(self.refusal()?)))),
                other => Err(DecodeError::BadAnswer(other)),
            },
            tag::REFUSAL => Ok(Value::Refusal(self.refusal()?)),
            tag::NODE => Err(DecodeError::WrongShape),
            other => Err(DecodeError::UnknownTag(other)),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{DecodeError, MAX_DEPTH, decode};
    use crate::cairn::Cairn;
    use crate::value::{AnswerOf, MAX_REFUSAL, Refusal, Value, tag};

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
            Value::Refusal(Refusal::Absent),
            Value::Refusal(Refusal::Conflict),
            Value::Answer(Box::new(AnswerOf::Given(Value::Int(11_204)))),
            Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(b"main.nc".to_vec())))),
            Value::Answer(Box::new(AnswerOf::Refused(Refusal::Absent))),
            Value::Answer(Box::new(AnswerOf::Refused(Refusal::Unreachable))),
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

    /// Every refusal the specification names has a distinct encoding, and the
    /// order they encode in is the order §5.1.1 lists them.
    #[test]
    fn the_refusal_set_is_closed_and_ordered() {
        let all = [
            Refusal::Absent,
            Refusal::Denied,
            Refusal::Malformed,
            Refusal::Unreachable,
            Refusal::Exhausted,
            Refusal::Conflict,
        ];
        for (code, refusal) in all.iter().enumerate() {
            let code = u8::try_from(code).unwrap();
            assert_eq!(*refusal as u8, code, "{refusal:?} is not code {code}");
            assert_eq!(Refusal::from_code(code), Some(*refusal));
        }
        assert_eq!(MAX_REFUSAL, 5);
        assert_eq!(Refusal::from_code(6), None);
    }

    /// A refusal and the answer that carries it are different values.
    #[test]
    fn a_refusal_is_not_the_answer_that_holds_it() {
        let bare = Value::Refusal(Refusal::Absent);
        let held = Value::Answer(Box::new(AnswerOf::Refused(Refusal::Absent)));
        assert_ne!(bare.cairn(), held.cairn());
    }

    #[test]
    fn rejects_answers_that_are_neither_given_nor_refused() {
        for bad in [0x02, 0xff] {
            assert_eq!(decode(&[tag::ANSWER, bad]), Err(DecodeError::BadAnswer(bad)));
        }
    }

    #[test]
    fn rejects_refusals_outside_the_closed_set() {
        assert_eq!(decode(&[tag::REFUSAL, 6]), Err(DecodeError::NoSuchRefusal(6)));
        assert_eq!(decode(&[tag::ANSWER, 0x01, 9]), Err(DecodeError::NoSuchRefusal(9)));
    }

    #[test]
    fn rejects_unknown_tags() {
        assert_eq!(decode(&[0xff]), Err(DecodeError::UnknownTag(0xff)));
        assert_eq!(decode(&[0x09]), Err(DecodeError::UnknownTag(0x09)));
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

    /// 0202: one allocation. `encode` counts, reserves exactly that, then
    /// writes, and a `Vec` reallocates only when it would outgrow what it
    /// reserved, so an encoding that ends at its capacity never reallocated.
    #[test]
    fn every_value_is_encoded_into_exactly_what_it_reserved() {
        let large = Value::Array(vec![Value::Bytes(vec![7; 1 << 16]); 64]);
        let nested = (0..MAX_DEPTH - 1).fold(Value::Unit, |v, _| Value::Array(vec![v]));
        for value in sample().into_iter().chain([large, nested]) {
            let bytes = value.encode();
            assert_eq!(bytes.capacity(), bytes.len(), "{value:?} reallocated");
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
        assert!(value.validate().is_ok(), "validate must match decode at MAX_DEPTH");
        let encoded = value.try_encode().expect("try_encode must match decode at MAX_DEPTH");
        assert_eq!(decode(&encoded).unwrap(), value);
        assert!(super::scan_stored(&encoded).is_ok());

        let too_deep = Value::Array(vec![value]);
        assert_eq!(too_deep.validate(), Err(DecodeError::TooDeep));
        assert_eq!(too_deep.try_encode(), Err(DecodeError::TooDeep));
        let too_deep_bytes = too_deep.encode();
        assert_eq!(decode(&too_deep_bytes), Err(DecodeError::TooDeep));
        assert_eq!(super::scan_stored(&too_deep_bytes), Err(DecodeError::TooDeep));
    }

    #[test]
    fn decode_and_scan_enforce_decoded_weight_budget() {
        let val_sz = core::mem::size_of::<Value>();
        let count = super::MAX_DECODED_BYTES / val_sz + 1;
        let mut bytes = Vec::with_capacity(9 + count);
        bytes.push(tag::ARRAY);
        bytes.extend_from_slice(&(count as u64).to_be_bytes());
        bytes.resize(9 + count, tag::UNIT);
        assert!(matches!(decode(&bytes), Err(DecodeError::TooLarge { .. })));
        assert!(matches!(super::scan_stored(&bytes), Err(DecodeError::TooLarge { .. })));
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
            Node::Hole { call: call(), stratum: 3, span: span() },
            Node::Hole {
                call: Call { function: "draw".to_owned(), args: vec![] },
                stratum: 7,
                span: span(),
            },
            Node::Deposit { value: c(b"greeting"), span: span() },
            Node::Witness { stratum: 3, call: call(), answer: c(b"bytes"), span: span() },
            Node::Trace {
                residue: c(b"residue"),
                holes: vec![c(b"hole")],
                witnesses: vec![c(b"witness")],
                deposits: vec![c(b"obj")],
                source: c(b"build.nc"),
                depth: 3,
                unrecorded: false,
            },
            Node::Trace {
                residue: c(b"residue"),
                holes: vec![],
                witnesses: vec![],
                deposits: vec![],
                source: c(b"x.nc"),
                depth: 8,
                unrecorded: true,
            },
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

    /// 0202, for every node kind. See the value test of the same shape.
    #[test]
    fn every_node_is_encoded_into_exactly_what_it_reserved() {
        for node in sample() {
            let bytes = node.encode();
            assert_eq!(bytes.capacity(), bytes.len(), "{node:?} reallocated");
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
        let hole = Node::Hole { call: call(), stratum: 3, span: span() };
        assert_eq!(hole.references(), vec![c(b"main.nc"), c(b"hello.nc")]);
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
        // residue, no holes, no witnesses, no deposits, source, depth, then
        // the mark.
        let mut bytes = vec![tag::NODE, kind::TRACE];
        bytes.extend_from_slice(c(b"residue").as_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.extend_from_slice(&0u64.to_be_bytes());
        bytes.extend_from_slice(c(b"source").as_bytes());
        bytes.push(0);
        bytes.push(0x02);
        assert_eq!(decode_node(&bytes), Err(DecodeError::BadMark(0x02)));
    }

    /// A count of four billion cairns must not become a four-billion allocation.
    #[test]
    fn rejects_counts_the_input_cannot_carry() {
        let mut bytes = vec![tag::NODE, kind::TRACE];
        bytes.extend_from_slice(c(b"residue").as_bytes());
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
            bytes.extend_from_slice(c(b"first").as_bytes());
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

    #[test]
    fn model_fixtures_match_the_rust_encoder_and_excluded_in_memory_states_are_rejected() {
        use crate::{AnswerOf, Refusal};

        let mut c0_raw = [0u8; 32];
        for (i, slot) in c0_raw.iter_mut().enumerate() {
            *slot = u8::try_from(i).unwrap();
        }
        let c0 = Cairn::from_bytes(c0_raw);

        let fixtures: &[(Value, &[u8])] = &[
            (Value::Unit, b"\x00"),
            (Value::Bool(false), b"\x01\x00"),
            (Value::Bool(true), b"\x01\x01"),
            (Value::Int(0), b"\x02\x00\x00\x00\x00\x00\x00\x00\x00"),
            (Value::Int(-1), b"\x02\xff\xff\xff\xff\xff\xff\xff\xff"),
            (Value::Int(i64::MIN), b"\x02\x80\x00\x00\x00\x00\x00\x00\x00"),
            (Value::Int(i64::MAX), b"\x02\x7f\xff\xff\xff\xff\xff\xff\xff"),
            (Value::Bytes(b"ab".to_vec()), b"\x03\x00\x00\x00\x00\x00\x00\x00\x02ab"),
            (Value::Str("hi".into()), b"\x04\x00\x00\x00\x00\x00\x00\x00\x02hi"),
            (
                Value::Answer(Box::new(AnswerOf::Given(Value::Int(7)))),
                b"\x07\x00\x02\x00\x00\x00\x00\x00\x00\x00\x07",
            ),
            (
                Value::Answer(Box::new(AnswerOf::Refused(Refusal::Conflict))),
                b"\x07\x01\x05",
            ),
            (Value::Refusal(Refusal::Absent), b"\x08\x00"),
            (
                Value::Struct {
                    name: "P".into(),
                    fields: vec![Value::Unit, Value::Bool(true)],
                },
                b"\x10\x00\x00\x00\x00\x00\x00\x00\x01P\x00\x00\x00\x00\x00\x00\x00\x02\x00\x01\x01",
            ),
            (
                Value::Array(vec![Value::Unit, Value::Int(1)]),
                b"\x11\x00\x00\x00\x00\x00\x00\x00\x02\x00\x02\x00\x00\x00\x00\x00\x00\x00\x01",
            ),
        ];
        for (val, expected) in fixtures {
            assert_eq!(val.encode(), *expected);
            assert_eq!(&decode(expected).unwrap(), val);
        }

        let mut expected_cairn = vec![0x05];
        expected_cairn.extend_from_slice(&c0_raw);
        assert_eq!(Value::Cairn(c0).encode(), expected_cairn);

        let mut expected_shade = vec![0x06, 0x03];
        expected_shade.extend_from_slice(&c0_raw);
        assert_eq!(Value::Shade { origin: 3, value: c0 }.encode(), expected_shade);

        // Invalid in-memory states excluded by §7.1.1 / §7.3.1 (pending 0115):
        assert_eq!(
            decode(&Value::Shade { origin: 9, value: c0 }.encode()),
            Err(DecodeError::NoSuchStratum(9))
        );
        assert_eq!(
            decode(&Value::Struct { name: String::new(), fields: vec![] }.encode()),
            Err(DecodeError::EmptyStructName)
        );
        assert_eq!(
            decode_node(
                &Node::Hole {
                    call: Call { function: String::new(), args: vec![] },
                    stratum: 3,
                    span: span(),
                }
                .encode()
            ),
            Err(DecodeError::EmptyCallName)
        );
        assert_eq!(
            decode_node(
                &Node::Deposit { value: c0, span: Span { source: c0, start: 5, end: 4 } }.encode()
            ),
            Err(DecodeError::BackwardsSpan { start: 5, end: 4 })
        );
    }

    #[test]
    fn model_node_fixtures_match_the_rust_encoder() {
        let c0_raw: [u8; 32] = core::array::from_fn(|i| u8::try_from(i).unwrap());
        let c1_raw: [u8; 32] = core::array::from_fn(|i| u8::try_from(i + 32).unwrap());
        let c2_raw: [u8; 32] = core::array::from_fn(|i| u8::try_from(i + 64).unwrap());
        let (c0, c1, c2) =
            (Cairn::from_bytes(c0_raw), Cairn::from_bytes(c1_raw), Cairn::from_bytes(c2_raw));
        let mut expected_apply = vec![0x20, 0x01];
        expected_apply.extend_from_slice(&c0_raw);
        expected_apply.extend_from_slice(&1u64.to_be_bytes());
        expected_apply.extend_from_slice(&c1_raw);
        expected_apply.extend_from_slice(&c2_raw);
        let mut expected_hole = vec![0x20, 0x02];
        expected_hole.extend_from_slice(&4u64.to_be_bytes());
        expected_hole.extend_from_slice(b"read");
        expected_hole.extend_from_slice(&1u64.to_be_bytes());
        expected_hole.extend_from_slice(&c1_raw);
        expected_hole.push(0x03);
        expected_hole.extend_from_slice(&c0_raw);
        expected_hole.extend_from_slice(&1u64.to_be_bytes());
        expected_hole.extend_from_slice(&5u64.to_be_bytes());
        let mut expected_deposit = vec![0x20, 0x03];
        expected_deposit.extend_from_slice(&c1_raw);
        expected_deposit.extend_from_slice(&c0_raw);
        expected_deposit.extend_from_slice(&0u64.to_be_bytes());
        expected_deposit.extend_from_slice(&4u64.to_be_bytes());
        let mut expected_witness = vec![0x20, 0x04, 0x03];
        expected_witness.extend_from_slice(&4u64.to_be_bytes());
        expected_witness.extend_from_slice(b"read");
        expected_witness.extend_from_slice(&1u64.to_be_bytes());
        expected_witness.extend_from_slice(&c1_raw);
        expected_witness.extend_from_slice(&c2_raw);
        expected_witness.extend_from_slice(&c0_raw);
        expected_witness.extend_from_slice(&1u64.to_be_bytes());
        expected_witness.extend_from_slice(&5u64.to_be_bytes());
        let mut expected_trace = vec![0x20, 0x05];
        expected_trace.extend_from_slice(&c0_raw);
        expected_trace.extend_from_slice(&1u64.to_be_bytes());
        expected_trace.extend_from_slice(&c1_raw);
        expected_trace.extend_from_slice(&0u64.to_be_bytes());
        expected_trace.extend_from_slice(&1u64.to_be_bytes());
        expected_trace.extend_from_slice(&c2_raw);
        expected_trace.extend_from_slice(&c0_raw);
        expected_trace.extend_from_slice(&[0x03, 0x00]);
        let node_fixtures = [
            (
                Node::Literal(Value::Int(7)),
                b"\x20\x00\x02\x00\x00\x00\x00\x00\x00\x00\x07".to_vec(),
            ),
            (Node::Apply { function: c0, args: vec![c1], result: c2 }, expected_apply),
            (
                Node::Hole {
                    call: Call { function: "read".to_owned(), args: vec![c1] },
                    stratum: 3,
                    span: Span { source: c0, start: 1, end: 5 },
                },
                expected_hole,
            ),
            (
                Node::Deposit { value: c1, span: Span { source: c0, start: 0, end: 4 } },
                expected_deposit,
            ),
            (
                Node::Witness {
                    stratum: 3,
                    call: Call { function: "read".to_owned(), args: vec![c1] },
                    answer: c2,
                    span: Span { source: c0, start: 1, end: 5 },
                },
                expected_witness,
            ),
            (
                Node::Trace {
                    residue: c0,
                    holes: vec![c1],
                    witnesses: vec![],
                    deposits: vec![c2],
                    source: c0,
                    depth: 3,
                    unrecorded: false,
                },
                expected_trace,
            ),
        ];
        for (node, expected) in node_fixtures {
            assert_eq!(node.encode(), expected);
            assert_eq!(decode_node(&expected).unwrap(), node);
        }
    }
}
