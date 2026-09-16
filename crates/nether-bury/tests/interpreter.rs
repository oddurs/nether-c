//! Execution proofs for the interpreter's sample-program bootstrap.
//!
//! `lib/interpreter.nc` is source in the language it is meant to interpret.
//! The host parses the interpreter and an entry point, never its guest bytes.
//! The unchanged samples exercise deposits, answered holes and Orpheus checking.

use nether_bury::{Answers, HaltKind, Residue, bury, bury_with};
use nether_core::{ExprKind, Literal, check};
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse, print};

const INTERPRETER: &str = concat!(
    include_str!("../../../lib/interpreter.nc"),
    "\n",
    include_str!("../../../lib/value.nc"),
    "\n",
    include_str!("../../../lib/evaluate.nc")
);
const HELLO: &str = include_str!("../../../tests/programs/hello.nc");
const BUILD: &str = include_str!("../../../tests/programs/build.nc");
const STAMP: &str = include_str!("../../../tests/programs/stamp.nc");

#[test]
fn fixed_programs_have_a_bounded_burial_cost() {
    // Before declaration indexing and shared memory: 1,300,946 / 20,340,206 /
    // 7,759,045 steps. Leave headroom, but do not silently return to that cost.
    for (label, program, budget) in
        [("hello", HELLO, 1_250_000), ("build", BUILD, 6_000_000), ("stamp", STAMP, 2_500_000)]
    {
        let residue = guest(program, &Answers::none());
        eprintln!("{label}: {} steps", residue.fuel_spent);
        assert!(residue.fuel_spent < budget, "{label}: {} >= {budget}", residue.fuel_spent);
    }
}

#[test]
fn a_shared_global_is_evaluated_once_across_calls_and_demands() {
    let program = r#"
Bytes make() { "once"; b"x" }
Bytes root = make();
Bytes left = concat(root, b"L");
Bytes right = concat(root, b"R");
Bytes both() { concat(left, right) }
demand both();
demand root;
demand both();
"#;
    let residue = guest(program, &Answers::none());
    assert_eq!(deposits(&residue), vec![Value::Str("once".into())]);
    assert_eq!(
        results(&residue),
        vec![
            ("Bytes".into(), 0, b"xLxR".to_vec()),
            ("Bytes".into(), 0, b"x".to_vec()),
            ("Bytes".into(), 0, b"xLxR".to_vec()),
        ]
    );
}

#[test]
fn cyclic_globals_name_the_binding_instead_of_exhausting_fuel() {
    for (program, name) in [
        ("Bytes self = self; demand self;", "self"),
        ("Bytes a = b; Bytes b = a;", "a"),
        ("Bytes f() { a } Bytes a = f(); demand a;", "a"),
    ] {
        let residue = guest(program, &Answers::none());
        assert!(residue.holes.is_empty());
        assert!(residue.deposits.is_empty());
        let values = results(&residue);
        assert_eq!(values[0].0, "Error");
        assert!(
            String::from_utf8_lossy(&values[0].2)
                .starts_with(&format!("cyclic global: {name} at byte "))
        );
        assert!(residue.fuel_spent < 1_000_000);
    }
}

#[test]
fn checking_and_function_locals_cannot_fill_the_evaluation_cache() {
    let program = r#"
Bytes content = make();
Bytes make() { "made"; b"world" }
Bytes ignore(Bytes content) { b"unused parameter" }
Bytes local() { Bytes content = b"local"; content }
Bytes load() { content }
U0 unused() { "unused"; }
demand local(); demand load(); demand content;
"#;
    let residue = guest(program, &Answers::none());
    assert_eq!(deposits(&residue), vec![Value::Str("made".into())]);
    assert_eq!(
        results(&residue),
        vec![
            ("Bytes".into(), 0, b"local".to_vec()),
            ("Bytes".into(), 0, b"world".to_vec()),
            ("Bytes".into(), 0, b"world".to_vec()),
        ]
    );
}

#[test]
fn independent_interpretations_do_not_share_cached_globals() {
    let program =
        "Bytes make() { \"once per unit\"; b\"x\" } Bytes x = make(); demand x; demand x;";
    let entry = format!(
        "{INTERPRETER}\ndemand interpret({0}); demand interpret({0});",
        bytes_literal(program.as_bytes())
    );
    assert_eq!(
        said(&entry),
        vec![Value::Str("once per unit".into()), Value::Str("once per unit".into())]
    );
}

#[test]
fn unreachable_code_is_checked_but_cannot_populate_runtime_memory() {
    let program = "Bytes make() { \"made\"; b\"x\" } Bytes x = make(); U0 f() { return; x; } demand f(); demand x;";
    let residue = guest(program, &Answers::none());
    assert_eq!(deposits(&residue), vec![Value::Str("made".into())]);
    assert_eq!(
        results(&residue),
        vec![("U0".into(), 0, vec![]), ("Bytes".into(), 0, b"x".to_vec())]
    );
    let broken = guest("U0 f() { return; missing; } demand f();", &Answers::none());
    assert_eq!(results(&broken)[0].0, "Error");
    assert!(deposits(&broken).is_empty());
}

#[test]
fn record_encoding_handles_binary_payloads_and_signed_extremes() {
    let source = format!(
        r#"{INTERPRETER}
U0 prove() {{
  Bytes payload = concat(b"4:fake\0", slice(raw("é"), 0, 1));
  head(packet(payload)) == payload;
  rest(concat(packet(payload), packet(b"tail"))) == packet(b"tail");
  decimal(-9223372036854775807 - 1);
  decimal(9223372036854775807);
  number(decimal(-9223372036854775807 - 1));
  number(decimal(9223372036854775807));
}}
demand prove();"#
    );
    assert_eq!(
        said(&source),
        vec![
            Value::Bool(true),
            Value::Bool(true),
            Value::Bytes(b"-9223372036854775808".to_vec()),
            Value::Bytes(b"9223372036854775807".to_vec()),
            Value::Int(i64::MIN),
            Value::Int(i64::MAX),
        ]
    );
}

#[test]
fn decimal_records_reject_missing_digits_and_overflow() {
    for input in
        ["", "-", "+1", "1x", "9223372036854775808", "-9223372036854775809", "18446744073709551616"]
    {
        let source = format!("{INTERPRETER}\ndemand number({});", bytes_literal(input.as_bytes()));
        let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
        let halt = bury(&unit, Cairn::of_encoded(source.as_bytes()), 1_000_000)
            .expect_err("malformed decimal must collapse");
        assert!(matches!(halt.kind, HaltKind::Collapsed(_)), "{input:?}: {halt:?}");
    }
}

#[test]
fn cursor_bounds_do_not_wrap_before_they_are_checked() {
    let source = format!(
        r#"{INTERPRETER}
U0 prove() {{
  at(b"abc", -1, b"a");
  at(b"abc", 9223372036854775807, b"a");
  at(b"abc", 3, b"");
  at(b"abc", 3, b"a");
  at(b"abc", 2, b"c");
  seek(b"abc", 9223372036854775807, b"a");
  seek(b"abc", 0, b"longer");
}}
demand prove();"#
    );
    assert_eq!(
        said(&source),
        vec![
            Value::Bool(false),
            Value::Bool(false),
            Value::Bool(true),
            Value::Bool(false),
            Value::Bool(true),
            Value::Int(3),
            Value::Int(3)
        ]
    );
}

#[test]
fn calls_respect_lexical_bindings_and_reject_non_functions() {
    for program in [
        "Bytes f = b\"x\"; demand f();",
        "Bytes f() { b\"global\" } Bytes g(Bytes f) { f() } demand g(b\"local\");",
        "Bytes f() { Bytes concat = b\"local\"; concat(b\"a\", b\"b\") } demand f();",
    ] {
        let program = format!("U0 before() {{ \"must not deposit\"; }} demand before(); {program}");
        let residue = guest(&program, &Answers::none());
        assert!(residue.holes.is_empty());
        assert!(residue.deposits.is_empty());
        let values = results(&residue);
        assert_eq!(values[0].0, "Error", "{program}");
        assert!(
            String::from_utf8_lossy(&values[0].2).starts_with("binding is not callable"),
            "{values:?}"
        );
    }
}

#[test]
fn invalid_utf8_guest_source_is_not_silently_repaired() {
    let source = format!("{INTERPRETER}\ndemand interpret(slice(raw(\"é\"), 0, 1));");
    let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
    let residue = bury(&unit, Cairn::of_encoded(source.as_bytes()), 1_000_000).unwrap();
    assert_eq!(
        results(&residue),
        vec![("Error".into(), 0, b"source is not UTF-8 at byte 0".to_vec())]
    );
}

fn guest(program: &str, answers: &Answers) -> Residue {
    let source = format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(program.as_bytes()));
    let unit = lower(&parse(source.as_bytes()).expect("parse")).expect("lower");
    assert!(check(&unit).is_empty(), "{:?}", check(&unit));
    bury_with(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000, answers).expect("bury")
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

#[test]
fn general_interpreter_build_starves_then_resumes() {
    let pending = guest(BUILD, &Answers::none());
    assert_eq!(pending.holes.len(), 1);
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("read hole") };
    assert_eq!(call.function, "read");
    assert_eq!(call.args.len(), 1);
    assert_eq!(pending.value(call.args[0]), Some(&Value::Str("main.nc".into())));
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(b"payload".to_vec())))),
    );
    let finished = guest(BUILD, &answers);
    assert!(finished.holes.is_empty());
    assert!(finished.deposits.is_empty());
    assert_eq!(results(&finished), vec![("Bytes".into(), 3, b"obj:payload".to_vec())]);
}

#[test]
fn general_interpreter_rejects_stamp_without_requesting_the_network() {
    let residue = guest(STAMP, &Answers::none());
    assert!(residue.holes.is_empty());
    assert!(residue.deposits.is_empty());
    let values = results(&residue);
    assert_eq!(values.len(), 1);
    assert_eq!(values[0].0, "Error");
    assert_eq!(
        String::from_utf8_lossy(&values[0].2),
        format!("Orpheus: origin 5, ambient 0 at byte {}", STAMP.find("look(reply)").unwrap())
    );
}

#[test]
fn repaired_stamp_checks_without_evaluating_unused_world_questions() {
    let repaired = STAMP.replace("len(look(reply))", "descend net { len(look(reply)) }");
    let residue = guest(&repaired, &Answers::none());
    assert!(residue.holes.is_empty());
    assert!(residue.deposits.is_empty());
    assert!(results(&residue).is_empty());
    let too_shallow = STAMP.replace("len(look(reply))", "descend disk { len(look(reply)) }");
    let values = results(&guest(&too_shallow, &Answers::none()));
    assert_eq!(values[0].0, "Error");
    assert!(String::from_utf8_lossy(&values[0].2).contains("origin 5, ambient 3"));
}

#[test]
fn a_demanded_legal_look_waits_for_its_network_answer() {
    let program = "Shade<Bytes> reply = descend net { shade must(get(\"https://data.invalid\")) }; I64 n = descend net { len(look(reply)) }; demand n;";
    let pending = guest(program, &Answers::none());
    assert_eq!(pending.holes.len(), 1);
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("network hole") };
    assert_eq!(call.function, "get");
    assert_eq!(pending.value(call.args[0]), Some(&Value::Str("https://data.invalid".into())));
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(b"five!".to_vec())))),
    );
    let finished = guest(program, &answers);
    assert!(finished.holes.is_empty());
    assert_eq!(results(&finished), vec![("I64".into(), 5, b"5".to_vec())]);
}

#[test]
fn refused_read_is_not_a_successful_empty_file() {
    let program = "Answer<Bytes> answer = descend disk { read(\"absent\") }; demand must(answer);";
    let pending = guest(program, &Answers::none());
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("file hole") };
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(nether_ledger::Refusal::Absent))),
    );
    let finished = guest(program, &answers);
    assert!(finished.holes.is_empty());
    let values = results(&finished);
    assert_eq!(values[0].0, "Error");
    assert!(String::from_utf8_lossy(&values[0].2).starts_with("must: world refused"));
}

#[test]
fn renamed_forward_bindings_and_parameters_are_not_sample_recognition() {
    let program = r#"
/* Bytes obj = compile(src); demand wrong(); get("bait"); */
demand product;
Bytes product = wrap(content, b":tail");
Bytes unused = must(descend net { get("https://unused.invalid") });
Bytes content = b"different";
Bytes wrap(Bytes Error, Bytes suffix) { Bytes prefix = b"new:"; concat(prefix, concat(Error, suffix)) }
"#;
    let residue = guest(program, &Answers::none());
    assert!(residue.holes.is_empty());
    assert!(residue.deposits.is_empty());
    assert_eq!(results(&residue), vec![("Bytes".into(), 0, b"new:different:tail".to_vec())]);
}

#[test]
fn calls_preserve_argument_depth_even_when_the_body_ignores_it() {
    let program = r#"
Bytes discard(Bytes argument) { b"constant" }
Bytes content = must(descend disk { read("renamed.nc") });
demand discard(content);
"#;
    let pending = guest(program, &Answers::none());
    assert_eq!(pending.holes.len(), 1);
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("read hole") };
    assert_eq!(pending.value(call.args[0]), Some(&Value::Str("renamed.nc".into())));
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(b"ignored".to_vec())))),
    );
    assert_eq!(results(&guest(program, &answers)), vec![("Bytes".into(), 3, b"constant".to_vec())]);
}

#[test]
fn semantic_errors_are_reported_before_any_deposit_or_world_request() {
    for (program, expected) in [
        ("demand missing();", "unknown call"),
        ("U0 f() { \"x\" } demand f();", "binding type mismatch"),
        ("Bytes f(Bytes a) { a } demand f();", "too few arguments"),
        ("Bytes f(Bytes a) { a } demand f(b\"a\", b\"b\");", "too many arguments"),
        ("Bytes f(Bytes a) { a } demand f(\"wrong\");", "binding type mismatch"),
        ("Bytes a = b\"a\"; Bytes a = b\"b\";", "duplicate declaration"),
        ("Bytes f(Bytes a, Bytes a) { a }", "duplicate parameter"),
        ("Bytes@0 a = must(descend disk { read(\"secret\") });", "depth assertion mismatch"),
        ("Bytes a = must(read(\"secret\"));", "request requires descend"),
        ("demand look(b\"not a shade\");", "look requires a Shade"),
    ] {
        let program = format!("U0 before() {{ \"must not deposit\"; }} demand before(); {program}");
        let residue = guest(&program, &Answers::none());
        assert!(residue.holes.is_empty(), "{program}");
        assert!(residue.deposits.is_empty(), "{program}");
        let values = results(&residue);
        assert_eq!(values[0].0, "Error", "{program}");
        assert!(
            String::from_utf8_lossy(&values[0].2).starts_with(expected),
            "{program}: {values:?}"
        );
    }
}

#[test]
fn literals_preserve_utf8_and_record_delimiters() {
    let program = "U0 f() { \"é雪:9\\0\\n\"; } demand f(); demand b\"7:Bytes1:5\";";
    let residue = guest(program, &Answers::none());
    assert_eq!(deposits(&residue), vec![Value::Str("é雪:9\0\n".into())]);
    assert_eq!(
        results(&residue),
        vec![("U0".into(), 0, vec![]), ("Bytes".into(), 0, b"7:Bytes1:5".to_vec())]
    );
}

#[test]
fn general_interpreter_deposits_hello() {
    let source = format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(HELLO.as_bytes()));
    assert_eq!(said(&source), vec![Value::Str("Hello from the nether\n".into())]);
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
fn demands_resolve_names_and_skip_unwanted_functions() {
    let program = r#"
// demand wrong(); "comment bait"
U0 wrong() { "unwanted"; }
demand second();
U0 first() { "first"; }
U0 second() { first(); "second"; }
demand first();
"#;
    let source = format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(program.as_bytes()));
    assert_eq!(
        said(&source),
        vec![Value::Str("first".into()), Value::Str("second".into()), Value::Str("first".into())]
    );
}

#[test]
fn no_demand_deposits_nothing() {
    let source = format!(
        "{INTERPRETER}\ndemand interpret({});",
        bytes_literal(b"U0 f() { \"demand f();\"; } // demand f();")
    );
    assert!(said(&source).is_empty());
}

#[test]
fn invalid_or_unsupported_input_cannot_report_success() {
    for program in [
        "/* unterminated",
        "U0 f() { \"unclosed",
        "U0 f() { \"x\"; } demand f(); garbage",
        "U0 f() { \"\\q\"; } demand f();",
        "Bytes@9 a = b\"a\";",
        "Bytes@-1 a = b\"a\";",
        "U0 f(] { }",
        "Bytes f(Bytes x) { x } demand f(b\"x\",);",
        "Bytes f(Bytes x,) { x } demand f(b\"x\");",
        "demand f(b\"x\"); Bytes f(Bytes x,) { x }",
        "U0 f() { \"backslash\\\nnewline\"; } demand f();",
    ] {
        let source =
            format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(program.as_bytes()));
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
    let source =
        format!("{INTERPRETER}\ndemand interpret(must(descend disk {{ read(\"program.nc\") }}));");
    let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
    assert!(check(&unit).is_empty());
    let name = Cairn::of_encoded(source.as_bytes());
    let pending = bury(&unit, name, 100_000_000).unwrap();
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
    let finished = bury_with(&unit, name, 100_000_000, &answers).unwrap();
    assert!(finished.holes.is_empty());
    assert_eq!(deposits(&finished), vec![Value::Str("Hello from the nether\n".into())]);
}

#[test]
fn source_then_file_answers_survive_printed_residue_and_reburial() {
    let source =
        format!("{INTERPRETER}\ndemand interpret(must(descend disk {{ read(\"program.nc\") }}));");
    let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
    let name = Cairn::of_encoded(source.as_bytes());
    let pending = bury(&unit, name, 100_000_000).unwrap();
    assert_eq!(pending.holes.len(), 1);
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("source hole") };
    let source_answer = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(
            BUILD.as_bytes().to_vec(),
        )))),
    );
    let awaiting_file = bury_with(&unit, name, 100_000_000, &source_answer).unwrap();
    assert_eq!(awaiting_file.holes.len(), 1);
    let Node::Hole { call, .. } = awaiting_file.questions()[0] else { panic!("file hole") };
    assert_eq!(awaiting_file.value(call.args[0]), Some(&Value::Str("main.nc".into())));
    let answers = source_answer.and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(b"staged".to_vec())))),
    );
    let printed = print(&awaiting_file.as_unit(&unit));
    let resumed_unit =
        lower(&parse(printed.as_bytes()).expect("residue parses")).expect("residue lowers");
    assert!(check(&resumed_unit).is_empty());
    let resumed =
        bury_with(&resumed_unit, Cairn::of_encoded(printed.as_bytes()), 100_000_000, &answers)
            .unwrap();
    let direct = bury_with(&unit, name, 100_000_000, &answers).unwrap();
    assert!(resumed.holes.is_empty());
    assert_eq!(results(&resumed), results(&direct));
    assert_eq!(results(&resumed), vec![("Bytes".into(), 3, b"obj:staged".to_vec())]);
    assert_eq!(deposits(&resumed), deposits(&direct));
}

#[test]
fn an_answered_global_is_cached_after_printed_residue_resumes() {
    let program = r#"
Bytes make() { Bytes v = must(descend disk { read("input") }); "settled"; v }
Bytes shared = make();
demand concat(shared, shared);
demand shared;
"#;
    let source = format!("{INTERPRETER}\ndemand interpret({});", bytes_literal(program.as_bytes()));
    let unit = lower(&parse(source.as_bytes()).unwrap()).unwrap();
    let pending = bury(&unit, Cairn::of_encoded(source.as_bytes()), 100_000_000).unwrap();
    assert_eq!(pending.holes.len(), 1);
    assert!(pending.deposits.is_empty());
    let Node::Hole { call, .. } = pending.questions()[0] else { panic!("input hole") };
    let answers = Answers::none().and(
        call.clone(),
        Value::Answer(Box::new(nether_ledger::AnswerOf::Given(Value::Bytes(b"ok".to_vec())))),
    );
    let printed = print(&pending.as_unit(&unit));
    let resumed_unit = lower(&parse(printed.as_bytes()).unwrap()).unwrap();
    assert!(check(&resumed_unit).is_empty());
    let resumed =
        bury_with(&resumed_unit, Cairn::of_encoded(printed.as_bytes()), 100_000_000, &answers)
            .unwrap();
    assert!(resumed.holes.is_empty());
    assert_eq!(deposits(&resumed), vec![Value::Str("settled".into())]);
    assert_eq!(
        results(&resumed),
        vec![("Bytes".into(), 3, b"okok".to_vec()), ("Bytes".into(), 3, b"ok".to_vec())]
    );
    let direct = guest(program, &answers);
    assert_eq!(deposits(&resumed), deposits(&direct));
    assert_eq!(results(&resumed), results(&direct));
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
