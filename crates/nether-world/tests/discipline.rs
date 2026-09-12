//! The proof for *the provider trait and the recording discipline*: no
//! provider can return a value it has not first written to the ledger.
//!
//! §1.4 states the order — the witness is written before the value reaches the
//! program — and says why: "an answer that is returned before it is recorded is
//! an answer that can be lost". What is checked here is that the order is not
//! something a provider can get wrong. It returns a `Recorded`, the only thing
//! that makes one writes first, and there is no other constructor.
//!
//! The compiler checks the last part. See `Recorded`'s `compile_fail` example.

use nether_core::{Capability, Depth};
use nether_ledger::{Cairn, Call, Span, Store, StoreError, Stored, Value};
use nether_world::{Provider, Recorded, Recorder, World};

fn scratch(what: &str) -> std::path::PathBuf {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nether-world-{what}-{at}"));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn source() -> Cairn {
    Cairn::of_encoded(b"a source")
}

fn span() -> Span {
    Span { source: source(), start: 0, end: 1 }
}

fn asking(function: &str) -> Call {
    Call { function: function.to_owned(), args: Vec::new() }
}

/// A provider that answers, which is the ordinary case.
struct Says(Value);

impl Provider for Says {
    fn capability(&self) -> Capability {
        Capability::Disk
    }
    fn answers(&self, function: &str) -> bool {
        function == "read"
    }
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, StoreError> {
        into.record(call, Depth::DISK, span, &self.0)
    }
}

#[test]
fn what_a_provider_returns_is_already_in_the_ledger() {
    let dir = scratch("recorded");
    let store = Store::open(&dir).expect("a store");
    let said = Value::Bytes(b"int main(void) { return 0; }".to_vec());
    let world = World::sealed().granting(Box::new(Says(said.clone())));

    let recorded = world
        .ask(&asking("read"), span(), &Recorder::new(&store, source()))
        .expect("disk was granted");

    // The answer is in the ledger, and it is what the world said.
    assert_eq!(store.get(recorded.answer()).expect("written"), Stored::Value(said));
    // And so is the witness that names it, at the stratum §1.1 gives disk.
    let nether_ledger::Stored::Node(nether_ledger::Node::Witness { stratum, answer, .. }) =
        store.get(recorded.witness()).expect("written")
    else {
        panic!("a witness is not a witness")
    };
    assert_eq!(stratum, 3);
    assert_eq!(answer, recorded.answer());
}

/// §9.9: a refusal is an ordinary witness — recorded, served back on replay,
/// and not a mark on the trace.
struct Refuses;

impl Provider for Refuses {
    fn capability(&self) -> Capability {
        Capability::Disk
    }
    fn answers(&self, function: &str) -> bool {
        function == "read"
    }
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, StoreError> {
        let no = Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(
            nether_ledger::Refusal::Absent,
        )));
        into.record(call, Depth::DISK, span, &no)
    }
}

#[test]
fn a_refusal_is_written_down_like_any_other_answer() {
    let dir = scratch("refusal");
    let store = Store::open(&dir).expect("a store");
    let world = World::sealed().granting(Box::new(Refuses));

    let recorded = world
        .ask(&asking("read"), span(), &Recorder::new(&store, source()))
        .expect("a refusal is an answer, not an error");
    assert!(matches!(
        store.get(recorded.answer()).expect("written"),
        Stored::Value(Value::Answer(_))
    ));
}

#[test]
fn a_world_with_nothing_granted_answers_nothing() {
    // §9.1: a capability is acquired by `descend`, never ambient. This is what
    // makes "no grant" mean nothing rather than mean everything.
    let dir = scratch("sealed");
    let store = Store::open(&dir).expect("a store");
    let world = World::sealed();

    let refused = world
        .ask(&asking("read"), span(), &Recorder::new(&store, source()))
        .expect_err("nothing was granted");
    assert!(refused.to_string().contains("descend disk"), "{refused}");
    assert_eq!(world.depth(), Depth::PURE);
    assert!(!world.holds(Capability::Disk));
}

#[test]
fn a_grant_answers_only_what_it_is_for() {
    let dir = scratch("only");
    let store = Store::open(&dir).expect("a store");
    let world = World::sealed().granting(Box::new(Says(Value::Unit)));

    assert!(world.holds(Capability::Disk));
    assert_eq!(world.depth(), Depth::DISK);
    // `get` is stratum 5, and disk does not answer it.
    let refused = world
        .ask(&asking("get"), span(), &Recorder::new(&store, source()))
        .expect_err("net was not granted");
    assert!(refused.to_string().contains("descend net"), "{refused}");
}

/// A provider that writes one thing and answers with another has still
/// written: the discipline is about the order, and the order holds whatever it
/// records. What it cannot do is answer with something that is nowhere.
struct Slippery;

impl Provider for Slippery {
    fn capability(&self) -> Capability {
        Capability::Disk
    }
    fn answers(&self, function: &str) -> bool {
        function == "read"
    }
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, StoreError> {
        let _ = into.record(call, Depth::DISK, span, &Value::Int(1))?;
        into.record(call, Depth::DISK, span, &Value::Int(2))
    }
}

#[test]
fn whatever_a_provider_hands_back_was_written_first() {
    let dir = scratch("slippery");
    let store = Store::open(&dir).expect("a store");
    let world = World::sealed().granting(Box::new(Slippery));

    let recorded =
        world.ask(&asking("read"), span(), &Recorder::new(&store, source())).expect("granted");
    assert_eq!(store.get(recorded.answer()).expect("written"), Stored::Value(Value::Int(2)));
}
