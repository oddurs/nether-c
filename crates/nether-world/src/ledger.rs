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
                Ok(stored) => {
                    let encoded = stored.encode();
                    // `Answer::Given(Value::Bytes(..))` adds 11 bytes of framing
                    // (`tag::ANSWER`, `0x00`, `tag::BYTES`, 8-byte length).
                    if encoded.len().saturating_add(11) > nether_ledger::MAX_OBJECT_BYTES {
                        refused(Refusal::Exhausted)
                    } else {
                        given(&call.function, Value::Bytes(encoded))
                    }
                }
                Err(StoreError::Absent(_)) => refused(Refusal::Absent),
                Err(e) => return Err(Refuse::NotRecorded(e)),
            },
            _ => given(&call.function, Value::Bool(into.store().has(name))),
        };
        into.record(call, Depth::STORE, span, &said)
    }
}

#[cfg(test)]
mod tests {
    use super::Ledger;
    use crate::{Recorder, World};
    use nether_ledger::{AnswerOf, Call, MAX_OBJECT_BYTES, Refusal, Span, Store, Stored, Value};
    use std::sync::atomic::{AtomicU64, Ordering};

    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let dir = std::env::temp_dir().join(format!(
                "nether-ledger-fetch-bound-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            let _ = std::fs::remove_dir_all(&dir);
            Self(dir)
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn fetch_node_within_eleven_bytes_of_limit_refuses_exhausted() {
        let dir = Scratch::new();
        let store = Store::open(&dir.0).unwrap();
        // `Value::Bytes(b)` encodes as 1 + 8 + b.len(); setting b.len() = MAX_OBJECT_BYTES - 9
        // produces an encoded object of exact size MAX_OBJECT_BYTES, so wrapping it in
        // `Answer::Given(Value::Bytes(..))` (+11 bytes) would exceed MAX_OBJECT_BYTES.
        let big = Stored::Value(Value::Bytes(vec![0u8; MAX_OBJECT_BYTES - 9]));
        let cairn = store.put(&big).expect("fits at exact MAX_OBJECT_BYTES");
        let arg = store.put(&Stored::Value(Value::Cairn(cairn))).unwrap();
        let src = store.put(&Stored::Value(Value::Bytes(b"x.nc".to_vec()))).unwrap();
        let call = Call { function: "fetch_node".into(), args: vec![arg] };
        let span = Span { source: src, start: 0, end: 1 };
        let world = World::sealed().granting(Box::new(Ledger));
        let rec = world
            .ask(&call, span, &Recorder::new(&store))
            .expect("must record Refusal::Exhausted instead of aborting NotRecorded");
        assert_eq!(
            store.get(rec.answer()).unwrap(),
            Stored::Value(Value::Answer(Box::new(AnswerOf::Refused(Refusal::Exhausted))))
        );
    }
}
