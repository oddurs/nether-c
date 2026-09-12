//! Burial.
//!
//! The only form of evaluation Nether C has, and the place the two halves of
//! the implementation meet: a unit is a fact about a source file, a cairn is a
//! name in the ledger, and burial is what turns the first into the second.
//!
//! `spec/06-evaluation.md`.

mod bury;

pub use bury::{Grinding, Halt, HaltKind, MAX_FRAMES, Residue, STACK, bury};
