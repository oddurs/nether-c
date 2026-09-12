//! §4.7: a bare expression statement **deposits** its value into the trace.
//!
//! Not printed and not discarded. A program holds no capability that reaches a
//! terminal, so depositing is the only thing it can do with a value it wants
//! kept, and §6.8 makes reading them a separate act performed by a person.

use nether_bury::{Residue, bury};
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse};

fn buried(src: &str) -> Residue {
    let ast = parse(src.as_bytes()).unwrap_or_else(|f| panic!("does not parse: {f:?}"));
    let unit = lower(&ast).unwrap_or_else(|f| panic!("does not lower: {f:?}"));
    bury(&unit, Cairn::of_encoded(src.as_bytes()), 1_000_000).expect("buries")
}

/// What was deposited, in order, as the values themselves.
fn said(r: &Residue) -> Vec<Value> {
    r.deposits
        .iter()
        .filter_map(|d| match r.get(*d) {
            Some(Stored::Node(Node::Deposit { value, .. })) => r.value(*value).cloned(),
            _ => None,
        })
        .collect()
}

#[test]
fn a_bare_expression_statement_deposits() {
    let r = buried("U0 g() @0 { \"hello\"; }\ndemand g();\n");
    assert_eq!(said(&r), vec![Value::Str("hello".to_owned())]);
}

/// `U0` has nothing to deposit.
#[test]
fn a_statement_with_no_value_deposits_nothing() {
    let r = buried("U0 g() @0 { }\ndemand g();\n");
    assert!(r.deposits.is_empty(), "{:?}", r.deposits);
}

/// Said twice is recorded twice: the program did say it twice.
#[test]
fn the_same_value_deposited_twice_is_two_deposits() {
    let r = buried("U0 g() @0 { \"x\"; \"x\"; }\ndemand g();\n");
    assert_eq!(r.deposits.len(), 2, "two statements, two deposits");
    assert_ne!(r.deposits[0], r.deposits[1], "two deposits differ by span");
}

#[test]
fn deposits_are_in_source_order() {
    let r = buried("U0 g() @0 { 1; 2; 3; }\ndemand g();\n");
    assert_eq!(said(&r), vec![Value::Int(1), Value::Int(2), Value::Int(3)]);
}

/// A deposit points at where it was written, which is what a lamp shows.
#[test]
fn a_deposit_carries_its_span() {
    let src = "U0 g() @0 { 7; }\ndemand g();\n";
    let r = buried(src);
    let Some(Stored::Node(Node::Deposit { span, .. })) = r.get(r.deposits[0]) else {
        panic!("not a Deposit node");
    };
    assert_eq!(span.source, Cairn::of_encoded(src.as_bytes()));
    assert!(span.end > span.start, "a span that ends before it starts");
    let at = usize::try_from(span.start).expect("fits");
    let to = usize::try_from(span.end).expect("fits");
    assert_eq!(&src[at..to], "7", "the span does not cover what was deposited");
}

/// A function nothing demands deposits nothing: §6.2, work nothing needs is
/// never done, and that is a guarantee rather than an optimisation.
#[test]
fn an_undemanded_function_deposits_nothing() {
    let r = buried("U0 g() @0 { \"never\"; }\nU0 h() @0 { \"said\"; }\ndemand h();\n");
    assert_eq!(said(&r), vec![Value::Str("said".to_owned())]);
}
