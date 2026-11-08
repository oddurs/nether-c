//! The first projection: an interpreter, specialised to a program.
//!
//! `spec/06-evaluation.md` §6.5. A starved call whose arguments are all values
//! residualises as its reduced body, so burying `lib/interpreter.nc` against a
//! guest with an unanswered question leaves a residue that does not name
//! `interpret`. The syntax read before that question is not read again.
//!
//! What it is not is a compiler. The residue is specialised up to the first
//! unanswered question on its path and reduces past the binding of that
//! answer where the arguments do not depend on it (0251, 0270, 0281); where
//! the next cursor or memory depends on the packed result record `r`, the
//! call waits on `r`.

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

/// The boundary, stated as a test rather than as a comment. A block reduces
/// speculatively past a statement that starves (§6.5, 0281), so the minted
/// functions specialise the guest source, environment and cursor into the
/// statements and branches after `Bytes r = expression__...();`, while still
/// calling `evaluate_program` where the next offset and memory wait on the
/// packed result record `r`.
#[test]
fn the_residue_still_interprets_past_the_unanswered_question() {
    let source = against(BUILD);
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    let residue = bury(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000).expect("buries");
    let printed = print(&residue.as_unit(&unit));
    let minted: String = printed.split("\nBytes ").filter(|f| f.contains("__")).collect();
    assert!(!minted.is_empty());
    // `evaluate_program__` specialised `s` past `Bytes r = expression__...();`
    // and pruned the binding, while `next` and `memory_of(r)` still wait on `r`.
    assert!(!minted.contains("evaluate_program(s,"), "s was not specialised:\n{minted}");
    assert!(minted.contains("evaluate_program(b\""), "the tail call disappeared:\n{minted}");
    // And the statements and branches after the starved bindings in
    // `expression`, `block_value` and `global_value` reduced in place.
    assert!(minted.contains("return relocate(r, 415);"), "cursor(args) did not fold:\n{minted}");
    assert!(!minted.contains("lookup(env, t)"), "branch after args did not fold:\n{minted}");
    assert!(!minted.contains("if (!checking)"), "branch after r did not fold:\n{minted}");
    assert!(
        minted.contains("b\"5:Error1:0unsupported deposit type at byte 399\""),
        "problem(...) inside the undecided branch after r did not fold:\n{minted}"
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

/// §6.5's staging law, for a program whose undecided branch or unreduced
/// statements write a local the burial already knew: burying the residue with
/// an answer deposits what burying the program with it does.
fn stages_like_the_program(src: &str) {
    let unit = lower(&parse(src.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&unit).is_empty(), "{:?}", check(&unit));
    let name = Cairn::of_encoded(src.as_bytes());
    let pending = bury(&unit, name, 1_000_000).expect("buries");
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("one read hole") };
    let printed = print(&pending.as_unit(&unit));
    let resumed_unit = lower(&parse(printed.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&resumed_unit).is_empty(), "{:?}\n{printed}", check(&resumed_unit));
    for said in [b"a".as_slice(), b"".as_slice()] {
        let answers = Answers::none().and(
            call.clone(),
            Value::Answer(Box::new(AnswerOf::Given(Value::Bytes(said.to_vec())))),
        );
        let resumed =
            bury_with(&resumed_unit, Cairn::of_encoded(printed.as_bytes()), 1_000_000, &answers)
                .expect("buries");
        let direct = bury_with(&unit, name, 1_000_000, &answers).expect("buries");
        assert_eq!(
            deposits(&resumed),
            deposits(&direct),
            "answered {said:?}, the residue disagrees with the program:\n{printed}"
        );
    }
}

/// The arm of an undecided branch was folded to its value, which is
/// `U0`, and the assignment in it went with it: `g` gave 5 where it gives 6. 0285.
#[test]
fn an_undecided_branch_keeps_the_assignment_in_its_arm() {
    stages_like_the_program(
        "I64 g() {\n  I64 x = 0;\n  Bytes@3 a = must(descend disk { read(\"f\") });\n  \
         x = 5;\n  if (len(a) > 0) { x = x + 1; }\n  x\n}\n\
         U0 show() { g(); }\ndemand show();\n",
    );
}

/// A statement after the starved one that jumps is left as written,
/// and the assignment reduced above it was not in the residue at all. 0285.
#[test]
fn a_statement_left_as_written_reads_what_was_assigned_above_it() {
    stages_like_the_program(
        "I64 g() {\n  I64 x = 0;\n  while (true) {\n    \
         Bytes@3 a = must(descend disk { read(\"f\") });\n    x = 5;\n    \
         if (len(a) > 0) { break; }\n    x = 7;\n    break;\n  }\n  x\n}\n\
         U0 show() { g(); }\ndemand show();\n",
    );
}

/// A local the branch only reads still has to hold, in the residue,
/// what an assignment the burial reduced away put there. 0285.
#[test]
fn a_branch_reads_a_local_assigned_before_the_question() {
    stages_like_the_program(
        "I64 g() {\n  I64 x = 0;\n  I64 y = 1;\n  y = 3;\n  \
         Bytes@3 a = must(descend disk { read(\"f\") });\n  \
         if (len(a) > 0) { x = y; }\n  x\n}\n\
         U0 show() { g(); }\ndemand show();\n",
    );
}

/// An assignment inside a block that finished is in no block's residue. 0285.
#[test]
fn a_branch_reads_a_local_a_finished_block_assigned() {
    stages_like_the_program(
        "I64 g() {\n  I64 x = 0;\n  { x = 5; }\n  \
         Bytes@3 a = must(descend disk { read(\"f\") });\n  \
         if (len(a) > 0) { x = x + 1; }\n  x\n}\n\
         U0 show() { g(); }\ndemand show();\n",
    );
}

/// A residue keeps its depths (§6.5). Each of these left a residue the
/// checker refused, `cannot look at a shade from stratum 3; 5 is held here`,
/// from a program it accepted. 0290.
#[test]
fn a_residue_keeps_its_depths() {
    // A local loses the arm that wrote it from 5.
    stages_like_the_program(
        "Bytes@5 g(Bool c) {\n  Bytes x;\n  \
         if (c) { x = must(descend net { get(\"u\") }); }\n  \
         else   { x = must(descend disk { read(\"f\") }); }\n  \
         descend net { look shade x }\n}\n\
         U0 show() { g(false); }\ndemand show();\n",
    );
    // A call specialised to a constant returns from 3 where its function returns from 5.
    stages_like_the_program(
        "Bytes@5 net_or_disk(Bool c) {\n  \
         if (c) { return must(descend net { get(\"u\") }); }\n  \
         must(descend disk { read(\"f\") })\n}\n\
         U0 show() { descend net { look shade net_or_disk(false) }; }\ndemand show();\n",
    );
}
