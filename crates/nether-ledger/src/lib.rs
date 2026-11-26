//! The ledger.
//!
//! A content-addressed store of immutable values. Every value has exactly one
//! encoding, that encoding has exactly one name, and nothing that has been
//! written is ever revised.
//!
//! The format is frozen in `spec/07-ledger.md`. This crate implements it and
//! is not permitted to disagree with it: where the two differ, the
//! specification is right and this is a bug.

mod cairn;
mod codec;
mod node;
mod store;
mod value;

pub use cairn::{Cairn, ParseCairnError, SHORT_LEN};
pub use codec::{
    DecodeError, MAX_DECODED_BYTES, MAX_DEPTH, Stored, decode, decode_node, decode_stored,
};
pub use node::{Call, Node, Span};
pub use store::{
    AffectedTrace, AttemptGuard, Audit, IndexError, MAX_OBJECT_BYTES, PACKAGE_DOMAIN, Package,
    PackageError, ResolvedPackage, Store, StoreError,
};
pub use value::{
    AnswerOf, EmptyNameError, MAX_REFUSAL, MAX_STRATUM, NonEmptyName, Refusal, Stratum, Value,
};
