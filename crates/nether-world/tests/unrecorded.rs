//! The proof for *the world: stratum 8*.
//!
//! `spec/09-prelude.md` §9.8 and §9.8.1, and §1.7 for what it costs. The
//! object under test is `tests/foreign/said.c`, built by `tests/foreign/build`
//! — real foreign code rather than a mock, because a mocked stratum 8 proves
//! nothing about the one thing this stratum is: a call the implementation
//! cannot see inside.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nether_core::{Capability, Depth};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, Store, Stored, Stratum, Value};
use nether_world::{Provider, Recorder, Unanswered, Unrecorded, World};

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let dir = std::env::temp_dir().join(format!(
            "nether-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir_all(&dir).expect("a scratch directory");
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

struct BuiltObject {
    path: String,
    _dir: Scratch,
}

impl Deref for BuiltObject {
    type Target = str;

    fn deref(&self) -> &str {
        &self.path
    }
}

/// Build the shared object into a directory this test alone owns.
///
/// Named after the caller, because cargo runs these in parallel and `cc`
/// writing a file while another test's `dlopen` reads it is `file too short`
/// on one machine and green on another. 0176.
///
/// `cc` is not a dependency of this repository; it is how a C file becomes a
/// shared object, and a test about calling C that could not compile any would
/// be testing something else.
fn object(what: &str) -> Option<BuiltObject> {
    let at = std::env::var("CARGO_MANIFEST_DIR").ok()?;
    let build = PathBuf::from(&at).join("../../tests/foreign/build");
    let out = Scratch::new(&format!("foreign-proof-{what}"));
    let said = std::process::Command::new(&build).arg(&*out).output().ok()?;
    if !said.status.success() {
        return None;
    }
    let path = String::from_utf8(said.stdout).ok()?.trim().to_owned();
    Some(BuiltObject { path, _dir: out })
}

struct Asked {
    store: Store,
    world: World,
    _dir: Scratch,
}

impl Asked {
    fn loading(what: &str, paths: &[&str]) -> Self {
        let root = Scratch::new(&format!("foreign-{what}"));
        let store = Store::open(root.join("ledger")).expect("a store");
        let owned: Vec<String> = paths.iter().map(|p| (*p).to_owned()).collect();
        let foreign: Box<dyn Provider> =
            Box::new(Unrecorded::loading(&owned).expect("the object loads"));
        Self { store, world: World::sealed().granting(foreign), _dir: root }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    fn call(&self, symbol: &str, args: &[u8]) -> Result<AnswerOf, Unanswered> {
        let call = Call::try_new(
            "call_foreign",
            vec![self.arg(Value::Str(symbol.into())), self.arg(Value::Bytes(args.to_vec()))],
        )
        .unwrap();
        let span = Span::new(self.arg(Value::Bytes(b"x.nc".to_vec())), 0, 1).unwrap();
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store))?;

        let Ok(Stored::Node(Node::Witness { stratum, answer, .. })) =
            self.store.get(recorded.witness())
        else {
            panic!("no witness")
        };
        assert_eq!(stratum, Stratum::EIGHT, "§9.8 puts it at 8");
        assert_eq!(answer, recorded.answer(), "the witness names something else");
        match self.store.get(recorded.answer()) {
            Ok(Stored::Value(Value::Answer(a))) => Ok(*a),
            other => panic!("not an answer: {other:?}"),
        }
    }
}

#[test]
fn foreign_code_is_called_and_what_it_said_is_recorded() {
    let Some(path) = object("calls") else { return };
    let a = Asked::loading("calls", &[&path]);
    assert_eq!(
        a.call("said", b"hello there").expect("granted"),
        AnswerOf::Given(Value::Bytes(b"HELLO THERE".to_vec()))
    );
    assert_eq!(a.world.depth(), Depth::UNRECORDED);
    assert!(a.world.holds(Capability::Unrecorded));
    assert!(!a.world.holds(Capability::Entropy), "calling out is drawing random bytes");
}

#[test]
fn the_attempt_is_written_before_the_call_without_polluting_objects() {
    // §9.8.1: an attempt record is written in `tmp/` before entering foreign
    // code so a crash leaves an auditable record, and removed on clean return
    // so successful calls never pollute `objects/` or `refs/` with a phantom
    // `Refused(Unreachable)` witness (M12).
    let Some(path) = object("attempt") else { return };
    let a = Asked::loading("attempt", &[&path]);
    a.call("said", b"x").expect("granted");

    let call = Call::try_new(
        "call_foreign",
        vec![a.arg(Value::Str("said".into())), a.arg(Value::Bytes(b"x".to_vec()))],
    )
    .unwrap();
    let span = Span::new(a.arg(Value::Bytes(b"x.nc".to_vec())), 0, 1).unwrap();
    let phantom = Cairn::of_encoded(
        &Stored::Node(Node::Witness {
            stratum: Stratum::EIGHT,
            call: call.clone(),
            answer: a.arg(Value::Answer(Box::new(AnswerOf::Refused(Refusal::Unreachable)))),
            span,
        })
        .encode(),
    );
    assert!(
        !a.store.has(phantom),
        "successful foreign call must not leave a phantom Refused(Unreachable) witness in objects/"
    );

    // Direct `begin_attempt` verification: file exists on disk until guard drop.
    let guard = a.store.begin_attempt(b"attempt-record").expect("attempt file");
    let attempt_path = guard.path().to_path_buf();
    assert!(attempt_path.is_file(), "attempt record missing before drop");
    drop(guard);
    assert!(!attempt_path.exists(), "attempt record not cleaned up after drop");
}

#[test]
fn a_callee_that_asks_twice_for_more_room_is_not_asked_a_third_time() {
    // §9.8.1. `greedy` always wants one more byte than it was given, so a
    // caller that kept obliging would never stop.
    let Some(path) = object("greedy") else { return };
    let a = Asked::loading("greedy", &[&path]);
    let no = a.call("greedy", b"x").expect_err("not an answer");
    let Unanswered::Unavailable(why) = no else { panic!("{no:?}") };
    assert!(why.contains("twice"), "{why}");
}

#[test]
fn a_return_value_is_the_refusal_section_five_gives_it() {
    // §9.8.1's table: 3 is `denied`, in the order §5.1.1 lists them.
    let Some(path) = object("refuses") else { return };
    let a = Asked::loading("refuses", &[&path]);
    assert_eq!(a.call("forbidden", b"").expect("granted"), AnswerOf::Refused(Refusal::Denied));
}

#[test]
fn a_symbol_in_nothing_loaded_is_denied() {
    // §8.3.3: `--grant unrecorded` says which stratum and `--load` says what,
    // and a symbol found in nothing named is denied rather than looked for on
    // the machine.
    let Some(path) = object("absent") else { return };
    let a = Asked::loading("absent", &[&path]);
    assert_eq!(
        a.call("nothing_defines_this", b"").expect("granted"),
        AnswerOf::Refused(Refusal::Denied)
    );
    // Transitive symbols from `libc` (via DT_NEEDED) must also be denied.
    assert_eq!(a.call("system", b"true\0").expect("granted"), AnswerOf::Refused(Refusal::Denied));
    assert_eq!(
        a.call("unlink", b"/nonexistent\0").expect("granted"),
        AnswerOf::Refused(Refusal::Denied)
    );

    // And loading nothing is a grant that answers nothing, which §8.3.3 calls
    // the one case where the useless invocation is also the safe one.
    let b = Asked::loading("empty", &[]);
    assert_eq!(b.call("said", b"x").expect("granted"), AnswerOf::Refused(Refusal::Denied));
}

/// Calls into one object take turns, whichever threads make them. The type
/// says so now: `Object` is `Sync` because of the lock in it, not because
/// callers agree not to share it.
#[test]
fn calls_into_one_object_take_turns_across_threads() {
    let Some(path) = object("turns") else { return };
    let loaded = nether_foreign::Object::open(Path::new(&*path)).expect("loads");
    let overlapped = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                for _ in 0..25 {
                    let (code, _) = loaded.call("alone", b"").expect("has alone");
                    if code != 0 {
                        overlapped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                    }
                }
            });
        }
    });
    assert_eq!(overlapped.into_inner(), 0, "two calls were inside the object at once");
}

#[test]
fn duplicate_open_handles_still_serialize_concurrent_calls() {
    let Some(path) = object("turns-dup") else { return };
    let first = nether_foreign::Object::open(Path::new(&*path)).expect("loads first");
    let second = nether_foreign::Object::open(Path::new(&*path)).expect("loads second");
    let overlapped = std::sync::atomic::AtomicUsize::new(0);
    std::thread::scope(|s| {
        for handle in [&first, &second] {
            for _ in 0..4 {
                s.spawn(|| {
                    for _ in 0..25 {
                        let (code, _) = handle.call("alone", b"").expect("has alone");
                        if code != 0 {
                            overlapped.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                        }
                    }
                });
            }
        }
    });
    assert_eq!(overlapped.into_inner(), 0, "two handles to the same shared object overlapped");
}

#[test]
fn atomic_replacement_of_loaded_object_preserves_symbol_ownership() {
    let Some(built) = object("replace") else { return };
    let dir = Scratch::new("foreign-replace");
    let live = dir.join("liblive.so");
    let replacement = dir.join("liblive.so.new");
    std::fs::copy(&*built, &live).unwrap();

    let loaded = nether_foreign::Object::open(&live).expect("opens initial copy");
    // Atomically replace the on-disk file with a fresh inode.
    std::fs::copy(&*built, &replacement).unwrap();
    std::fs::rename(&replacement, &live).unwrap();

    assert!(loaded.has("said"), "symbol ownership must survive atomic .so replacement");
    assert!(!loaded.has("system"), "DT_NEEDED symbol must remain quarantined");
    let (code, out) = loaded.call("said", b"ok").expect("call succeeds after replacement");
    assert_eq!((code, out), (0, b"OK".to_vec()));
}
