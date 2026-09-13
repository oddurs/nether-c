//! Stratum 7: entropy.
//!
//! `spec/09-prelude.md` §9.7. `draw` is the only source of nondeterminism in
//! the language, and it is nearly the deepest thing in it: the one act after
//! which a program can never be re-derived, only replayed.
//!
//! What makes that survivable is §1.4. The bytes are written to the ledger
//! before the program sees them, so the witness holds the draw and §6.7 serves
//! it back exactly. Burying the same source again gives a different trace;
//! replaying a trace that drew gives the same one.
//!
//! The source is the operating system's. Nothing here mixes, stretches or
//! seeds anything — that would be writing cryptography, and this repository's
//! one dependency exception says not to.

use std::fs::File;
use std::io::Read;

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Span, Stored, Value};

use crate::provider::{Provider, Refuse, given};
use crate::recorder::{Recorded, Recorder};

/// Where the bytes come from.
///
/// A path rather than a syscall, because `std` has no way to ask for random
/// bytes and a hand-written `getrandom` binding is foreign code at stratum 8
/// to save one `open`.
const SOURCE: &str = "/dev/urandom";

/// The most one `draw` may ask for.
///
/// A limit of the implementation and not of the language, stated here because
/// §6.4 requires an implementation to state its limits rather than discover
/// them. A draw is recorded whole, and a witness nothing can hold is a witness.
pub const MOST: i64 = 16 * 1024 * 1024;

/// The Oracle, buried.
pub struct Entropy;

impl Entropy {
    /// A provider, if this machine has a source to draw from.
    ///
    /// Asked once, when the capability is granted, rather than once per draw.
    /// A machine with nothing to draw from cannot grant `entropy` at all, and
    /// finding that out at the grant is better than finding it out halfway
    /// through a burial.
    ///
    /// # Errors
    ///
    /// The reason the source could not be opened.
    pub fn opening() -> Result<Self, std::io::Error> {
        File::open(SOURCE).map(|_| Self)
    }
}

/// How many bytes the call asked for, if it asked for a number at all.
fn many(into: &Recorder, call: &Call) -> Option<i64> {
    match into.store().get(*call.args.first()?) {
        Ok(Stored::Value(Value::Int(n))) => Some(n),
        _ => None,
    }
}

impl Provider for Entropy {
    fn capability(&self) -> Capability {
        Capability::Entropy
    }

    fn answers(&self, function: &str) -> bool {
        function == "draw"
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        // §9.7 types `draw` as `Bytes` and not `Answer<Bytes>`: there is no no
        // to say. So every way of asking wrongly is §9.9's other failure.
        let Some(n) = many(into, call) else {
            return Err(Refuse::Collapse("`draw` was not asked for a number of bytes".into()));
        };
        if n < 0 {
            return Err(Refuse::Collapse(format!("`draw({n})` asks for fewer than no bytes")));
        }
        if n > MOST {
            return Err(Refuse::Collapse(format!(
                "`draw({n})` is past this implementation's limit of {MOST} bytes, \
                 which is a limit of the implementation and not of the language"
            )));
        }
        let Ok(want) = usize::try_from(n) else {
            return Err(Refuse::Collapse(format!("`draw({n})` is more than this machine holds")));
        };

        let mut drawn = vec![0u8; want];
        if let Err(e) = File::open(SOURCE).and_then(|mut f| f.read_exact(&mut drawn)) {
            // Not a refusal — §9.7 gives `draw` no way to say no — and not the
            // program's mistake either. The machine promised a source at the
            // grant and does not have one now.
            return Err(Refuse::Unavailable(format!("{SOURCE}: {e}")));
        }
        into.record(call, Depth::ENTROPY, span, &given(&call.function, Value::Bytes(drawn)))
    }
}
