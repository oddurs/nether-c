//! Execution proofs for the literal-function interpreter subset.
//!
//! `lib/interpreter.nc` is source in the language it is meant to interpret.
//! This test appends a tiny entry point, gives its lexical cursor the first
//! sample program as bytes, and checks function resolution, demand and deposit
//! behavior. No Rust parser examines the sample after it becomes Bytes.

use nether_bury::{Answers, Residue, bury, bury_with};
use nether_core::check;
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse};

const INTERPRETER: &str = include_str!("../../../lib/interpreter.nc");
const HELLO: &str = include_str!("../../../tests/programs/hello.nc");

fn bytes_literal(bytes: &[u8]) -> String {
    let mut out = String::from("b\"");
    for byte in bytes {
        match byte {
            b'\\' => out.push_str("\\\\"),
            b'\"' => out.push_str("\\\""),
            b'\n' => out.push_str("\\n"),
            b'\r' => out.push_str("\\r"),
            b'\t' => out.push_str("\\t"),
            _ => out.push(char::from(*byte)),
        }
    }
    out.push('\"');
    out
}

fn said(source: &str) -> Vec<Value> {
    let ast = parse(source.as_bytes()).expect("interpreter program parses");
    let unit = lower(&ast).expect("interpreter program lowers");
    assert!(check(&unit).is_empty(), "interpreter program checks");
    let residue = bury(&unit, Cairn::of_encoded(source.as_bytes()), 10_000_000).expect("buries");
    deposits(&residue)
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

#[test]
fn the_nether_c_cursor_reads_the_first_sample_s_literal() {
    let source = format!(
        r#"{INTERPRETER}

U0 prove()
{{
  Bytes sample = b"U0 greet() {{ \"Hello from the nether\\n\"; }} demand greet();";
  text(sample, seek(sample, 0, b"\""));
}}

demand prove();
"#
    );
    assert_eq!(said(&source), vec![Value::Bytes(b"Hello from the nether\n".to_vec())]);
}

#[test]
fn literal_functions_interpret_hello_nc() {
    let source = format!(
        "{INTERPRETER}\ndemand interpret_literal_functions({});\n",
        bytes_literal(HELLO.as_bytes())
    );
    assert_eq!(said(&source), vec![Value::Str("Hello from the nether\n".into())]);
}

#[test]
fn demands_resolve_names_and_skip_unwanted_functions() {
    let program = r#"
// demand wrong(); "comment bait"
U0 wrong() { "unwanted"; }
demand second();
U0 first() { "first"; }
U0 second() { first(); "second"; }
demand first();
"#;
    let source = format!(
        "{INTERPRETER}\ndemand interpret_literal_functions({});",
        bytes_literal(program.as_bytes())
    );
    assert_eq!(
        said(&source),
        vec![Value::Str("first".into()), Value::Str("second".into()), Value::Str("first".into())]
    );
}

#[test]
fn no_demand_deposits_nothing() {
    let source = format!(
        "{INTERPRETER}\ndemand interpret_literal_functions({});",
        bytes_literal(b"U0 f() { \"demand f();\"; } // demand f();")
    );
    assert!(said(&source).is_empty());
}

#[test]
fn invalid_or_unsupported_input_cannot_report_success() {
    for program in [
        "/* unterminated",
        "U0 f() { \"unclosed",
        "U0 f() { \"x\" } demand f();",
        "demand missing();",
        "U0 f() { \"x\"; } demand f(); garbage",
        "U0 f() { \"\\q\"; } demand f();",
        "I64 n = 1; demand n;",
    ] {
        let source = format!(
            "{INTERPRETER}\ndemand interpret_literal_functions({});",
            bytes_literal(program.as_bytes())
        );
        let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
        assert!(
            bury(&unit, Cairn::of_encoded(source.as_bytes()), 10_000_000).is_err(),
            "accepted {program:?}"
        );
    }
}

#[test]
fn a_line_comment_at_eof_stops_at_eof() {
    let source =
        format!("{INTERPRETER}\nU0 prove() {{ source_start(b\"// last\"); }} demand prove();");
    assert_eq!(said(&source), vec![Value::Int(7)]);
}

#[test]
fn source_bytes_can_arrive_through_a_hole() {
    let source = format!(
        "{INTERPRETER}\ndemand interpret_literal_functions(must(descend disk {{ read(\"program.nc\") }}));"
    );
    let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
    assert!(check(&unit).is_empty());
    let name = Cairn::of_encoded(source.as_bytes());
    let pending = bury(&unit, name, 1_000_000).unwrap();
    assert!(pending.deposits.is_empty());
    assert_eq!(pending.holes.len(), 1);
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("source hole") };
    assert_eq!(call.function, "read");
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(
            HELLO.as_bytes().to_vec(),
        )))),
    );
    let finished = bury_with(&unit, name, 1_000_000, &answers).unwrap();
    assert!(finished.holes.is_empty());
    assert_eq!(deposits(&finished), vec![Value::Str("Hello from the nether\n".into())]);
}

#[test]
fn the_cursor_respects_block_comments_and_quoted_escapes() {
    let source = format!(
        r#"{INTERPRETER}
U0 prove() {{ text(b"/* ignored */ \"one \\\"two\\\"\\n\"", 0); }}
demand prove();
"#
    );
    assert_eq!(said(&source), vec![Value::Bytes(b"one \"two\"\n".to_vec())]);
}
