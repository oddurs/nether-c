//! Strata 3 and 4, and what they write down.
//!
//! §9.5's five functions, the refusals §9.5 says each may give, and the two
//! things that are facts about this implementation rather than the language:
//! a root nothing may reach out of, and a directory listing that is sorted
//! because a filesystem's order would not replay.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use nether_core::{Capability, Depth};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, Store, Stored, Value};
use nether_world::{Disk, Recorder, World};

struct Scratch(PathBuf);

impl Deref for Scratch {
    type Target = Path;

    fn deref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<Path> for Scratch {
    fn as_ref(&self) -> &Path {
        &self.0
    }
}

impl AsRef<std::ffi::OsStr> for Scratch {
    fn as_ref(&self) -> &std::ffi::OsStr {
        self.0.as_os_str()
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn scratch(what: &str) -> Scratch {
    static N: AtomicU64 = AtomicU64::new(0);
    let dir = std::env::temp_dir().join(format!(
        "nether-disk-{what}-{}-{}",
        std::process::id(),
        N.fetch_add(1, Ordering::Relaxed)
    ));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    Scratch(dir)
}

struct Asked {
    store: Store,
    root: PathBuf,
    world: World,
    _dir: Scratch,
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
            _dir: dir,
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
        let call = Call::try_new(function, args).unwrap();
        let span = Span::new(self.arg(Value::Bytes(b"x.nc".to_vec())), 0, 1).unwrap();
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
    let call = Call::try_new(
        "write",
        vec![reading.arg(Value::Str("out".into())), reading.arg(Value::Bytes(b"x".to_vec()))],
    )
    .unwrap();
    let span = Span::new(reading.arg(Value::Bytes(b"x".to_vec())), 0, 1).unwrap();
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
fn the_disk_write_provider_does_not_answer_reads() {
    // §9.1 / §8.2: each capability grants its own stratum and no other.
    let a = Asked::new("deeper", true);
    assert!(a.world.holds(Capability::DiskWrite));
    assert!(!a.world.holds(Capability::Disk), "a grant of disk! is not a grant of disk");
    assert!(!a.world.holds(Capability::Net), "disk! does not reach the network");

    std::fs::write(a.root.join("main.nc"), b"x").expect("a file");
    let call = Call::try_new("read", vec![a.arg(Value::Str("main.nc".into()))]).unwrap();
    let span = Span::new(a.arg(Value::Bytes(b"x.nc".to_vec())), 0, 1).unwrap();
    assert!(matches!(
        a.world.ask(&call, span, &Recorder::new(&a.store)),
        Err(nether_world::Unanswered::Ungranted { .. })
    ));
}

#[test]
fn what_was_read_outlives_the_file_it_was_read_from() {
    // The proof. §9.5: "the answer is sealed and recorded before it reaches
    // the program", so the ledger holds it whatever happens to the disk
    // afterwards — which is what makes a trace replayable at all (§6.7).
    let a = Asked::new("outlives", false);
    let file = a.root.join("main.nc");
    std::fs::write(&file, b"int main(void) { return 0; }").expect("a file");

    let call = Call::try_new("read", vec![a.arg(Value::Str("main.nc".into()))]).unwrap();
    let span = Span::new(a.arg(Value::Bytes(b"build.nc".to_vec())), 0, 1).unwrap();
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

#[test]
#[cfg(unix)]
fn a_dangling_symlink_cannot_escape_the_root_on_write() {
    let a = Asked::new("dangling-write", true);
    let outside = a.root.parent().expect("a parent").join("escaped.txt");
    assert!(!outside.exists());
    std::os::unix::fs::symlink(&outside, a.root.join("escape")).expect("dangling link");
    let bytes = a.arg(Value::Bytes(b"pwned".to_vec()));
    assert_eq!(
        a.ask("write", vec![a.arg(Value::Str("escape".into())), bytes]),
        AnswerOf::Refused(Refusal::Denied)
    );
    assert!(!outside.exists(), "write followed a dangling symlink outside root");
}

#[test]
fn exists_on_escaped_path_is_false_and_empty_or_nul_path_is_malformed() {
    let a = Asked::new("exists-edge", false);
    assert_eq!(
        a.said("exists", vec![a.arg(Value::Str("../../etc/passwd".into()))]),
        Value::Bool(false)
    );
    assert_eq!(a.reading(""), AnswerOf::Refused(Refusal::Malformed));
    assert_eq!(a.reading("a\0b"), AnswerOf::Refused(Refusal::Malformed));
}

#[test]
#[cfg(unix)]
fn exponential_symlink_dag_stops_at_total_hop_bound_and_remove_unlinks_symlink() {
    let r = Asked::new("symlink-dag-read", false);
    // Build a binary-branching symlink DAG: s_i -> s_{i+1}/s_{i+1}.
    // With a total hop budget of 40, this finishes immediately instead of 2^25 work.
    std::fs::create_dir(r.root.join("d")).unwrap();
    std::os::unix::fs::symlink(".", r.root.join("d/s24")).unwrap();
    for i in (0..24).rev() {
        let next = format!("s{}/s{}", i + 1, i + 1);
        std::os::unix::fs::symlink(&next, r.root.join(format!("d/s{i}"))).unwrap();
    }
    assert_eq!(r.reading("d/s0"), AnswerOf::Refused(Refusal::Denied));

    let a = Asked::new("symlink-dag-and-remove", true);
    // Removing a symlink deletes the symlink itself, leaving its target intact.
    std::fs::write(a.root.join("keep.txt"), b"keep").unwrap();
    std::os::unix::fs::symlink("keep.txt", a.root.join("alias")).unwrap();
    assert_eq!(
        a.ask("remove", vec![a.arg(Value::Str("alias".into()))]),
        AnswerOf::Given(Value::Unit)
    );
    assert!(a.root.join("keep.txt").exists(), "remove deleted target instead of symlink");
    assert!(std::fs::symlink_metadata(a.root.join("alias")).is_err());

    // A dangling symlink inside root is also removable.
    std::os::unix::fs::symlink("does-not-exist", a.root.join("dangling")).unwrap();
    assert_eq!(
        a.ask("remove", vec![a.arg(Value::Str("dangling".into()))]),
        AnswerOf::Given(Value::Unit)
    );
    assert!(std::fs::symlink_metadata(a.root.join("dangling")).is_err());
}

#[test]
fn corrupt_argument_propagates_not_recorded_and_write_leaves_no_temps() {
    // H15: absent or corrupt argument cairn must fail with NotRecorded, not a
    // fabricated Malformed witness.
    let a = Asked::new("arg-store-err", false);
    let missing_arg = nether_ledger::Cairn::of_encoded(b"never-stored-arg");
    let call = Call::try_new("read", vec![missing_arg]).unwrap();
    let span = Span::new(a.arg(Value::Bytes(b"x.nc".to_vec())), 0, 1).unwrap();
    assert!(matches!(
        a.world.ask(&call, span, &Recorder::new(&a.store)),
        Err(nether_world::Unanswered::NotRecorded(_))
    ));

    // Corrupting a stored argument cairn also fails with NotRecorded.
    let path_cairn = a.arg(Value::Str("hello.txt".into()));
    let hex = path_cairn.to_string();
    let ledger_dir = a.root.parent().unwrap().join("ledger");
    let obj_path = ledger_dir.join("objects").join(&hex[..2]).join(&hex[2..]);
    std::fs::write(&obj_path, b"corrupted-bytes").unwrap();
    let fresh_store = Store::open(&ledger_dir).unwrap();
    let call2 = Call::try_new("read", vec![path_cairn]).unwrap();
    assert!(matches!(
        a.world.ask(&call2, span, &Recorder::new(&fresh_store)),
        Err(nether_world::Unanswered::NotRecorded(_))
    ));

    // H16: atomic Disk::write overwrites cleanly and leaves no temp files behind.
    let w = Asked::new("atomic-write", true);
    std::fs::write(w.root.join("target.txt"), b"old").unwrap();
    let p = w.arg(Value::Str("target.txt".into()));
    let b = w.arg(Value::Bytes(b"new-content".to_vec()));
    assert_eq!(w.ask("write", vec![p, b]), AnswerOf::Given(Value::Unit));
    assert_eq!(std::fs::read(w.root.join("target.txt")).unwrap(), b"new-content");
    let entries: Vec<_> =
        std::fs::read_dir(&w.root).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(entries.len(), 1, "unexpected leftover files: {entries:?}");

    // H16: reading a FIFO with O_NONBLOCK returns Denied immediately without hanging.
    #[cfg(unix)]
    {
        let fifo = a.root.join("pipe");
        let status = std::process::Command::new("mkfifo").arg(&fifo).status();
        if status.is_ok_and(|s| s.success()) {
            assert_eq!(a.reading("pipe"), AnswerOf::Refused(Refusal::Denied));
            assert_eq!(
                a.ask("list", vec![a.arg(Value::Str("pipe".into()))]),
                AnswerOf::Refused(Refusal::Unreachable)
            );
        }
    }
}

#[test]
#[cfg(unix)]
fn symlinked_root_ancestor_allows_in_root_absolute_symlink_and_denies_escapes() {
    let base = scratch("symlink-ancestor");
    let real_parent = base.join("real");
    let root = real_parent.join("work");
    std::fs::create_dir_all(&root).unwrap();
    std::fs::write(root.join("inside.txt"), b"ok-inside").unwrap();
    std::fs::write(real_parent.join("outside.txt"), b"secret-outside").unwrap();

    let alias_parent = base.join("alias");
    std::os::unix::fs::symlink(&real_parent, &alias_parent).unwrap();
    let alias_root = alias_parent.join("work");

    // In-root absolute symlink pointing via the symlinked ancestor path.
    std::os::unix::fs::symlink(alias_root.join("inside.txt"), root.join("abs_in.txt")).unwrap();
    // Out-of-root absolute symlink via the same symlinked ancestor path.
    std::os::unix::fs::symlink(alias_parent.join("outside.txt"), root.join("abs_out.txt")).unwrap();

    let store = Store::open(base.join("ledger")).unwrap();
    let world = World::sealed().granting(Box::new(Disk::reading(&alias_root)));
    let src = store.put(&Stored::Value(Value::Bytes(b"x.nc".to_vec()))).unwrap();
    let span = Span::new(src, 0, 1).unwrap();
    let ask_read = |name: &str| {
        let arg = store.put(&Stored::Value(Value::Str(name.into()))).unwrap();
        let call = Call::try_new("read", vec![arg]).unwrap();
        let rec = world.ask(&call, span, &Recorder::new(&store)).unwrap();
        match store.get(rec.answer()).unwrap() {
            Stored::Value(Value::Answer(a)) => *a,
            other => panic!("unexpected {other:?}"),
        }
    };
    assert_eq!(ask_read("abs_in.txt"), AnswerOf::Given(Value::Bytes(b"ok-inside".to_vec())));
    assert_eq!(ask_read("abs_out.txt"), AnswerOf::Refused(Refusal::Denied));
}
