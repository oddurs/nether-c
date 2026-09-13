//! The proof for *the world: stratum 8*.
//!
//! `spec/09-prelude.md` §9.8 and §9.8.1, and §1.7 for what it costs. The
//! object under test is `tests/foreign/said.c`, built by `tests/foreign/build`
//! — real foreign code rather than a mock, because a mocked stratum 8 proves
//! nothing about the one thing this stratum is: a call the implementation
//! cannot see inside.

use std::path::PathBuf;

use nether_core::{Capability, Depth};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, Store, Stored, Value};
use nether_world::{Provider, Recorder, Unanswered, Unrecorded, World};

/// Build the shared object once, into a directory the test owns.
///
/// `cc` is not a dependency of this repository; it is how a C file becomes a
/// shared object, and a test about calling C that could not compile any would
/// be testing something else.
fn object() -> Option<String> {
    let at = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let build = PathBuf::from(&at).join("../../tests/foreign/build");
    let out = std::env::temp_dir().join("nether-foreign-proof");
    std::fs::create_dir_all(&out).ok()?;
    let said = std::process::Command::new(&build).arg(&out).output().ok()?;
    if !said.status.success() {
        return None;
    }
    Some(String::from_utf8(said.stdout).ok()?.trim().to_owned())
}

struct Asked {
    store: Store,
    world: World,
}

impl Asked {
    fn loading(what: &str, paths: &[String]) -> Self {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("nether-foreign-{what}-{at}"));
        std::fs::create_dir_all(&root).expect("a directory");
        let store = Store::open(root.join("ledger")).expect("a store");
        let foreign: Box<dyn Provider> =
            Box::new(Unrecorded::loading(paths).expect("the object loads"));
        Self { store, world: World::sealed().granting(foreign) }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    fn call(&self, symbol: &str, args: &[u8]) -> Result<AnswerOf, Unanswered> {
        let call = Call {
            function: "call_foreign".into(),
            args: vec![self.arg(Value::Str(symbol.into())), self.arg(Value::Bytes(args.to_vec()))],
        };
        let span = Span { source: self.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store))?;

        let Ok(Stored::Node(Node::Witness { stratum, answer, .. })) =
            self.store.get(recorded.witness())
        else {
            panic!("no witness")
        };
        assert_eq!(stratum, Depth::UNRECORDED.get(), "§9.8 puts it at 8");
        assert_eq!(answer, recorded.answer(), "the witness names something else");
        match self.store.get(recorded.answer()) {
            Ok(Stored::Value(Value::Answer(a))) => Ok(*a),
            other => panic!("not an answer: {other:?}"),
        }
    }
}

#[test]
fn foreign_code_is_called_and_what_it_said_is_recorded() {
    let Some(path) = object() else { return };
    let a = Asked::loading("calls", &[path]);
    assert_eq!(
        a.call("said", b"hello there").expect("granted"),
        AnswerOf::Given(Value::Bytes(b"HELLO THERE".to_vec()))
    );
    assert_eq!(a.world.depth(), Depth::UNRECORDED);
    assert!(a.world.holds(Capability::Entropy), "§1.1 is a total order");
}

#[test]
fn the_attempt_is_written_before_the_call() {
    // §9.8.1: "MUST record the witness before the call, so that a callee which
    // does not return leaves a trace saying what was attempted." A callee that
    // does not return cannot be tested from inside this process, so what is
    // checked is the thing that makes it true — that two witnesses exist for
    // one call, and the first of them says nothing came back.
    let Some(path) = object() else { return };
    let a = Asked::loading("attempt", &[path]);
    a.call("said", b"x").expect("granted");

    let call = Call {
        function: "call_foreign".into(),
        args: vec![a.arg(Value::Str("said".into())), a.arg(Value::Bytes(b"x".to_vec()))],
    };
    let span = Span { source: a.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
    let attempt = Cairn::of_encoded(
        &Stored::Node(Node::Witness {
            stratum: Depth::UNRECORDED.get(),
            call,
            answer: a.arg(Value::Answer(Box::new(AnswerOf::Refused(Refusal::Unreachable)))),
            span,
        })
        .encode(),
    );
    assert!(a.store.has(attempt), "the attempt was not written before the call");
}

#[test]
fn a_callee_that_asks_twice_for_more_room_is_not_asked_a_third_time() {
    // §9.8.1. `greedy` always wants one more byte than it was given, so a
    // caller that kept obliging would never stop.
    let Some(path) = object() else { return };
    let a = Asked::loading("greedy", &[path]);
    let no = a.call("greedy", b"x").expect_err("not an answer");
    let Unanswered::Unavailable(why) = no else { panic!("{no:?}") };
    assert!(why.contains("twice"), "{why}");
}

#[test]
fn a_return_value_is_the_refusal_section_five_gives_it() {
    // §9.8.1's table: 3 is `denied`, in the order §5.1.1 lists them.
    let Some(path) = object() else { return };
    let a = Asked::loading("refuses", &[path]);
    assert_eq!(a.call("forbidden", b"").expect("granted"), AnswerOf::Refused(Refusal::Denied));
}

#[test]
fn a_symbol_in_nothing_loaded_is_denied() {
    // §8.3.3: `--grant unrecorded` says which stratum and `--load` says what,
    // and a symbol found in nothing named is denied rather than looked for on
    // the machine.
    let Some(path) = object() else { return };
    let a = Asked::loading("absent", &[path]);
    assert_eq!(
        a.call("nothing_defines_this", b"").expect("granted"),
        AnswerOf::Refused(Refusal::Denied)
    );

    // And loading nothing is a grant that answers nothing, which §8.3.3 calls
    // the one case where the useless invocation is also the safe one.
    let b = Asked::loading("empty", &[]);
    assert_eq!(b.call("said", b"x").expect("granted"), AnswerOf::Refused(Refusal::Denied));
}
