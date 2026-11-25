//! Stratum 1: reading the ledger.
//!
//! `spec/09-prelude.md` §9.3. One stratum down rather than pure because the
//! ledger is a place on a machine and a program that reads one can fail in a
//! way a pure program cannot — and only one, because what comes back is fully
//! determined by the cairn that asked for it.
//!
//! This provider holds no store of its own. The [`Recorder`] carries the one
//! the answer is being written to, and a `store` capability that could read a
//! ledger it does not write to would be two ledgers wearing one name.

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, StoreError, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// The ledger, read by cairn.
pub struct Ledger;

impl Provider for Ledger {
    fn capability(&self) -> Capability {
        Capability::Store
    }

    fn answers(&self, function: &str) -> bool {
        matches!(function, "fetch_node" | "has_node")
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        // §6.3: a world-call whose argument is not finished cannot have become
        // a hole, and §04 types this one as a `Cairn`, so getting here means a
        // program the checker should have rejected.
        let Some(name) = into.arg_cairn(call, 0)? else {
            if call.function == "has_node" {
                return Err(Refuse::Collapse("`has_node` was not given a cairn".into()));
            }
            return into.record(call, Depth::STORE, span, &refused(Refusal::Malformed));
        };
        let said = match call.function.as_str() {
            // §7.2: the encoding is the node. Handing back the bytes rather
            // than a decoded value is what makes `fetch_node` the inverse of
            // `seal` — and `Store::get` has already checked that they hash to
            // the name that asked for them. Only `Absent` is a valid §9.3
            // refusal; corruption and I/O errors abort via `NotRecorded`.
            "fetch_node" => match into.store().get(name) {
                Ok(stored) => given(&call.function, Value::Bytes(stored.encode())),
                Err(StoreError::Absent(_)) => refused(Refusal::Absent),
                Err(e) => return Err(Refuse::NotRecorded(e)),
            },
            _ => given(&call.function, Value::Bool(into.store().has(name))),
        };
        into.record(call, Depth::STORE, span, &said)
    }
}
