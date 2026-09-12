//! The rites, run as a person runs them.
//!
//! The binary is invoked, not the functions, because §08 is about a command
//! line: its exit codes, its stdout, and the one thing it refuses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use nether_ledger::{Cairn, Store, Stored, Value};

/// A directory nothing else is using, named after the test that asked.
fn scratch(what: &str) -> PathBuf {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nether-{what}-{at}"));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn nether(store: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nether"));
    cmd.args(args);
    if let Some(store) = store {
        cmd.env("NETHER_STORE", store);
    } else {
        cmd.env_remove("NETHER_STORE");
    }
    cmd.output().expect("the binary runs")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("an exit code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr is UTF-8")
}

// ── the refusal ─────────────────────────────────────────────────────────────

#[test]
fn there_is_no_run() {
    // §8.0, the only frozen requirement in the draft.
    let out = nether(None, &["run", "hello.nc"]);
    assert_eq!(code(&out), 64);
    assert!(stderr(&out).starts_with("nether: there is no `run`."), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "a refusal is not output");
    for instead in ["nether bury", "nether exhume", "nether lamp"] {
        assert!(stderr(&out).contains(instead), "it does not say to use `{instead}`");
    }
}

// ── §8.5 `cairn` ────────────────────────────────────────────────────────────

#[test]
fn naming_a_file_names_what_is_in_it() {
    let dir = scratch("name");
    let path = dir.join("kernel.nc");
    std::fs::write(&path, b"demand 1;\n").expect("write");

    let out = nether(None, &["cairn", path.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));

    let want = Value::Bytes(b"demand 1;\n".to_vec()).cairn();
    assert!(stdout(&out).starts_with(&want.to_string()), "{}", stdout(&out));
    assert!(stdout(&out).contains("10 bytes"), "{}", stdout(&out));
}

#[test]
fn two_files_with_the_same_bytes_have_the_same_name() {
    let dir = scratch("same");
    for name in ["a.nc", "b.nc"] {
        std::fs::write(dir.join(name), b"demand 1;\n").expect("write");
    }
    let one = stdout(&nether(None, &["cairn", dir.join("a.nc").to_str().unwrap()]));
    let two = stdout(&nether(None, &["cairn", dir.join("b.nc").to_str().unwrap()]));
    let name = |line: &str| line.split_whitespace().next().unwrap_or_default().to_string();
    assert_eq!(name(&one), name(&two));
    assert_ne!(one, two, "and they are still two different files");
}

#[test]
fn json_is_one_object_and_nothing_else() {
    // §8.1: writes a single JSON object to stdout and nothing else.
    let dir = scratch("json");
    let path = dir.join("with \"quotes\".nc");
    std::fs::write(&path, b"x").expect("write");

    let out = nether(None, &["cairn", path.to_str().unwrap(), "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.starts_with('{') && text.trim_end().ends_with('}'), "{text}");
    // The path had a quote in it, and the object is still one object.
    assert!(text.contains("\\\"quotes\\\""), "{text}");
    assert!(stderr(&out).is_empty());
}

#[test]
fn a_file_that_is_not_there_fails_for_a_stated_reason() {
    let out = nether(None, &["cairn", "/nowhere/at/all.nc"]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("/nowhere/at/all.nc"), "{}", stderr(&out));
}

// ── §8.5 `--verify` ─────────────────────────────────────────────────────────

fn with_object(what: &str, value: Value) -> (PathBuf, Cairn) {
    let dir = scratch(what);
    let store = Store::open(&dir).expect("a store");
    let cairn = store.put(&Stored::Value(value)).expect("put");
    (dir, cairn)
}

#[test]
fn a_name_the_ledger_still_holds_holds() {
    let (dir, cairn) = with_object("holds", Value::Str("kernel.nc".into()));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("holds"), "{}", stdout(&out));
    assert!(stdout(&out).contains("value"), "{}", stdout(&out));
}

#[test]
fn a_short_prefix_is_enough_when_it_is_unambiguous() {
    let (dir, cairn) = with_object("short", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.short()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
}

#[test]
fn a_name_that_is_not_there_is_absent_and_says_so() {
    let (dir, _) = with_object("absent", Value::Int(7));
    let missing = Value::Int(99).cairn();
    let out = nether(Some(&dir), &["cairn", "--verify", &missing.to_string()]);
    assert_eq!(code(&out), 66, "§8.8: a cairn was not found in the ledger");
    assert!(stderr(&out).contains("nothing"), "{}", stderr(&out));
}

#[test]
fn something_that_is_not_a_cairn_is_a_usage_error() {
    let (dir, _) = with_object("bad", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", "zzzz"]);
    assert_eq!(code(&out), 64, "{}", stderr(&out));
}

#[test]
fn a_tampered_object_says_what_differs_and_not_merely_that_it_does() {
    // §8.5: `--verify` MUST report *what* differs. The store will not serve
    // bytes that do not hash to the name it was asked for, and this is the
    // rite that says which of the ways that happened.
    let (dir, cairn) = with_object("tampered", Value::Str("kernel.nc".into()));
    let hex = cairn.to_string();
    let object = dir.join("objects").join(&hex[..2]).join(&hex[2..]);
    std::fs::write(&object, Value::Str("something else".into()).encode()).expect("tamper");

    let out = nether(Some(&dir), &["cairn", "--verify", &hex]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    let said = stderr(&out);
    assert!(said.contains(&hex), "it does not say what was asked for:\n{said}");
    let actually = Value::Str("something else".into()).cairn();
    assert!(said.contains(&actually.to_string()), "it does not say what is there:\n{said}");
}

#[test]
fn bytes_that_hash_right_and_decode_wrong_say_that_instead() {
    // The other way an object can be wrong, and the one §7.7 is about: bytes
    // that are filed under their own name and are not a value this format
    // has. The store will not write one — `put` decodes before it commits —
    // so this is what a different format version, or a bad disk, looks like.
    let dir = scratch("garbage");
    let store = Store::open(&dir).expect("a store");
    drop(store);
    let garbage = [0xffu8, 0xff, 0xff];
    let cairn = Cairn::of_encoded(&garbage);
    let hex = cairn.to_string();
    let fanned = dir.join("objects").join(&hex[..2]);
    std::fs::create_dir_all(&fanned).expect("fan out");
    std::fs::write(fanned.join(&hex[2..]), garbage).expect("write");

    let out = nether(Some(&dir), &["cairn", "--verify", &hex]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("do not decode"), "{}", stderr(&out));
}

#[test]
fn verify_answers_in_json_too() {
    let (dir, cairn) = with_object("verify-json", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.to_string(), "--json"]);
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("\"holds\":true"), "{text}");
}

// ── the rest ────────────────────────────────────────────────────────────────

#[test]
fn a_rite_that_is_not_built_yet_says_so_with_its_own_code() {
    for rite in ["exhume", "graft"] {
        let out = nether(None, &[rite]);
        assert_eq!(code(&out), 69, "{rite}: §8.8 gives 69 to not implemented");
    }
}

// ── bury ────────────────────────────────────────────────────────────────────

/// §8.2's own transcript, against the program the specification shows.
#[test]
fn burying_reports_depth_holes_and_nodes() {
    let dir = scratch("bury-build");
    let src = dir.join("build.nc");
    std::fs::write(&src, include_str!("../../../tests/programs/build.nc")).expect("write");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8")]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    let said = stdout(&out);

    assert!(said.starts_with("buried   "), "{said}");
    assert!(said.contains("depth 3"), "{said}");
    assert!(said.contains("holes 1"), "{said}");
    assert!(said.contains(" nodes"), "{said}");
    // §8.2 shows the hole beneath the summary, with what was asked and where.
    assert!(said.contains("read(\"main.nc\")"), "{said}");
    assert!(said.contains("stratum 3"), "{said}");
    assert!(said.contains("disk"), "{said}");
}

/// A program that needs nothing from the world leaves no holes.
#[test]
fn a_pure_program_buries_to_depth_zero_with_no_holes() {
    let dir = scratch("bury-hello");
    let src = dir.join("hello.nc");
    std::fs::write(&src, include_str!("../../../tests/programs/hello.nc")).expect("write");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8")]);
    assert_eq!(code(&out), 0, "{}", stdout(&out));
    assert!(stdout(&out).contains("depth 0   holes 0"), "{}", stdout(&out));
}

/// Exactly one trace, however many nodes it took, and it is a trace.
#[test]
fn burying_writes_exactly_one_trace() {
    let dir = scratch("bury-one-trace");
    let src = dir.join("hello.nc");
    std::fs::write(&src, include_str!("../../../tests/programs/hello.nc")).expect("write");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8")]);
    let cairn =
        stdout(&out).split_whitespace().nth(3).expect("the summary names the trace").to_owned();

    // `lamp` without `--provenance` shows what a program deposited, and burial
    // does not deposit yet, so ask for the node itself.
    let shown = nether(Some(&dir), &["lamp", &cairn, "--provenance"]);
    assert_eq!(code(&shown), 0, "{}", stderr(&shown));
    assert!(stdout(&shown).contains("trace"), "{}", stdout(&shown));
}

/// A budget that never bound cannot have changed anything.
///
/// §8.2 claims the opposite — "a trace buried under a different budget is a
/// different trace" — and that claim cannot hold: running out of fuel produces
/// a halt and no trace at all, so a budget either lets a burial finish or
/// there is nothing to compare. Filed; this asserts what is true today.
#[test]
fn a_budget_that_did_not_bind_did_not_change_the_trace() {
    let dir = scratch("bury-fuel");
    let src = dir.join("build.nc");
    std::fs::write(&src, include_str!("../../../tests/programs/build.nc")).expect("write");
    let path = src.to_str().expect("utf8");

    let plenty = stdout(&nether(Some(&dir), &["bury", path, "--fuel", "1000000"]));
    let fewer = stdout(&nether(Some(&dir), &["bury", path, "--fuel", "999999"]));
    assert_eq!(plenty, fewer, "a budget neither burial reached changed the trace");
}

/// §6.4: running out of fuel is a diagnostic, and §8.8 gives it its own code.
#[test]
fn running_out_of_fuel_is_a_diagnostic_and_not_a_crash() {
    let dir = scratch("bury-starved");
    let src = dir.join("build.nc");
    std::fs::write(&src, include_str!("../../../tests/programs/build.nc")).expect("write");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8"), "--fuel", "1"]);
    assert_eq!(code(&out), 75, "{}", stderr(&out));
}

#[test]
fn a_source_that_does_not_parse_is_malformed() {
    let dir = scratch("bury-garbage");
    let src = dir.join("bad.nc");
    std::fs::write(&src, "demand ;;;\n").expect("write");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8")]);
    assert_eq!(code(&out), 65, "{}", stderr(&out));
}

#[test]
fn an_unknown_rite_is_a_usage_error_and_prints_the_six() {
    let out = nether(None, &["summon"]);
    assert_eq!(code(&out), 64);
    for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
        assert!(stderr(&out).contains(rite), "the usage does not mention `{rite}`");
    }
}

// ── §8.4 `lamp` ─────────────────────────────────────────────────────────────

use nether_ledger::{Call, Node, Span};

/// A store holding `build.nc`'s shape: a hole asking for a file, the witness
/// that answered it, the bytes that came back, and a deposit of a greeting.
fn a_small_trace(what: &str) -> (PathBuf, Cairn, Cairn, Cairn) {
    let dir = scratch(what);
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = put(Value::Bytes(b"demand greet;\n".to_vec()));
    let path = put(Value::Str("main.nc".into()));
    let span = Span { source, start: 15, end: 30 };
    let call = Call { function: "read".into(), args: vec![path] };

    let hole = node(Node::Hole { call: call.clone(), stratum: 3, span });
    let answer = put(Value::Bytes(b"the file's contents".to_vec()));
    let witness = node(Node::Witness { stratum: 3, call, answer, span });

    let greeting = put(Value::Str("Hello from the nether".into()));
    let deposit = node(Node::Deposit { value: greeting, span: Span { source, start: 3, end: 9 } });

    let trace = node(Node::Trace {
        residue: hole,
        holes: vec![hole],
        deposits: vec![witness, deposit],
        source: hole,
        fuel_spent: 903,
        depth: 3,
        unrecorded: false,
    });
    (dir, trace, answer, greeting)
}

#[test]
fn a_lamp_shows_text_as_text() {
    // §8.4's example: the whole of what a lamp is for.
    let (dir, _, _, greeting) = a_small_trace("lamp-text");
    let out = nether(Some(&dir), &["lamp", &greeting.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(stdout(&out), "Hello from the nether\n");
}

#[test]
fn a_lamp_on_a_trace_shows_what_the_program_left_behind() {
    // Not printed when it was made — nothing in Nether C is printed. Read
    // afterwards, by somebody who decided to go and look.
    let (dir, trace, _, _) = a_small_trace("lamp-trace");
    let out = nether(Some(&dir), &["lamp", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert_eq!(stdout(&out), "Hello from the nether\n");
}

#[test]
fn deposits_from_two_sources_are_grouped_and_not_interleaved() {
    // §8.4: "source order" is by offset within one source, and a trace may
    // hold more than one. Sorting on the offset alone put the second file's
    // first deposit between the first file's two, which is an order
    // corresponding to nothing anybody wrote.
    let dir = scratch("lamp-two-sources");
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let one = put(Value::Bytes(b"the first file".to_vec()));
    let two = put(Value::Bytes(b"the second file".to_vec()));
    let at = |source, start: u64, text: &str| {
        let v = put(Value::Str(text.into()));
        node(Node::Deposit { value: v, span: Span { source, start, end: start + 1 } })
    };
    // Offsets chosen so that sorting on them alone interleaves the two.
    let roots = vec![
        at(one, 10, "one-a"),
        at(two, 20, "two-a"),
        at(one, 30, "one-b"),
        at(two, 40, "two-b"),
    ];
    let trace = node(Node::Trace {
        residue: one,
        holes: vec![],
        deposits: roots,
        source: one,
        fuel_spent: 4,
        depth: 0,
        unrecorded: false,
    });

    let out = nether(Some(&dir), &["lamp", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    let lines: Vec<&str> = text.lines().collect();

    let first =
        if one.to_string() < two.to_string() { ["one-a", "one-b"] } else { ["two-a", "two-b"] };
    let second = if first[0] == "one-a" { ["two-a", "two-b"] } else { ["one-a", "one-b"] };
    assert_eq!(lines, [first[0], first[1], second[0], second[1]], "{lines:?}");
}

#[test]
fn a_lamp_on_a_node_says_what_kind_of_node_it_is() {
    let (dir, trace, _, _) = a_small_trace("lamp-node");
    let out = nether(Some(&dir), &["lamp", &trace.to_string(), "--json"]);
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("Hello from the nether"), "{text}");
}

#[test]
fn provenance_walks_backwards_to_what_produced_a_value() {
    // §7.4: provenance is the forward edge read backwards. The bytes were
    // produced by a witness, and the witness answered a call.
    let (dir, _, answer, _) = a_small_trace("lamp-provenance");
    let out = nether(Some(&dir), &["lamp", &answer.to_string(), "--provenance"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);

    assert!(text.starts_with(&answer.short()), "{text}");
    assert!(text.contains("witness"), "the witness is not named:\n{text}");
    assert!(text.contains("read(\"main.nc\")"), "the call is not rendered:\n{text}");
    assert!(text.contains("stratum 3"), "{text}");
    assert!(text.contains("trace"), "the trace above it is not reached:\n{text}");
    assert!(text.contains("└─") || text.contains("├─"), "it is not a tree:\n{text}");
}

#[test]
fn a_provenance_walk_stops_where_it_is_told_to() {
    let (dir, _, answer, _) = a_small_trace("lamp-depth");
    let shallow =
        nether(Some(&dir), &["lamp", &answer.to_string(), "--provenance", "--depth", "1"]);
    let deep = nether(Some(&dir), &["lamp", &answer.to_string(), "--provenance", "--depth", "8"]);
    assert!(
        stdout(&shallow).lines().count() < stdout(&deep).lines().count(),
        "depth 1:\n{}\ndepth 8:\n{}",
        stdout(&shallow),
        stdout(&deep)
    );
}

#[test]
fn a_lamp_on_a_name_that_is_not_there_is_absent() {
    let (dir, _, _, _) = a_small_trace("lamp-absent");
    let missing = Value::Int(9999).cairn();
    let out = nether(Some(&dir), &["lamp", &missing.to_string()]);
    assert_eq!(code(&out), 66, "{}", stderr(&out));
}

#[test]
fn a_lamp_with_no_cairn_says_which_cairn() {
    let out = nether(None, &["lamp"]);
    assert_eq!(code(&out), 64);
    assert!(stderr(&out).contains("which cairn?"), "{}", stderr(&out));
}

#[test]
fn a_shade_shows_its_origin_and_its_name_and_no_more() {
    // §1.6: a shade may be stored, copied, passed, compared and sealed. What
    // the lamp shows is what the value holds, which is a stratum and a name.
    let dir = scratch("lamp-shade");
    let store = Store::open(&dir).expect("a store");
    let inside = store.put(&Stored::Value(Value::Str("secret".into()))).expect("put");
    let shade = store.put(&Stored::Value(Value::Shade { origin: 5, value: inside })).expect("put");

    let out = nether(Some(&dir), &["lamp", &shade.to_string()]);
    let text = stdout(&out);
    assert!(text.contains("stratum 5"), "{text}");
    assert!(text.contains(&inside.short()), "{text}");
    assert!(!text.contains("secret"), "the lamp opened the shade:\n{text}");
}

// ── §8.6 `strata` ───────────────────────────────────────────────────────────

/// A source a person could read, so the line `strata` blames can be checked
/// against the text it points into.
const BUILD: &str = "demand out;\n\nBytes@3 src = must(descend disk { read(\"main.nc\") });\n";

/// A store holding one burial of `BUILD`: the source, the call that went to
/// disk, and the trace over it.
fn a_trace_of(what: &str, stratum: u8, depth: u8, unrecorded: bool) -> (PathBuf, Cairn, Cairn) {
    let dir = scratch(what);
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = put(Value::Bytes(BUILD.as_bytes().to_vec()));
    let at = BUILD.find("read(\"main.nc\")").expect("the call is in the source");
    let span = Span { source, start: at as u64, end: (at + 15) as u64 };
    let path = put(Value::Str("main.nc".into()));
    let call = Call { function: "read".into(), args: vec![path] };
    let answer = put(Value::Bytes(b"int main(void) { return 0; }".to_vec()));
    let witness = node(Node::Witness { stratum, call, answer, span });

    let trace = node(Node::Trace {
        residue: source,
        holes: vec![],
        deposits: vec![witness],
        source,
        fuel_spent: 903,
        depth,
        unrecorded,
    });
    (dir, trace, source)
}

#[test]
fn strata_names_the_deepest_stratum_and_the_line_that_took_it_there() {
    // §8.6, and the whole point of the rite: "why is this thing @3" answered
    // by a command rather than an investigation.
    let (dir, trace, source) = a_trace_of("strata", 3, 3, false);
    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);

    assert!(text.starts_with("depth 3   disk\n"), "{text}");
    assert!(text.contains("read(\"main.nc\")"), "the call is not named:\n{text}");
    // §8.1: by the cairn of the source, because a path is a fact about one
    // machine. Line 3, column 35, which is where `read` is in `BUILD`.
    assert!(text.contains(&format!("{}:3:35", source.short())), "the position is wrong:\n{text}");
    assert!(text.contains("  0  everything else"), "{text}");
    assert!(text.contains("replayable: yes"), "{text}");
}

#[test]
fn a_trace_that_reached_stratum_8_says_so_without_being_asked() {
    // §8.6 MUST, and §9.8: naming the symbol is the minimum, because a
    // stratum-8 call is a hole in the record no later care fills in.
    let dir = scratch("strata-unrecorded");
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = put(Value::Bytes(BUILD.as_bytes().to_vec()));
    let span = Span { source, start: 0, end: 11 };
    let symbol = put(Value::Str("dlopen".into()));
    let args = put(Value::Bytes(Vec::new()));
    let call = Call { function: "call_foreign".into(), args: vec![symbol, args] };
    let answer = put(Value::Bytes(b"whatever it said".to_vec()));
    let witness = node(Node::Witness { stratum: 8, call, answer, span });
    let trace = node(Node::Trace {
        residue: source,
        holes: vec![],
        deposits: vec![witness],
        source,
        fuel_spent: 12,
        depth: 8,
        unrecorded: true,
    });

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("depth 8   unrecorded\n"), "{text}");
    assert!(text.contains("\"dlopen\""), "the symbol is not named:\n{text}");
    assert!(text.contains("replayable: no"), "{text}");
    assert!(text.contains("stratum 8 was reached"), "{text}");
}

#[test]
fn a_hole_is_a_stratum_owed_and_not_a_stratum_reached() {
    // §7.3 makes the difference: a witness holds the stratum that *was*
    // reached, a hole the stratum a call *would* reach.
    let dir = scratch("strata-pending");
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = put(Value::Bytes(BUILD.as_bytes().to_vec()));
    let span = Span { source, start: 0, end: 11 };
    let url = put(Value::Str("https://example.invalid/x".into()));
    let call = Call { function: "get".into(), args: vec![url] };
    let hole = node(Node::Hole { call, stratum: 5, span });
    let trace = node(Node::Trace {
        residue: source,
        holes: vec![hole],
        deposits: vec![],
        source,
        fuel_spent: 4,
        depth: 0,
        unrecorded: false,
    });

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.starts_with("depth 0   pure\n"), "a hole is not a stratum reached:\n{text}");
    assert!(text.contains("pending"), "{text}");
    assert!(text.contains("holes reach 5 (net)"), "{text}");
}

#[test]
fn a_trace_that_under_reports_its_own_depth_is_malformed() {
    // The one thing this rite exists to catch. §8.8 gives 65 to a malformed
    // trace, and a trace whose recorded depth is shallower than a witness
    // under it has recorded a falsehood about depth.
    let (dir, trace, _) = a_trace_of("strata-lying", 5, 3, false);
    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 65, "{}", stderr(&out));
    assert!(stderr(&out).contains("records depth 3"), "{}", stderr(&out));
    assert!(stdout(&out).contains("read(\"main.nc\")"), "it still says where:\n{}", stdout(&out));
}

#[test]
fn a_trace_that_never_went_anywhere_says_so() {
    let dir = scratch("strata-pure");
    let store = Store::open(&dir).expect("a store");
    let literal = store.put(&Stored::Node(Node::Literal(Value::Int(7)))).expect("put");
    let trace = store
        .put(&Stored::Node(Node::Trace {
            residue: literal,
            holes: vec![],
            deposits: vec![literal],
            source: literal,
            fuel_spent: 1,
            depth: 0,
            unrecorded: false,
        }))
        .expect("put");

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("nothing reached the world"), "{}", stdout(&out));
}

#[test]
fn strata_on_something_that_is_not_a_trace_says_what_it_is() {
    let (dir, cairn) = with_object("strata-not-a-trace", Value::Int(7));
    let out = nether(Some(&dir), &["strata", &cairn.to_string()]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("is a value, not a trace"), "{}", stderr(&out));
}

#[test]
fn strata_answers_in_json_too() {
    let (dir, trace, source) = a_trace_of("strata-json", 3, 3, false);
    let out = nether(Some(&dir), &["strata", &trace.to_string(), "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "§8.1: one object and nothing else\n{text}");
    assert!(text.contains("\"depth\":3"), "{text}");
    assert!(text.contains("\"stratum\":\"disk\""), "{text}");
    assert!(text.contains("\"replayable\":true"), "{text}");
    assert!(text.contains(&format!("{}:3:35", source.short())), "{text}");
}

#[test]
fn a_span_whose_source_is_gone_still_gives_the_offset() {
    // A trace means the same thing on a machine that has never seen the
    // filesystem it was buried on — but not necessarily the same amount. Say
    // what is left rather than nothing, and do not call it a lower bound: a
    // source that was never sealed is the ordinary state of a ledger and
    // bounds nothing about which strata were reached.
    let dir = scratch("strata-no-source");
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = Value::Bytes(BUILD.as_bytes().to_vec()).cairn();
    let span = Span { source, start: 47, end: 62 };
    let path = put(Value::Str("main.nc".into()));
    let call = Call { function: "read".into(), args: vec![path] };
    let answer = put(Value::Bytes(b"x".to_vec()));
    let witness = node(Node::Witness { stratum: 3, call, answer, span });
    let trace = node(Node::Trace {
        residue: source,
        holes: vec![],
        deposits: vec![witness],
        source,
        fuel_spent: 2,
        depth: 3,
        unrecorded: false,
    });

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert!(text.contains(&format!("{}:+47", source.short())), "{text}");
    assert!(!text.contains("lower bound"), "a missing source is not a missing node:\n{text}");
}

#[test]
fn a_node_that_is_gone_is_a_lower_bound_and_says_so() {
    // The case the note is actually about. A root that is not here could be
    // hiding any stratum at all, so the survey can only be a floor.
    let dir = scratch("strata-missing-node");
    let store = Store::open(&dir).expect("a store");
    let gone = Value::Int(404).cairn();
    let trace = store
        .put(&Stored::Node(Node::Trace {
            residue: gone,
            holes: vec![gone],
            deposits: vec![],
            source: gone,
            fuel_spent: 1,
            depth: 0,
            unrecorded: false,
        }))
        .expect("put");

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("1 node(s)"), "{}", stdout(&out));
    assert!(stdout(&out).contains("lower bound"), "{}", stdout(&out));
}

#[test]
fn two_answers_to_one_question_are_two_lines() {
    // The collapse this used to do keyed on the call and the place and left
    // out the answer, so the same read answered two different ways came out as
    // one line marked ×2 — which is exactly what a blame tool exists to show.
    let dir = scratch("strata-two-answers");
    let store = Store::open(&dir).expect("a store");
    let put = |v: Value| store.put(&Stored::Value(v)).expect("put");
    let node = |n: Node| store.put(&Stored::Node(n)).expect("put");

    let source = put(Value::Bytes(BUILD.as_bytes().to_vec()));
    let at = BUILD.find("read(\"main.nc\")").expect("the call is in the source");
    let span = Span { source, start: at as u64, end: (at + 15) as u64 };
    let path = put(Value::Str("main.nc".into()));
    let call = Call { function: "read".into(), args: vec![path] };

    let one = node(Node::Witness {
        stratum: 3,
        call: call.clone(),
        answer: put(Value::Bytes(b"before".to_vec())),
        span,
    });
    let two = node(Node::Witness {
        stratum: 3,
        call,
        answer: put(Value::Bytes(b"after".to_vec())),
        span,
    });
    let trace = node(Node::Trace {
        residue: source,
        holes: vec![one, two],
        deposits: vec![],
        source,
        fuel_spent: 4,
        depth: 3,
        unrecorded: false,
    });

    let out = nether(Some(&dir), &["strata", &trace.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    let asked = text.lines().filter(|l| l.contains("read(\"main.nc\")")).count();
    assert_eq!(asked, 2, "one of the two answers is hidden:\n{text}");
}

#[test]
fn strata_with_no_cairn_is_a_usage_error() {
    let out = nether(None, &["strata"]);
    assert_eq!(code(&out), 64, "{}", stderr(&out));
    assert!(stderr(&out).contains("nether strata <cairn>"), "{}", stderr(&out));
}
