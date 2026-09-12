//! Types.
//!
//! `spec/05-types.md`. Depth is deliberately absent: `τ@d` is a type paired
//! with a depth, not a type of its own (§5.5), and the pair is
//! [`Expr`](crate::Expr), which carries both. Stating the depth in two places
//! would eventually state it two different ways.

use core::fmt;

use crate::depth::Depth;

/// A type.
///
/// The prelude's type names are ordinary identifiers in source
/// (`spec/03-lexical.md` §3.4) and concrete variants here, because the IR is
/// what a name resolves *to*. A `typedef` is an alias and does not survive
/// lowering: it is replaced by the type it names.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Type {
    /// `U0`. Exactly one inhabitant.
    Unit,
    /// `Bool`.
    Bool,
    /// `I64`, the only integer type the language has.
    Int,
    /// `Bytes`.
    Bytes,
    /// `Str`. Well-formed UTF-8.
    Str,
    /// `Cairn`. A content address, held as a value.
    Cairn,
    /// `Shadeᵈ⟨T⟩`. Opaque, and a distinct type rather than a depth on `T`,
    /// because a shade's whole purpose is to be something you cannot use as a
    /// `T`. `spec/01-strata.md` §1.6.
    Shade {
        /// The stratum the value inside came from. Inferred, never written:
        /// writing it would let a program claim an origin it does not have.
        origin: Depth,
        /// The type of the value inside.
        inner: Box<Type>,
    },
    /// `Answer⟨T⟩`: what the world said, which it is entitled to say no to.
    Answer(Box<Type>),
    /// `Refusal`: one of six codes, and nothing else.
    Refusal,
    /// A struct, by name. Structs are nominal: two structs with identical
    /// fields are different types, so the name is the identity.
    Struct(String),
    /// A sequence. Fixed-length when the length is written, a slice when not.
    Array {
        /// The element type.
        elem: Box<Type>,
        /// The length, when it was written.
        len: Option<u64>,
    },
    /// `τ₁ --dƒ--> τ₂@d_r`. Two depths, because they are two facts: what a
    /// caller must already hold, and how deep what comes back is. A function
    /// that descends for itself asks for nothing and still hands back
    /// something deep. `spec/02-calculus.md` §2.1.
    Fn {
        /// The parameter types, in order.
        params: Vec<Type>,
        /// `dƒ`: what a caller must already hold to apply it. [APP]'s premise.
        latent: Depth,
        /// What applying it produces.
        result: Box<Type>,
        /// `d_r`: the depth of what comes back. [APP] joins this, not `dƒ`.
        result_depth: Depth,
    },
}

impl fmt::Display for Type {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unit => f.write_str("U0"),
            Self::Bool => f.write_str("Bool"),
            Self::Int => f.write_str("I64"),
            Self::Bytes => f.write_str("Bytes"),
            Self::Str => f.write_str("Str"),
            Self::Cairn => f.write_str("Cairn"),
            Self::Shade { inner, .. } => write!(f, "Shade<{inner}>"),
            Self::Answer(inner) => write!(f, "Answer<{inner}>"),
            Self::Refusal => f.write_str("Refusal"),
            Self::Struct(name) => f.write_str(name),
            Self::Array { elem, len: Some(n) } => write!(f, "{elem}[{n}]"),
            Self::Array { elem, len: None } => write!(f, "{elem}[]"),
            Self::Fn { params, latent, result, result_depth } => {
                write!(f, "{result}@{result_depth}(")?;
                for (i, p) in params.iter().enumerate() {
                    if i > 0 {
                        f.write_str(", ")?;
                    }
                    write!(f, "{p}")?;
                }
                write!(f, ") @{latent}")
            }
        }
    }
}

/// Which no the world said.
///
/// The set is **closed** and fixed by `spec/05-types.md` §5.1.1. A refusal
/// carries a code and nothing else: no message, no platform error number, no
/// path, because a value whose encoding varies between operating systems
/// cannot have a stable cairn. The detail is not lost — it is in the witness,
/// where a person can read it and no program can branch on it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Refusal {
    /// The thing is not there.
    Absent,
    /// It is there and you may not have it.
    Denied,
    /// It is there and it is not what it claims to be.
    Malformed,
    /// Nothing answered.
    Unreachable,
    /// A limit was reached: space, quota, size.
    Exhausted,
    /// Something else changed it first.
    Conflict,
}

impl Refusal {
    /// Every refusal there is, and every one there will be.
    pub const ALL: [Self; 6] = [
        Self::Absent,
        Self::Denied,
        Self::Malformed,
        Self::Unreachable,
        Self::Exhausted,
        Self::Conflict,
    ];

    /// The name it is bound to in the prelude scope.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Absent => "absent",
            Self::Denied => "denied",
            Self::Malformed => "malformed",
            Self::Unreachable => "unreachable",
            Self::Exhausted => "exhausted",
            Self::Conflict => "conflict",
        }
    }

    /// The refusal of that name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|r| r.name() == name)
    }
}

impl fmt::Display for Refusal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refusal_names_round_trip() {
        for r in Refusal::ALL {
            assert_eq!(Refusal::from_name(r.name()), Some(r));
        }
        assert_eq!(Refusal::from_name("timeout"), None);
    }

    #[test]
    fn a_shade_does_not_print_where_it_came_from() {
        // The origin is part of the type and is not part of the syntax, so it
        // is not part of how a type is written back to a person either.
        let s = Type::Shade { origin: Depth::NET, inner: Box::new(Type::Struct("Json".into())) };
        assert_eq!(s.to_string(), "Shade<Json>");
    }

    #[test]
    fn signatures_print_the_way_they_are_written() {
        let read = Type::Fn {
            params: vec![Type::Str],
            latent: Depth::DISK,
            result: Box::new(Type::Answer(Box::new(Type::Bytes))),
            result_depth: Depth::DISK,
        };
        assert_eq!(read.to_string(), "Answer<Bytes>@3(Str) @3");
    }

    #[test]
    fn a_function_that_descends_for_itself_asks_for_nothing() {
        let load = Type::Fn {
            params: vec![Type::Str],
            latent: Depth::PURE,
            result: Box::new(Type::Bytes),
            result_depth: Depth::DISK,
        };
        assert_eq!(load.to_string(), "Bytes@3(Str) @0");
    }

    #[test]
    fn arrays_print_fixed_and_slice_apart() {
        let row = Type::Array { elem: Box::new(Type::Int), len: Some(16) };
        let parts = Type::Array { elem: Box::new(Type::Str), len: None };
        assert_eq!(row.to_string(), "I64[16]");
        assert_eq!(parts.to_string(), "Str[]");
    }
}
