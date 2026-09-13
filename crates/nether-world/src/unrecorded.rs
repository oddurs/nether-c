//! Stratum 8: the Unrecorded.
//!
//! `spec/09-prelude.md` §9.8, §9.8.1 for the other side of the call, and
//! `spec/01-strata.md` §1.7 for what it costs. The deepest stratum, and the
//! only one that breaks the language's promise rather than deepening it: a
//! trace that reached here has a gap in it and no later care fills it in.
//!
//! Everything `unsafe` is in `nether-foreign`. What is here is the part that
//! can be written without any: which object answers, what the return value
//! meant, and the one ordering that matters.

use std::path::Path;

use nether_core::{Capability, Depth};
use nether_foreign::Object;
use nether_ledger::{Call, Refusal, Span, Stored, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// The objects §8.3.3 said may be loaded, in the order it said them.
pub struct Unrecorded {
    loaded: Vec<Object>,
}

impl Unrecorded {
    /// Open every declared object, or say which one would not open.
    ///
    /// At the grant, not at the call. A burial that finds out halfway through
    /// that an object is missing has already done half its work at stratum 8,
    /// and there is no undoing that half.
    ///
    /// # Errors
    ///
    /// The path that would not open, and what the loader said about it.
    pub fn loading(paths: &[String]) -> Result<Self, String> {
        let mut loaded = Vec::with_capacity(paths.len());
        for path in paths {
            match Object::open(Path::new(path)) {
                Ok(object) => loaded.push(object),
                Err(why) => return Err(format!("{path}: {why}")),
            }
        }
        Ok(Self { loaded })
    }

    /// The first declared object that has that symbol. §8.3.3: order is the
    /// invocation's, because two objects defining one symbol is a question
    /// only the person running the rite can settle.
    fn who_has(&self, symbol: &str) -> Option<&Object> {
        self.loaded.iter().find(|o| o.has(symbol))
    }
}

/// The two `Str`, `Bytes` arguments §9.8 gives `call_foreign`.
fn asked(into: &Recorder, call: &Call) -> Option<(String, Vec<u8>)> {
    let Ok(Stored::Value(Value::Str(symbol))) = into.store().get(*call.args.first()?) else {
        return None;
    };
    let Ok(Stored::Value(Value::Bytes(args))) = into.store().get(*call.args.get(1)?) else {
        return None;
    };
    Some((symbol, args))
}

/// §9.8.1's return table. `0` is the answer, `1` never reaches here, and
/// `2..=7` are §5.1.1's six in the order it lists them.
const fn meant(code: i32) -> Option<Refusal> {
    match code {
        2 => Some(Refusal::Absent),
        3 => Some(Refusal::Denied),
        4 => Some(Refusal::Malformed),
        5 => Some(Refusal::Unreachable),
        6 => Some(Refusal::Exhausted),
        7 => Some(Refusal::Conflict),
        _ => None,
    }
}

impl Provider for Unrecorded {
    fn capability(&self) -> Capability {
        Capability::Unrecorded
    }

    fn answers(&self, function: &str) -> bool {
        function == "call_foreign"
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        let Some((symbol, args)) = asked(into, call) else {
            return into.record(call, Depth::UNRECORDED, span, &refused(Refusal::Malformed));
        };
        // §8.3.3: a symbol found in nothing named is denied. Before the call,
        // like every other reach in this crate.
        let Some(object) = self.who_has(&symbol) else {
            return into.record(call, Depth::UNRECORDED, span, &refused(Refusal::Denied));
        };

        // §9.8.1: the witness goes down before the call, so a callee that does
        // not return still leaves a trace saying what was attempted. It is the
        // only guarantee this stratum has, and it is the reverse of every
        // other provider, which records what came back.
        into.record(call, Depth::UNRECORDED, span, &refused(Refusal::Unreachable))?;
        let (code, said) = match object.call(&symbol, &args) {
            Ok(answered) => answered,
            // The symbol was there a moment ago and the call did not happen.
            // The attempt is already written, which is the whole point of
            // having written it.
            Err(why) => return Err(Refuse::Unavailable(why.to_string())),
        };

        let value = match code {
            0 => given(&call.function, Value::Bytes(said)),
            other => refused(meant(other).unwrap_or(Refusal::Malformed)),
        };
        into.record(call, Depth::UNRECORDED, span, &value)
    }
}
