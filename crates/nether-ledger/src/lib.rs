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
mod value;

pub use cairn::Cairn;
pub use codec::{DecodeError, MAX_DEPTH, decode};
pub use value::{MAX_STRATUM, Value};
