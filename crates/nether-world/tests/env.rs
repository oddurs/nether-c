//! The proof for *the world: strata 1 and 2*.
//!
//! `spec/09-prelude.md` §9.3 and §9.4, and §8.3.1 for where the declaration
//! comes from. The discipline itself is `discipline.rs`; this is about what
//! the two providers say.

use nether_core::Depth;
use nether_ledger::{AnswerOf, Cairn, Call, Node, Span, Store, Stored, Value};
use nether_world::{Declared, Env, Ledger, Provider, Recorder, World};

/// A ledger nothing else is using, and a world granted one capability.
struct Asked {
    store: Store,
    world: World,
}

impl Asked {
    fn granting(what: &str, provider: Box<dyn Provider>) -> Self {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("nether-world-{what}-{at}"));
        std::fs::create_dir_all(&root).expect("a directory");
        let store = Store::open(root.join("ledger")).expect("a store");
        Self { store, world: World::sealed().granting(provider) }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    /// What was recorded, exactly as the program would receive it.
    fn said(&self, function: &str, args: Vec<Cairn>) -> Value {
        let call = Call { function: function.to_owned(), args };
        let span = Span { source: self.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store)).expect("granted");

        // §1.4: the witness names the answer, and both were written first.
        let Ok(Stored::Node(Node::Witness { answer, .. })) = self.store.get(recorded.witness())
        else {
            panic!("no witness")
        };
        assert_eq!(answer, recorded.answer(), "the witness names something else");
        match self.store.get(recorded.answer()) {
            Ok(Stored::Value(v)) => v,
            other => panic!("not a value: {other:?}"),
        }
    }
}

fn given(v: Value) -> Value {
    Value::Answer(Box::new(AnswerOf::Given(v)))
}

// ── §9.3 store ──────────────────────────────────────────────────────────────

#[test]
fn fetch_node_gives_back_the_encoding_and_has_node_a_plain_bool() {
    let a = Asked::granting("store", Box::new(Ledger));
    let held = a.arg(Value::Bytes(b"a value the ledger holds".to_vec()));
    let absent = Cairn::of_encoded(b"a name this ledger never wrote");

    // §7.2: the encoding *is* the node, which is what makes `fetch_node` the
    // inverse of `seal`.
    let want = Stored::Value(Value::Bytes(b"a value the ledger holds".to_vec())).encode();
    assert_eq!(a.said("fetch_node", vec![a.arg(Value::Cairn(held))]), given(Value::Bytes(want)));
    assert_eq!(
        a.said("fetch_node", vec![a.arg(Value::Cairn(absent))]),
        Value::Answer(Box::new(AnswerOf::Refused(nether_ledger::Refusal::Absent))),
        "§9.3 gives it `absent` and nothing else"
    );

    // §9.3 types `has_node` as `Bool`, so no wrapping. 0171.
    assert_eq!(a.said("has_node", vec![a.arg(Value::Cairn(held))]), Value::Bool(true));
    assert_eq!(a.said("has_node", vec![a.arg(Value::Cairn(absent))]), Value::Bool(false));
}

#[test]
fn the_store_capability_is_stratum_one() {
    let a = Asked::granting("store-depth", Box::new(Ledger));
    assert_eq!(a.world.depth(), Depth::STORE);
    assert!(!a.world.holds(nether_core::Capability::Env), "§1.1 is a total order");
}

// ── §9.4 env ────────────────────────────────────────────────────────────────

fn an_env(what: &str, declared: Declared) -> Asked {
    Asked::granting(what, Box::new(Env::pinned(declared, 1_700_000_000, "aarch64-apple-darwin")))
}

#[test]
fn a_declared_variable_is_given_and_a_declared_empty_one_is_refused() {
    // §9.4 distinguishes two mistakes, and this is the one that is a refusal:
    // the world was asked and said no, which is an answer.
    let declared =
        Declared::none().and("CC", "clang").expect("CC").and("LDFLAGS", "").expect("LDFLAGS");
    let a = an_env("declared", declared);
    assert_eq!(
        a.said("env", vec![a.arg(Value::Str("CC".into()))]),
        given(Value::Str("clang".into()))
    );
    assert_eq!(
        a.said("env", vec![a.arg(Value::Str("LDFLAGS".into()))]),
        Value::Answer(Box::new(AnswerOf::Refused(nether_ledger::Refusal::Absent))),
        "declared and unset is a refusal, not an empty string"
    );
}

#[test]
fn a_variable_that_was_never_declared_collapses() {
    // §9.4, and §9.9: not a refusal. "A build that silently behaves
    // differently because a variable was absent is exactly the class of bug
    // this language exists to make impossible."
    let a = an_env("undeclared", Declared::none());
    let call = Call { function: "env".into(), args: vec![a.arg(Value::Str("CC".into()))] };
    let span = Span { source: a.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
    let no = a.world.ask(&call, span, &Recorder::new(&a.store)).expect_err("not an answer");
    let nether_world::Unanswered::Collapsed(why) = no else { panic!("{no:?} is not a collapse") };
    assert!(why.contains("never declared"), "{why}");
}

#[test]
fn the_clock_and_the_target_are_the_ones_that_were_pinned() {
    // §8.3.1: never the host's. A rite whose answer depends on which machine
    // ran it is what §6.7 exists to prevent.
    let a = an_env("pinned", Declared::none());
    assert_eq!(a.said("clock", Vec::new()), Value::Int(1_700_000_000));
    assert_eq!(a.said("target", Vec::new()), Value::Str("aarch64-apple-darwin".into()));
}

#[test]
fn a_name_declared_twice_is_an_error() {
    // §8.3.1: not a last-one-wins. The two invocations differ and only one of
    // them can be what was meant.
    let once = Declared::none().and("CC", "clang").expect("CC");
    let twice = once.and("CC", "gcc").expect_err("the same name again");
    assert!(twice.to_string().contains("CC"), "{twice}");
}
