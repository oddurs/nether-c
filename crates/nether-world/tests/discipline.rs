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
use nether_ledger::{Cairn, Call, Span, Store, Stored, Value};
use nether_world::{Provider, Recorded, Recorder, Refuse, World};

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
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        into.record(call, Depth::DISK, span, &self.0)
    }
}

#[test]
fn what_a_provider_returns_is_already_in_the_ledger() {
    let dir = scratch("recorded");
    let store = Store::open(&dir).expect("a store");
    let said = Value::Bytes(b"int main(void) { return 0; }".to_vec());
    let world = World::sealed().granting(Box::new(Says(said.clone())));

    let recorded =
        world.ask(&asking("read"), span(), &Recorder::new(&store)).expect("disk was granted");

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
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
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
        .ask(&asking("read"), span(), &Recorder::new(&store))
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
        .ask(&asking("read"), span(), &Recorder::new(&store))
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
    let refused =
        world.ask(&asking("get"), span(), &Recorder::new(&store)).expect_err("net was not granted");
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
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        let _ = into.record(call, Depth::DISK, span, &Value::Int(1))?;
        into.record(call, Depth::DISK, span, &Value::Int(2))
    }
}

#[test]
fn whatever_a_provider_hands_back_was_written_first() {
    let dir = scratch("slippery");
    let store = Store::open(&dir).expect("a store");
    let world = World::sealed().granting(Box::new(Slippery));

    let recorded = world.ask(&asking("read"), span(), &Recorder::new(&store)).expect("granted");
    assert_eq!(store.get(recorded.answer()).expect("written"), Stored::Value(Value::Int(2)));
}

struct OutOfOrder {
    together: std::sync::Barrier,
    done_before_first: std::sync::atomic::AtomicUsize,
    events: std::sync::Mutex<Vec<String>>,
}

impl Provider for OutOfOrder {
    fn capability(&self) -> Capability {
        Capability::Disk
    }
    fn answers(&self, function: &str) -> bool {
        matches!(function, "read" | "write")
    }
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        use std::sync::atomic::Ordering::SeqCst;
        let tag = span.start;
        let num = i64::try_from(tag).unwrap_or(0);
        if call.function == "write" {
            self.events.lock().expect("lock").push(format!("write:{tag}"));
            return into.record(call, Depth::DISK_WRITE, span, &Value::Int(num));
        }
        if tag < 3 {
            self.together.wait();
            if tag == 0 {
                while self.done_before_first.load(SeqCst) < 2 {
                    std::thread::yield_now();
                }
            }
        }
        let said = if tag == 2 {
            Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(
                nether_ledger::Refusal::Absent,
            )))
        } else {
            Value::Int(num)
        };
        let rec = into.record(call, Depth::DISK, span, &said)?;
        self.events.lock().expect("lock").push(format!("read:{tag}"));
        if tag == 1 || tag == 2 {
            self.done_before_first.fetch_add(1, SeqCst);
        }
        Ok(rec)
    }
}

#[test]
fn many_holes_answer_concurrently_and_keep_discovery_order() {
    struct Shared(std::sync::Arc<OutOfOrder>);
    impl Provider for Shared {
        fn capability(&self) -> Capability {
            self.0.capability()
        }
        fn answers(&self, function: &str) -> bool {
            self.0.answers(function)
        }
        fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
            self.0.answer(call, span, into)
        }
    }
    let dir = scratch("concurrent-order");
    let store = Store::open(&dir).expect("a store");
    let state = std::sync::Arc::new(OutOfOrder {
        together: std::sync::Barrier::new(3),
        done_before_first: std::sync::atomic::AtomicUsize::new(0),
        events: std::sync::Mutex::new(Vec::new()),
    });
    let world = World::sealed().granting(Box::new(Shared(state.clone())));
    let at = |start: u64| Span { source: source(), start, end: start + 1 };
    let asked = vec![
        (asking("read"), at(0)),
        (asking("read"), at(1)),
        (asking("read"), at(2)),
        (asking("write"), at(10)),
        (asking("read"), at(3)),
    ];
    let out = world.ask_all(&asked, &Recorder::new(&store));
    assert_eq!(out.len(), 5);
    let spans: Vec<u64> = out
        .iter()
        .map(|r| {
            let rec = r.as_ref().expect("recorded");
            let Ok(Stored::Node(nether_ledger::Node::Witness { span, .. })) =
                store.get(rec.witness())
            else {
                panic!("not a witness");
            };
            span.start
        })
        .collect();
    // Holes 1 and 2 finished and recorded before hole 0, yet the returned
    // witnesses are in the exact discovery order [0, 1, 2, 10, 3].
    assert_eq!(spans, vec![0, 1, 2, 10, 3]);
    let events = state.events.lock().expect("lock").clone();
    let zero_pos = events.iter().position(|e| e == "read:0").expect("read:0");
    let one_pos = events.iter().position(|e| e == "read:1").expect("read:1");
    let two_pos = events.iter().position(|e| e == "read:2").expect("read:2");
    let write_pos = events.iter().position(|e| e == "write:10").expect("write:10");
    let three_pos = events.iter().position(|e| e == "read:3").expect("read:3");
    assert!(one_pos < zero_pos && two_pos < zero_pos, "{events:?}");
    assert!(zero_pos < write_pos && write_pos < three_pos, "{events:?}");
}

#[test]
fn concurrency_is_bounded_and_collapsing_cancels_later_holes() {
    use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering::SeqCst};
    struct Bounded {
        active: AtomicUsize,
        peak: AtomicUsize,
        second_failed: AtomicBool,
        after_cutoff: AtomicUsize,
    }
    impl Provider for Bounded {
        fn capability(&self) -> Capability {
            Capability::Disk
        }
        fn answers(&self, function: &str) -> bool {
            function == "read"
        }
        fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
            let cur = self.active.fetch_add(1, SeqCst) + 1;
            self.peak.fetch_max(cur, SeqCst);
            let tag = span.start;
            let num = Value::Int(i64::try_from(tag).unwrap_or(0));
            let res = if tag >= 100 {
                std::thread::sleep(std::time::Duration::from_millis(10));
                into.record(call, Depth::DISK, span, &num)
            } else if tag == 2 {
                self.second_failed.store(true, SeqCst);
                Err(Refuse::Collapse("second collapsed first".to_owned()))
            } else if tag == 1 {
                while !self.second_failed.load(SeqCst) {
                    std::thread::yield_now();
                }
                Err(Refuse::Collapse("first collapsed second".to_owned()))
            } else if tag < 8 {
                while !self.second_failed.load(SeqCst) {
                    std::thread::yield_now();
                }
                std::thread::sleep(std::time::Duration::from_millis(15));
                into.record(call, Depth::DISK, span, &num)
            } else {
                self.after_cutoff.fetch_add(1, SeqCst);
                into.record(call, Depth::DISK, span, &num)
            };
            self.active.fetch_sub(1, SeqCst);
            res
        }
    }
    struct Ref(std::sync::Arc<Bounded>);
    impl Provider for Ref {
        fn capability(&self) -> Capability {
            self.0.capability()
        }
        fn answers(&self, function: &str) -> bool {
            self.0.answers(function)
        }
        fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
            self.0.answer(call, span, into)
        }
    }
    let dir = scratch("bounded");
    let store = Store::open(&dir).expect("a store");
    let state = std::sync::Arc::new(Bounded {
        active: AtomicUsize::new(0),
        peak: AtomicUsize::new(0),
        second_failed: AtomicBool::new(false),
        after_cutoff: AtomicUsize::new(0),
    });
    let world = World::sealed().granting(Box::new(Ref(state.clone())));
    let at = |start: u64| Span { source: source(), start, end: start + 1 };

    let sixteen: Vec<(Call, Span)> = (100..116).map(|n| (asking("read"), at(n))).collect();
    let ok = world.ask_all(&sixteen, &Recorder::new(&store));
    assert_eq!(ok.len(), 16);
    let peak = state.peak.load(SeqCst);
    assert!((2..=8).contains(&peak), "peak in-flight was {peak}");

    let collapsing: Vec<(Call, Span)> = (0..12).map(|n| (asking("read"), at(n))).collect();
    let halted = world.ask_all(&collapsing, &Recorder::new(&store));
    assert_eq!(halted.len(), 2, "{halted:?}");
    assert!(halted[0].is_ok());
    assert_eq!(
        halted[1],
        Err(nether_world::Unanswered::Collapsed("first collapsed second".to_owned()))
    );
    assert_eq!(state.after_cutoff.load(SeqCst), 0, "holes past the in-flight window ran");
}
