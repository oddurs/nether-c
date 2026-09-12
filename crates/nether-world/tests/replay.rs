//! §6.7: replay does not prefer the ledger over the world, it cannot reach it.
//!
//! The claim is about what a `Replay` *is*. It holds a map from a question to
//! what was said and nothing else — no `World`, no `Provider`, no path, no
//! socket — and there is no method that takes one. The compiler checks the
//! last part: see `Replay`'s `compile_fail` example.

use nether_ledger::{Call, Value};
use nether_world::Replay;

fn asking(function: &str) -> Call {
    Call { function: function.to_owned(), args: Vec::new() }
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
