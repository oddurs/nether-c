//! Strata 3 and 4, and what they write down.
//!
//! §9.5's five functions, the refusals §9.5 says each may give, and the two
//! things that are facts about this implementation rather than the language:
//! a root nothing may reach out of, and a directory listing that is sorted
//! because a filesystem's order would not replay.

use nether_core::{Capability, Depth};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, Store, Stored, Value};
use nether_world::{Disk, Recorder, World};

fn scratch(what: &str) -> std::path::PathBuf {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nether-disk-{what}-{at}"));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

struct Asked {
    store: Store,
    root: std::path::PathBuf,
    world: World,
}

impl Asked {
    fn new(what: &str, writable: bool) -> Self {
        let dir = scratch(what);
        let (root, ledger) = (dir.join("work"), dir.join("ledger"));
        std::fs::create_dir_all(&root).expect("a working directory");
        let disk = if writable { Disk::writing(&root) } else { Disk::reading(&root) };
        Self {
            store: Store::open(&ledger).expect("a store"),
            root,
            world: World::sealed().granting(Box::new(disk)),
        }
    }

    fn arg(&self, v: Value) -> Cairn {
        self.store.put(&Stored::Value(v)).expect("put")
    }

    /// Ask, and give back what the world said — read from the ledger, because
    /// that is the only place a provider can have put it.
    /// What was recorded, unwrapped, for the functions §09 types as answers.
    fn ask(&self, function: &str, args: Vec<Cairn>) -> AnswerOf {
        match self.said(function, args) {
            Value::Answer(a) => *a,
            other => panic!("not an answer: {other:?}"),
        }
    }

    /// What was recorded, exactly as the program would receive it.
    fn said(&self, function: &str, args: Vec<Cairn>) -> Value {
        let call = Call { function: function.to_owned(), args };
        let span = Span { source: self.arg(Value::Bytes(b"x.nc".to_vec())), start: 0, end: 1 };
        let recorded = self.world.ask(&call, span, &Recorder::new(&self.store)).expect("granted");

        // §1.4: the witness names the answer, and both were written before
        // this returned.
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

    fn reading(&self, path: &str) -> AnswerOf {
        self.ask("read", vec![self.arg(Value::Str(path.into()))])
    }
}

#[test]
fn reading_a_file_seals_what_it_said() {
    let a = Asked::new("read", false);
    std::fs::write(a.root.join("main.nc"), b"int main(void) { return 0; }").expect("a file");

    let AnswerOf::Given(Value::Bytes(said)) = a.reading("main.nc") else {
        panic!("the world did not answer")
    };
    assert_eq!(said, b"int main(void) { return 0; }");
}

#[test]
fn the_refusals_are_the_ones_section_nine_names() {
    // §9.5: `read` may give absent or denied.
    let a = Asked::new("refusals", false);
    assert_eq!(a.reading("nowhere.nc"), AnswerOf::Refused(Refusal::Absent));
    // A path that climbs out of the root is denied rather than followed.
    assert_eq!(a.reading("../../etc/passwd"), AnswerOf::Refused(Refusal::Denied));
    assert_eq!(a.reading("/etc/passwd"), AnswerOf::Refused(Refusal::Denied));
}

#[test]
fn a_listing_is_sorted_because_a_filesystem_has_no_order() {
    // A directory's order is a fact about one machine at one moment, and a
    // trace that depended on it would not replay. §6.7.
    let a = Asked::new("list", false);
    for name in ["c.nc", "a.nc", "b.nc"] {
        std::fs::write(a.root.join(name), b"").expect("a file");
    }
    let AnswerOf::Given(Value::Array(names)) = a.ask("list", vec![a.arg(Value::Str(".".into()))])
    else {
        panic!("the world did not answer")
    };
    let names: Vec<&str> = names
        .iter()
        .map(|n| match n {
            Value::Str(s) => s.as_str(),
            _ => panic!("not a name"),
        })
        .collect();
    assert_eq!(names, ["a.nc", "b.nc", "c.nc"]);
}

#[test]
fn exists_is_a_bool_and_not_an_answer() {
    // §9.5 gives `exists` a `Bool` and no refusals: "is it there" has an answer
    // either way, so there is no no to distinguish and nothing to unwrap. An
    // `Answer` here is an `Answer` where the program declared a `Bool`.
    let a = Asked::new("exists", false);
    std::fs::write(a.root.join("here.nc"), b"").expect("a file");
    assert_eq!(a.said("exists", vec![a.arg(Value::Str("here.nc".into()))]), Value::Bool(true));
    assert_eq!(a.said("exists", vec![a.arg(Value::Str("gone.nc".into()))]), Value::Bool(false));
}

#[test]
fn writing_is_stratum_four_and_reading_alone_will_not_do_it() {
    // §1.8: writing is deeper than reading because the ordering is how much of
    // the world a mistake reaches.
    let reading = Asked::new("no-write", false);
    let call = Call {
        function: "write".to_owned(),
        args: vec![reading.arg(Value::Str("out".into())), reading.arg(Value::Bytes(b"x".to_vec()))],
    };
    let span = Span { source: reading.arg(Value::Bytes(b"x".to_vec())), start: 0, end: 1 };
    let refused = reading
        .world
        .ask(&call, span, &Recorder::new(&reading.store))
        .expect_err("disk! was not granted");
    assert!(refused.to_string().contains("descend disk!"), "{refused}");

    let writing = Asked::new("write", true);
    assert!(writing.world.holds(Capability::DiskWrite));
    assert_eq!(writing.world.depth(), Depth::DISK_WRITE);
    let bytes = writing.arg(Value::Bytes(b"obj:nc".to_vec()));
    assert_eq!(
        writing.ask("write", vec![writing.arg(Value::Str("out/obj".into())), bytes]),
        AnswerOf::Given(Value::Unit)
    );
    assert_eq!(std::fs::read(writing.root.join("out/obj")).expect("written"), b"obj:nc");
}

#[test]
fn removing_something_that_is_not_there_is_absent() {
    let a = Asked::new("remove", true);
    assert_eq!(
        a.ask("remove", vec![a.arg(Value::Str("gone".into()))]),
        AnswerOf::Refused(Refusal::Absent)
    );
    std::fs::write(a.root.join("here"), b"").expect("a file");
    assert_eq!(
        a.ask("remove", vec![a.arg(Value::Str("here".into()))]),
        AnswerOf::Given(Value::Unit)
    );
    assert!(!a.root.join("here").exists());
}

#[test]
fn a_deeper_grant_answers_a_shallower_question() {
    // §1.1's total order, and [DESCEND] raising the ambient to what was
    // granted: `descend disk! { read(...) }` is stratum 3 within stratum 4.
    let a = Asked::new("deeper", true);
    assert!(a.world.holds(Capability::DiskWrite));
    assert!(a.world.holds(Capability::Disk), "disk! does not reach disk");
    assert!(!a.world.holds(Capability::Net), "disk! reaches the network");

    std::fs::write(a.root.join("main.nc"), b"x").expect("a file");
    assert_eq!(a.reading("main.nc"), AnswerOf::Given(Value::Bytes(b"x".to_vec())));
}

#[test]
fn what_was_read_outlives_the_file_it_was_read_from() {
    // The proof. §9.5: "the answer is sealed and recorded before it reaches
    // the program", so the ledger holds it whatever happens to the disk
    // afterwards — which is what makes a trace replayable at all (§6.7).
    let a = Asked::new("outlives", false);
    let file = a.root.join("main.nc");
    std::fs::write(&file, b"int main(void) { return 0; }").expect("a file");

    let call =
        Call { function: "read".to_owned(), args: vec![a.arg(Value::Str("main.nc".into()))] };
    let span = Span { source: a.arg(Value::Bytes(b"build.nc".to_vec())), start: 0, end: 1 };
    let recorded = a.world.ask(&call, span, &Recorder::new(&a.store)).expect("disk was granted");

    std::fs::remove_file(&file).expect("delete it");

    // Gone from the world, and exactly as it was in the ledger.
    let Ok(Stored::Value(Value::Answer(said))) = a.store.get(recorded.answer()) else {
        panic!("the answer is not in the ledger")
    };
    assert_eq!(*said, AnswerOf::Given(Value::Bytes(b"int main(void) { return 0; }".to_vec())));

    // And the world no longer has it, which is the half that makes the first
    // half worth anything: the ledger is the only place that answer still is.
    assert_eq!(a.reading("main.nc"), AnswerOf::Refused(Refusal::Absent));
}

#[test]
fn a_link_does_not_reach_out_of_the_root() {
    // The root is what makes granting `disk` to a burial safe, and refusing
    // `..` textually is not enough: a link inside the root pointing out of it
    // is a path that never climbs and still leaves.
    let a = Asked::new("symlink", false);
    let outside = a.root.parent().expect("a parent").join("outside.txt");
    std::fs::write(&outside, b"SECRET").expect("a file outside the root");

    #[cfg(unix)]
    {
        std::os::unix::fs::symlink(&outside, a.root.join("link.txt")).expect("a link to it");
        assert_eq!(a.reading("link.txt"), AnswerOf::Refused(Refusal::Denied));

        // And through a linked directory, which is the same trick one level up.
        let elsewhere = a.root.parent().expect("a parent").join("elsewhere");
        std::fs::create_dir_all(&elsewhere).expect("a directory outside");
        std::fs::write(elsewhere.join("secret.txt"), b"ALSO SECRET").expect("a file in it");
        std::os::unix::fs::symlink(&elsewhere, a.root.join("out")).expect("a link to it");
        assert_eq!(a.reading("out/secret.txt"), AnswerOf::Refused(Refusal::Denied));
    }

    // What is inside still reads, which is the half that makes it a root
    // rather than a wall.
    std::fs::write(a.root.join("inside.txt"), b"fine").expect("a file inside");
    assert_eq!(a.reading("inside.txt"), AnswerOf::Given(Value::Bytes(b"fine".to_vec())));
}

#[test]
fn a_path_that_does_not_exist_yet_still_resolves() {
    // Asking the filesystem must not break `write`, whose whole point is a
    // path that is not there yet — including one whose parent is not either.
    let a = Asked::new("not-yet", true);
    let bytes = a.arg(Value::Bytes(b"obj:nc".to_vec()));
    assert_eq!(
        a.ask("write", vec![a.arg(Value::Str("deep/down/obj".into())), bytes]),
        AnswerOf::Given(Value::Unit)
    );
    assert_eq!(std::fs::read(a.root.join("deep/down/obj")).expect("written"), b"obj:nc");
}
