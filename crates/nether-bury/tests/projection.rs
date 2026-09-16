//! The first projection: an interpreter, specialised to a program.
//!
//! `spec/06-evaluation.md` §6.5. A starved call whose arguments are all values
//! residualises as its reduced body, so burying `lib/interpreter.nc` against a
//! guest with an unanswered question leaves a residue that does not name
//! `interpret`. The syntax read before that question is not read again.
//!
//! What it is not is a compiler. The residue is specialised up to the first
//! unanswered question on its path and interprets from there, because an `if`
//! whose condition waits on the world residualises with both arms unreduced.
//! That boundary is 0251 and is asserted here so that it cannot move quietly.

use nether_bury::{Answers, Residue, bury, bury_with};
use nether_core::{ExprKind, Literal, check};
use nether_ledger::{AnswerOf, Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse, print};

const INTERPRETER: &str = concat!(
    include_str!("../../../lib/interpreter.nc"),
    "\n",
    include_str!("../../../lib/value.nc"),
    "\n",
    include_str!("../../../lib/evaluate.nc")
);
const BUILD: &str = include_str!("../../../tests/programs/build.nc");

/// The interpreter, with the guest written into it as a constant.
fn against(guest: &str) -> String {
    format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(guest.as_bytes()))
}

#[test]
fn a_fixed_guest_leaves_a_residue_that_does_not_name_the_interpreter() {
    let source = against(BUILD);
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&unit).is_empty());
    let residue = bury(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000).expect("buries");

    // The guest's own question survives, with its path already resolved: the
    // interpreter read `read("main.nc")` out of the guest's syntax and will
    // not read it again.
    assert_eq!(residue.holes.len(), 1);
    let Node::Hole { call, .. } = residue.questions()[0] else { panic!("read hole") };
    assert_eq!(call.function, "read");
    assert_eq!(residue.value(call.args[0]), Some(&Value::Str("main.nc".into())));

    // The demand is a call to something burial minted, not to `interpret`.
    let ExprKind::Call { callee, args } = &residue.demands[0].kind else {
        panic!("the demand did not reduce to a call: {:?}", residue.demands[0].kind);
    };
    assert!(args.is_empty(), "a specialised body takes no arguments");
    let ExprKind::Func(id) = callee.kind else { panic!("not a function") };
    let named = &residue.as_unit(&unit).funcs[id.0 as usize].name;
    assert!(named.starts_with("interpret__"), "{named}");
    assert!(!residue.specialised.is_empty());

    // And no demand names the entry point it was written against.
    let printed = print(&residue.as_unit(&unit));
    let last = printed.rsplit_once("demand ").expect("a demand").1;
    assert!(!last.contains("interpret("), "the residue still calls interpret: {last}");
}

#[test]
fn the_residue_answers_the_guest_s_question_the_way_the_interpreter_would() {
    let source = against(BUILD);
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    let name = Cairn::of_encoded(source.as_bytes());
    let pending = bury(&unit, name, 100_000_000).expect("buries");
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("read hole") };

    for said in [b"contents".as_slice(), b"".as_slice(), b"\xff\x00".as_slice()] {
        let answers = Answers::none().and(
            call.clone(),
            Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(said.to_vec())))),
        );
        // Through the residue, which is a program and is buried like any
        // other, and directly, which is the staging law's other side.
        let printed = print(&pending.as_unit(&unit));
        let resumed_unit = lower(&parse(printed.as_bytes()).expect("parses")).expect("lowers");
        assert!(check(&resumed_unit).is_empty(), "{:?}", check(&resumed_unit));
        let resumed =
            bury_with(&resumed_unit, Cairn::of_encoded(printed.as_bytes()), 100_000_000, &answers)
                .expect("buries");
        let direct = bury_with(&unit, name, 100_000_000, &answers).expect("buries");

        let mut want = b"obj:".to_vec();
        want.extend_from_slice(said);
        assert!(resumed.holes.is_empty());
        assert_eq!(results(&resumed), vec![("Bytes".into(), 3, want)]);
        assert_eq!(results(&resumed), results(&direct));
        assert_eq!(deposits(&resumed), deposits(&direct));
    }
}

/// §6.5: what burial leaves is what burying it again leaves. A residue that
/// specialised itself one layer further on every burial would be a staging law
/// that never settles.
#[test]
fn burying_the_residue_again_leaves_the_same_residue() {
    let source = against(BUILD);
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    let once = bury(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000).expect("buries");
    let printed = print(&once.as_unit(&unit));
    let again = lower(&parse(printed.as_bytes()).expect("parses")).expect("lowers");
    let twice =
        bury(&again, Cairn::of_encoded(printed.as_bytes()), 100_000_000).expect("buries again");
    assert_eq!(printed, print(&twice.as_unit(&again)));
}

/// The boundary, stated as a test rather than as a comment. Burial does not
/// reduce inside an arm it may not take (§6.2), so the residue still calls the
/// general interpreter for whatever comes after the guest's question. When
/// 0251 settles, this is the assertion that has to change.
#[test]
fn the_residue_still_interprets_past_the_unanswered_question() {
    let source = against(BUILD);
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    let residue = bury(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000).expect("buries");
    let printed = print(&residue.as_unit(&unit));
    let minted = printed.split("\nBytes ").filter(|f| f.contains("__")).count();
    assert!(minted > 0);
    assert!(
        printed.contains("evaluate_program(s,"),
        "nothing calls the general evaluator any more, which would mean 0251 landed"
    );
}

fn bytes_literal(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for character in std::str::from_utf8(bytes).expect("UTF-8 source").chars() {
        match character {
            '\\' => out.push_str("\\\\"),
            '\"' => out.push_str("\\\""),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            '\0' => out.push_str("\\0"),
            _ => out.push(character),
        }
    }
    out.push('\"');
    out
}

fn take_packet(bytes: &mut &[u8]) -> Vec<u8> {
    let colon = bytes.iter().position(|b| *b == b':').expect("packet colon");
    let size: usize = std::str::from_utf8(&bytes[..colon]).unwrap().parse().unwrap();
    let payload = bytes[colon + 1..colon + 1 + size].to_vec();
    *bytes = &bytes[colon + 1 + size..];
    payload
}

fn results(residue: &Residue) -> Vec<(String, u8, Vec<u8>)> {
    let ExprKind::Literal(Literal::Bytes(bytes)) = &residue.demands[0].kind else {
        panic!("unreduced result: {:?}", residue.demands[0]);
    };
    let mut input = bytes.as_slice();
    let mut values = Vec::new();
    while !input.is_empty() {
        let packet = take_packet(&mut input);
        let mut value = packet.as_slice();
        let kind = String::from_utf8(take_packet(&mut value)).unwrap();
        let depth = String::from_utf8(take_packet(&mut value)).unwrap().parse().unwrap();
        values.push((kind, depth, value.to_vec()));
    }
    values
}

fn deposits(residue: &Residue) -> Vec<Value> {
    residue
        .deposits
        .iter()
        .filter_map(|deposit| match residue.get(*deposit) {
            Some(Stored::Node(Node::Deposit { value, .. })) => residue.value(*value).cloned(),
            _ => None,
        })
        .collect()
}
