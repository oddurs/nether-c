//! The values a ledger holds.
//!
//! One variant per tag in `spec/07-ledger.md` §7.1. Tag `0x20`, `Node`, is not
//! here: a node is a thing the ledger stores *about* evaluation rather than a
//! value a program can hold, and it lands with the node model.

use crate::cairn::Cairn;

/// Tag bytes, fixed by `spec/07-ledger.md` §7.1 and not ours to change.
pub(crate) mod tag {
    pub const UNIT: u8 = 0x00;
    pub const BOOL: u8 = 0x01;
    pub const INT: u8 = 0x02;
    pub const BYTES: u8 = 0x03;
    pub const STR: u8 = 0x04;
    pub const CAIRN: u8 = 0x05;
    pub const SHADE: u8 = 0x06;
    pub const STRUCT: u8 = 0x10;
    pub const ARRAY: u8 = 0x11;
    pub const NODE: u8 = 0x20;
}

/// The deepest stratum a shade may claim to have come from.
pub const MAX_STRATUM: u8 = 8;

/// A value, as the ledger sees it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Value {
    /// `U0`. Exactly one inhabitant.
    Unit,
    /// `Bool`.
    Bool(bool),
    /// `I64`. The only integer type the language has.
    Int(i64),
    /// `Bytes`. Any finite byte string.
    Bytes(Vec<u8>),
    /// `Str`. Well-formed UTF-8, stored exactly as given.
    ///
    /// The ledger does not normalise. Identifiers are normalised in the lexer,
    /// where the ambiguity actually matters; see `spec/03-lexical.md` §3.3.
    Str(String),
    /// A content address, held as a value.
    Cairn(Cairn),
    /// A value carried up out of a deeper stratum, opaque.
    ///
    /// Only the origin and the name survive: a shade may be stored, sealed and
    /// compared, and its contents may not be reached without descending again.
    Shade {
        /// The stratum it came from, `0..=8`.
        origin: u8,
        /// The name of the value inside.
        value: Cairn,
    },
    /// A nominal struct. The name is part of the value.
    Struct {
        /// The type's name. Never empty.
        name: String,
        /// Fields, in declaration order.
        fields: Vec<Value>,
    },
    /// A sequence.
    Array(Vec<Value>),
}

impl Value {
    /// The tag byte this value encodes under.
    #[must_use]
    pub(crate) const fn tag(&self) -> u8 {
        match self {
            Self::Unit => tag::UNIT,
            Self::Bool(_) => tag::BOOL,
            Self::Int(_) => tag::INT,
            Self::Bytes(_) => tag::BYTES,
            Self::Str(_) => tag::STR,
            Self::Cairn(_) => tag::CAIRN,
            Self::Shade { .. } => tag::SHADE,
            Self::Struct { .. } => tag::STRUCT,
            Self::Array(_) => tag::ARRAY,
        }
    }

    /// The name of this value.
    ///
    /// Equality of cairns is equality of values, which is what makes deep
    /// comparison a thirty-two byte operation.
    #[must_use]
    pub fn cairn(&self) -> Cairn {
        Cairn::of_encoded(&self.encode())
    }
}
