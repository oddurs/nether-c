//! §6.7: replay does not prefer the ledger over the world, it cannot reach it.
//!
//! The claim is about what a `Replay` *is*. It holds a map from a question to
//! what was said and nothing else — no `World`, no `Provider`, no path, no
//! socket — and there is no method that takes one. The compiler checks the
//! last part: see `Replay`'s `compile_fail` example.

use nether_core::Depth;
use nether_ledger::{Call, Node, Span, Store, StoreError, Stored, Value};
use nether_world::{Disk, MergeError, Recorder, Replay, World};

fn asking(function: &str) -> Call {
    Call { function: function.to_owned(), args: Vec::new() }
}

fn temp_dir(tag: &str) -> std::path::PathBuf {
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nether-replay-{tag}-{nanos}"));
    std::fs::create_dir_all(&dir).expect("temp dir");
    dir
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
    let a = Call { function: "read".into(), args: vec![nether_ledger::Cairn::of_encoded(b"a")] };
    let b = Call { function: "read".into(), args: vec![nether_ledger::Cairn::of_encoded(b"b")] };
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
                call: Call { function: "read".into(), args: vec![arg] },
                stratum: 3,
                span: Span { source: src, start: idx as u64, end: idx as u64 + 1 },
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
    let call = Call { function: "post".into(), args: vec![arg] };
    let span0 = Span { source: src, start: 0, end: 10 };
    let span1 = Span { source: src, start: 12, end: 22 };

    let h0 = store
        .put(&Stored::Node(Node::Hole { call: call.clone(), stratum: 6, span: span0 }))
        .unwrap();
    let h1 = store
        .put(&Stored::Node(Node::Hole { call: call.clone(), stratum: 6, span: span1 }))
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
