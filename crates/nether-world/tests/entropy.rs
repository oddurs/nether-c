//! The proof for *the world: stratum 7*.
//!
//! `spec/09-prelude.md` §9.7. `draw` is the only nondeterminism in the
//! language, so it is the only provider whose answer cannot be predicted —
//! which means what is checked here is everything around the bytes.

use nether_core::{Capability, Depth};
use nether_ledger::{Cairn, Call, Node, Span, Store, Stored, Value};
use nether_world::{Entropy, Provider, Recorder, Unanswered, World};

struct Asked {
    store: Store,
    world: World,
}

impl Asked {
    fn new(what: &str) -> Self {
        let at = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map_or(0, |d| d.as_nanos());
        let root = std::env::temp_dir().join(format!("nether-entropy-{what}-{at}"));
        std::fs::create_dir_all(&root).expect("a directory");
        let store = Store::open(root.join("ledger")).expect("a store");
        let source: Box<dyn Provider> = Box::new(Entropy::opening().expect("a source"));
        Self { store, world: World::sealed().granting(source) }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    fn draw(&self, n: i64) -> Result<Vec<u8>, Unanswered> {
        let call = Call { function: "draw".into(), args: vec![self.arg(Value::Int(n))] };
        let span = Span { source: self.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store))?;

        // §1.4, and it is the whole of why stratum 7 is survivable: the bytes
        // were in the ledger before this returned, so §6.7 can serve them.
        let Ok(Stored::Node(Node::Witness { stratum, answer, .. })) =
            self.store.get(recorded.witness())
        else {
            panic!("no witness")
        };
        assert_eq!(stratum, Depth::ENTROPY.get(), "§9.7 puts `draw` at 7");
        assert_eq!(answer, recorded.answer(), "the witness names something else");
        match self.store.get(recorded.answer()) {
            // §9.7 types it `Bytes`, not `Answer<Bytes>`: there is no no.
            Ok(Stored::Value(Value::Bytes(b))) => Ok(b),
            other => panic!("not bytes: {other:?}"),
        }
    }
}

#[test]
fn a_draw_is_the_length_asked_for_and_is_recorded_before_it_is_returned() {
    let a = Asked::new("draws");
    assert_eq!(a.draw(0).expect("nothing is a length").len(), 0);
    assert_eq!(a.draw(32).expect("thirty-two").len(), 32);
    assert_eq!(a.world.depth(), Depth::ENTROPY);
    assert!(a.world.holds(Capability::Net), "§1.1 is a total order");
}

#[test]
fn two_draws_are_not_the_same_draw() {
    // The one thing §9.7 promises about the bytes. Thirty-two bytes twice by
    // chance is a thing that does not happen.
    let a = Asked::new("twice");
    assert_ne!(a.draw(32).expect("one"), a.draw(32).expect("two"));
}

#[test]
fn asking_wrongly_collapses_rather_than_refusing() {
    // §9.7 gives `draw` no way to say no, so §9.9's other failure is the only
    // one left: a collapse, which is a bug in the program.
    let a = Asked::new("wrongly");
    for (n, said) in [(-1, "fewer than no bytes"), (nether_world::MOST_DRAWN + 1, "limit")] {
        let no = a.draw(n).expect_err("not an answer");
        let Unanswered::Collapsed(why) = no else { panic!("{no:?} is not a collapse") };
        assert!(why.contains(said), "{why}");
    }
}
