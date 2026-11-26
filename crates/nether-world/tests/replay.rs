//! §6.7: replay does not prefer the ledger over the world, it cannot reach it.
//!
//! The claim is about what a `Replay` *is*. It holds a map from a question to
//! what was said and nothing else — no `World`, no `Provider`, no path, no
//! socket — and there is no method that takes one. The compiler checks the
//! last part: see `Replay`'s `compile_fail` example.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nether_core::Depth;
use nether_ledger::{Call, Node, Span, Store, StoreError, Stored, Stratum, Value};
use nether_world::{Disk, MergeError, Recorder, Replay, World};

struct Scratch(PathBuf);

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn asking(function: &str) -> Call {
    Call::try_new(function, Vec::new()).unwrap()
}

fn temp_dir(tag: &str) -> Scratch {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nether-replay-{tag}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("temp dir");
    Scratch(dir)
}

#[test]
fn a_replay_answers_what_was_recorded() {
    let said = Value::Bytes(b"int main(void) { return 0; }".to_vec());
    let replay = Replay::of(&[(asking("read"), said.clone())]);
    assert_eq!(replay.answer(&asking("read")), Some(&said));
    assert_eq!(replay.len(), 1);
}

#[test]
fn a_replay_that_was_told_nothing_answers_nothing() {
    // The whole guarantee in one line: there is nowhere else to look. §6.7
    // requires failing rather than reaching the world for a missing answer,
    // and this cannot do anything else.
    let replay = Replay::of(&[]);
    assert!(replay.is_empty());
    assert_eq!(replay.answer(&asking("read")), None);
    assert_eq!(replay.answer(&asking("get")), None);
}

#[test]
fn a_replay_answers_the_question_and_not_one_like_it() {
    // §6.3 makes a hole's identity its call, arguments included. An answer to
    // `read("a.nc")` is not an answer to `read("b.nc")`.
    let a = Call::try_new("read", vec![nether_ledger::Cairn::of_encoded(b"a")]).unwrap();
    let b = Call::try_new("read", vec![nether_ledger::Cairn::of_encoded(b"b")]).unwrap();
    let replay = Replay::of(&[(a.clone(), Value::Int(1))]);
    assert_eq!(replay.answer(&a), Some(&Value::Int(1)));
    assert_eq!(replay.answer(&b), None);
}

#[test]
fn two_workers_answer_assigned_slots_and_merge_in_slot_order_under_retries() {
    let root = temp_dir("merge-workers");
    std::fs::write(root.join("a.txt"), b"alpha").unwrap();
    std::fs::write(root.join("b.txt"), b"bravo").unwrap();
    std::fs::write(root.join("c.txt"), b"charlie").unwrap();
    std::fs::write(root.join("d.txt"), b"delta").unwrap();

    let origin = Store::open(root.join("origin")).unwrap();
    let src = origin.put(&Stored::Value(Value::Bytes(b"main.nc".to_vec()))).unwrap();
    let mut holes = Vec::new();
    for (idx, name) in ["a.txt", "b.txt", "c.txt", "d.txt"].into_iter().enumerate() {
        let arg = origin.put(&Stored::Value(Value::Str(name.into()))).unwrap();
        let hole = origin
            .put(&Stored::Node(Node::Hole {
                call: Call::try_new("read", vec![arg]).unwrap(),
                stratum: Stratum::THREE,
                span: Span::new(src, idx as u64, idx as u64 + 1).unwrap(),
            }))
            .unwrap();
        holes.push(hole);
    }

    let mut base_bundle = Vec::new();
    origin.export(&mut base_bundle).unwrap();

    // Serial baseline: one machine answers all four slots in order.
    let serial_store = Store::open(root.join("serial")).unwrap();
    serial_store.import(&mut &base_bundle[..]).unwrap();
    let world = World::sealed().granting(Box::new(Disk::reading(&root)));
    let serial_deliv =
        world.ask_slots(&holes, &[0, 1, 2, 3], &Recorder::new(&serial_store)).unwrap();
    let (serial_witnesses, serial_replay) =
        Replay::merge(&serial_store, &[], &holes, &[&serial_deliv]).unwrap();
    assert_eq!(serial_witnesses.len(), 4);

    // Two isolated workers each answer half the trace's holes.
    let worker_a = Store::open(root.join("worker-a")).unwrap();
    let worker_b = Store::open(root.join("worker-b")).unwrap();
    worker_a.import(&mut &base_bundle[..]).unwrap();
    worker_b.import(&mut &base_bundle[..]).unwrap();

    let deliv_a = world.ask_slots(&holes, &[2, 0], &Recorder::new(&worker_a)).unwrap();
    let deliv_b = world.ask_slots(&holes, &[3, 1], &Recorder::new(&worker_b)).unwrap();
    assert_eq!(deliv_a.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![0, 2]);
    assert_eq!(deliv_b.iter().map(|(s, _)| *s).collect::<Vec<_>>(), vec![1, 3]);

    let mut bundle_a = Vec::new();
    let mut bundle_b = Vec::new();
    worker_a.export(&mut bundle_a).unwrap();
    worker_b.export(&mut bundle_b).unwrap();

    let coord = Store::open(root.join("coord")).unwrap();
    coord.import(&mut &bundle_a[..]).unwrap();
    // Before worker B's bundle arrives (or after a truncated transfer),
    // merging fails on the absent witness rather than skipping slots 1 and 3.
    assert!(matches!(
        coord.import(&mut &bundle_b[..bundle_b.len() / 2]),
        Err(StoreError::Bundle(_))
    ));
    let missing_b = Replay::merge(&coord, &[], &holes, &[&deliv_a, &deliv_b]);
    assert!(matches!(missing_b, Err(MergeError::Store(StoreError::Absent(_)))));

    // Completing the transfer and merging in any order (including duplicate
    // retry deliveries) produces the exact serial witness list.
    coord.import(&mut &bundle_b[..]).unwrap();
    for deliveries in [
        vec![&deliv_a[..], &deliv_b[..]],
        vec![&deliv_b[..], &deliv_a[..]],
        vec![&deliv_b[..], &deliv_a[..], &deliv_b[..], &deliv_a[..]],
    ] {
        let (merged_witnesses, merged_replay) =
            Replay::merge(&coord, &[], &holes, &deliveries).unwrap();
        assert_eq!(merged_witnesses, serial_witnesses);
        assert_eq!(merged_replay.len(), serial_replay.len());
        for (call, val) in serial_replay.all() {
            assert_eq!(merged_replay.answer(call), Some(val));
        }
    }
}

#[test]
fn conflicting_and_mismatched_worker_witnesses_are_rejected() {
    let root = temp_dir("merge-conflicts");
    let store = Store::open(root.join("store")).unwrap();
    let src = store.put(&Stored::Value(Value::Bytes(b"main.nc".to_vec()))).unwrap();
    let arg = store.put(&Stored::Value(Value::Str("http://127.0.0.1/in".into()))).unwrap();
    let body = store.put(&Stored::Value(Value::Bytes(b"payload".to_vec()))).unwrap();
    let call = Call::try_new("post", vec![arg, body]).unwrap();
    let span0 = Span::new(src, 0, 10).unwrap();
    let span1 = Span::new(src, 12, 22).unwrap();

    let h0 = store
        .put(&Stored::Node(Node::Hole { call: call.clone(), stratum: Stratum::SIX, span: span0 }))
        .unwrap();
    let h1 = store
        .put(&Stored::Node(Node::Hole { call: call.clone(), stratum: Stratum::SIX, span: span1 }))
        .unwrap();
    let holes = vec![h0, h1];

    let rec = Recorder::new(&store);
    let w0_a = rec.record(&call, Depth::NET_WRITE, span0, &Value::Bytes(b"one".to_vec())).unwrap();
    let w0_b = rec.record(&call, Depth::NET_WRITE, span0, &Value::Bytes(b"two".to_vec())).unwrap();
    let w1 = rec.record(&call, Depth::NET_WRITE, span1, &Value::Bytes(b"three".to_vec())).unwrap();

    // Two different witnesses for slot 0 conflict.
    let conflict =
        Replay::merge(&store, &[], &holes, &[&[(0, w0_a.witness())], &[(0, w0_b.witness())]]);
    assert!(
        matches!(
            conflict,
            Err(MergeError::Conflict(0, hole, first, second))
                if hole == h0 && first == w0_a.witness() && second == w0_b.witness()
        ),
        "{conflict:?}"
    );

    // Delivering slot 1's witness as if it answered slot 0 is rejected.
    let wrong_span = Replay::merge(&store, &[], &holes, &[&[(0, w1.witness())]]);
    assert!(matches!(wrong_span, Err(MergeError::Mismatched(0, ..))));

    // Answering the second identical-call act slot while leaving the first open is rejected.
    let gap = Replay::merge(&store, &[], &holes, &[&[(1, w1.witness())]]);
    assert!(matches!(gap, Err(MergeError::Mismatched(1, ..))));

    // Out-of-range slot index is rejected.
    let oob = Replay::merge(&store, &[], &holes, &[&[(9, w0_a.witness())]]);
    assert!(matches!(oob, Err(MergeError::OutOfRange(9, 2))));

    // A base witness that is not a witness answers no slot, so the error names
    // the object rather than slot 0, which it has nothing to do with.
    let base = Replay::merge(&store, &[h1], &holes, &[]);
    assert!(matches!(base, Err(MergeError::BaseWitness(w)) if w == h1), "{base:?}");
}

#[test]
fn of_trace_rejects_missing_witnesses_and_merge_keeps_base_answers() {
    let root = temp_dir("replay-base");
    let store = Store::open(root.join("store")).unwrap();
    let missing = nether_ledger::Cairn::of_encoded(b"absent-witness");
    assert!(matches!(Replay::of_trace(&store, &[missing]), Err(StoreError::Absent(_))));

    let src = store.put(&Stored::Value(Value::Bytes(b"m.nc".to_vec()))).unwrap();
    let arg = store.put(&Stored::Value(Value::Str("a.txt".into()))).unwrap();
    let call = Call::try_new("read", vec![arg]).unwrap();
    let span0 = Span::new(src, 0, 4).unwrap();
    let span1 = Span::new(src, 5, 9).unwrap();
    let h1 = store
        .put(&Stored::Node(Node::Hole { call: call.clone(), stratum: Stratum::THREE, span: span1 }))
        .unwrap();
    let rec = Recorder::new(&store);
    let base_w = rec.record(&call, Depth::DISK, span0, &Value::Bytes(b"v1".to_vec())).unwrap();
    let clash_w = rec.record(&call, Depth::DISK, span1, &Value::Bytes(b"v2".to_vec())).unwrap();

    // Base witness is preserved in the merged witness list, while the returned
    // Replay holds only newly delivered answers so resumed burial does not
    // re-serve stale act answers consumed in earlier stages.
    let (merged_wits, merged) = Replay::merge(&store, &[base_w.witness()], &[], &[]).unwrap();
    assert_eq!(merged_wits, vec![base_w.witness()]);
    assert!(merged.is_empty());

    // A worker delivery contradicting a read-stratum base witness is rejected.
    let err = Replay::merge(&store, &[base_w.witness()], &[h1], &[&[(0, clash_w.witness())]]);
    assert!(matches!(err, Err(MergeError::Conflict(0, ..))), "{err:?}");

    // Contradictory read-stratum base witnesses in the same trace are rejected.
    let bad_base = Replay::merge(&store, &[base_w.witness(), clash_w.witness()], &[], &[]);
    assert!(
        matches!(bad_base, Err(MergeError::BaseWitness(w)) if w == clash_w.witness()),
        "{bad_base:?}"
    );
}

#[test]
fn forged_stratum_is_rejected_and_replay_order_and_dedup_hold() {
    let root = temp_dir("replay-hardening");
    let store = Store::open(root.join("store")).unwrap();
    let src = store.put(&Stored::Value(Value::Bytes(b"m.nc".to_vec()))).unwrap();
    let arg = store.put(&Stored::Value(Value::Str("sym".into()))).unwrap();
    let span = Span::new(src, 0, 4).unwrap();

    // H20: forged `call_foreign` claiming stratum 1 is rejected by both ask_slots and merge.
    let forged_call = Call::try_new("call_foreign", vec![arg]).unwrap();
    let forged_hole = store
        .put(&Stored::Node(Node::Hole { call: forged_call.clone(), stratum: Stratum::ONE, span }))
        .unwrap();
    let forged_ans = store.put(&Stored::Value(Value::Bytes(b"ok".to_vec()))).unwrap();
    let forged_wit = store
        .put(&Stored::Node(Node::Witness {
            stratum: Stratum::ONE,
            call: forged_call,
            answer: forged_ans,
            span,
        }))
        .unwrap();
    let world = World::sealed();
    assert!(matches!(
        world.ask_slots(&[forged_hole], &[0], &Recorder::new(&store)),
        Err(nether_world::Unanswered::NotRecorded(_))
    ));
    assert!(matches!(
        Replay::merge(&store, &[], &[forged_hole], &[&[(0, forged_wit)]]),
        Err(MergeError::Mismatched(0, ..))
    ));
    assert!(matches!(
        Replay::merge(&store, &[forged_wit], &[], &[]),
        Err(MergeError::BaseWitness(w)) if w == forged_wit
    ));

    // L9: Replay::all() preserves deterministic insertion order.
    let c1 = Call::try_new("read", vec![arg]).unwrap();
    let arg2 = store.put(&Stored::Value(Value::Str("other".into()))).unwrap();
    let c2 = Call::try_new("read", vec![arg2]).unwrap();
    let r = Replay::of(&[
        (c2.clone(), Value::Int(2)),
        (c1.clone(), Value::Int(1)),
        (c2.clone(), Value::Int(3)),
    ]);
    let got: Vec<_> = r.all().map(|(c, v)| (c.clone(), v.clone())).collect();
    assert_eq!(got, vec![(c2.clone(), Value::Int(2)), (c2, Value::Int(3)), (c1, Value::Int(1))]);

    // M18: World::granting replaces duplicate capability providers cleanly.
    let d1 = root.join("d1");
    let d2 = root.join("d2");
    std::fs::create_dir_all(&d1).unwrap();
    std::fs::create_dir_all(&d2).unwrap();
    std::fs::write(d1.join("f.txt"), b"first").unwrap();
    std::fs::write(d2.join("f.txt"), b"second").unwrap();
    let replaced = World::sealed()
        .granting(Box::new(nether_world::Disk::reading(&d1)))
        .granting(Box::new(nether_world::Disk::reading(&d2)));
    let f_arg = store.put(&Stored::Value(Value::Str("f.txt".into()))).unwrap();
    let read_call = Call::try_new("read", vec![f_arg]).unwrap();
    let rec = replaced.ask(&read_call, span, &Recorder::new(&store)).unwrap();
    assert_eq!(
        store.get(rec.answer()).unwrap(),
        Stored::Value(Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(
            b"second".to_vec()
        )))))
    );

    // M9 & L3: Replay::of_trace rejects forged stratum, pure prims, wrong arity,
    // and contradictory read witnesses.
    assert!(matches!(Replay::of_trace(&store, &[forged_wit]), Err(StoreError::Corrupt { .. })));

    let pure_call = Call::try_new("concat", vec![arg, arg2]).unwrap();
    let pure_wit = store
        .put(&Stored::Node(Node::Witness {
            stratum: Stratum::ZERO,
            call: pure_call,
            answer: forged_ans,
            span,
        }))
        .unwrap();
    assert!(matches!(Replay::of_trace(&store, &[pure_wit]), Err(StoreError::Corrupt { .. })));

    let bad_arity_call = Call::try_new("read", vec![arg, arg2]).unwrap();
    let bad_arity_wit = store
        .put(&Stored::Node(Node::Witness {
            stratum: Stratum::THREE,
            call: bad_arity_call,
            answer: forged_ans,
            span,
        }))
        .unwrap();
    assert!(matches!(Replay::of_trace(&store, &[bad_arity_wit]), Err(StoreError::Corrupt { .. })));

    let ans1 = store.put(&Stored::Value(Value::Bytes(b"one".to_vec()))).unwrap();
    let ans2 = store.put(&Stored::Value(Value::Bytes(b"two".to_vec()))).unwrap();
    let w1 = store
        .put(&Stored::Node(Node::Witness {
            stratum: Stratum::THREE,
            call: read_call.clone(),
            answer: ans1,
            span,
        }))
        .unwrap();
    let w2 = store
        .put(&Stored::Node(Node::Witness {
            stratum: Stratum::THREE,
            call: read_call,
            answer: ans2,
            span: Span::new(src, 6, 10).unwrap(),
        }))
        .unwrap();
    assert!(matches!(Replay::of_trace(&store, &[w1, w2]), Err(StoreError::Corrupt { .. })));
}
