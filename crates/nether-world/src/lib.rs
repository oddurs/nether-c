//! The world, and the one rule about touching it.
//!
//! `spec/01-strata.md` §1.4: when an expression deeper than 0 is evaluated, the
//! implementation MUST write the witness to the ledger **before** the value is
//! returned to the program. "The order matters. An answer that is returned
//! before it is recorded is an answer that can be lost, and a ledger with a gap
//! in it cannot support any of the guarantees in section 06."
//!
//! That order is not left to whoever writes the next provider. A [`Provider`]
//! answers by returning a [`Recorded`], and the only way to make one of those
//! is [`Recorder::record`], which writes before it returns. A provider that
//! forgot to record has nothing to return and does not compile.

mod disk;
mod provider;
mod recorder;
mod replay;

pub use disk::Disk;
pub use provider::{Provider, Unanswered, World};
pub use recorder::{Recorded, Recorder};
pub use replay::Replay;
