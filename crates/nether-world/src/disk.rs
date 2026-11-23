//! Strata 3 and 4: the disk.
//!
//! `spec/09-prelude.md` §9.5. Reading is depth 3 and writing is depth 4,
//! because §1.8 orders them by how much of the world a mistake reaches: a read
//! that is wrong is a build that is wrong, and a write that is wrong is a
//! filesystem that is wrong.
//!
//! Every answer is sealed before the program is told. §1.1 sets the witness
//! obligation: at stratum 3 the path, the bytes read and their cairn; at
//! stratum 4 the path and the cairn of what was written. Recording the call
//! and the answer covers both, because the call holds the path.

use std::io::Read;
use std::path::{Component, Path, PathBuf};

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, Store, Stored, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// How much of a file is read. Matches `net::MOST`, so a huge file or FIFO
/// cannot exhaust memory or block a burial forever (§6.4).
const MOST: u64 = 64 * 1024 * 1024;

/// The disk, under a root nothing may reach out of.
///
/// A path is resolved against `root` and may not climb above it. That is not
/// in §09 — the specification says nothing about where a path is rooted —
/// because it is a fact about this implementation rather than the language: a
/// burial that can read `/etc/shadow` because a program asked it to is a
/// burial nobody can grant a capability to.
pub struct Disk {
    root: PathBuf,
    /// Whether stratum 4 was granted as well as stratum 3.
    writable: bool,
}

impl Disk {
    /// Reading only: `read`, `list` and `exists`, at depth 3.
    #[must_use]
    pub fn reading(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), writable: false }
    }

    /// Reading and writing: `write` and `remove` as well, at depth 4.
    #[must_use]
    pub fn writing(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), writable: true }
    }

    /// Where a path lands, or the refusal that says it does not.
    ///
    /// Two passes, because one is not enough. The first is textual and refuses
    /// `..`, an absolute path and a drive letter — that is everything that can
    /// be said about a path which does not exist yet. The second asks the
    /// filesystem, because nothing textual can see a symbolic link: one inside
    /// the root pointing out of it is a path that never climbs and still
    /// leaves.
    ///
    /// What the second pass is worth, exactly: it is a check before an act, so
    /// a link created between the two still wins. What it stops is a link that
    /// was already there — which is the one a program being buried can arrange
    /// for itself, by writing at stratum 4 and reading at stratum 3.
    fn resolve(&self, path: &str) -> Result<PathBuf, Refusal> {
        if path.is_empty() {
            return Err(Refusal::Malformed);
        }
        let mut rel = Vec::new();
        for part in Path::new(path).components() {
            match part {
                Component::Normal(name) => rel.push(name),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Refusal::Denied);
                }
            }
        }
        let Ok(root) = self.root.canonicalize() else { return Err(Refusal::Denied) };
        let mut cur = root.clone();
        for name in rel {
            cur.push(name);
            if !step_within(&root, &mut cur, 0) {
                return Err(Refusal::Denied);
            }
        }
        if within(&root, &cur) { Ok(cur) } else { Err(Refusal::Denied) }
    }
}

/// Resolve one path component while refusing any symlink — including a dangling
/// symlink — whose target escapes `root`.
fn step_within(root: &Path, cur: &mut PathBuf, hops: u8) -> bool {
    if hops > 40 {
        return false;
    }
    let Ok(meta) = std::fs::symlink_metadata(&*cur) else {
        // Not present yet (e.g. a new file or directory for `write`).
        return true;
    };
    if !meta.file_type().is_symlink() {
        return true;
    }
    let Ok(target) = std::fs::read_link(&*cur) else { return false };
    let mut resolved = if target.is_absolute() {
        PathBuf::new()
    } else {
        cur.parent().map_or_else(|| root.to_path_buf(), Path::to_path_buf)
    };
    for part in target.components() {
        match part {
            Component::RootDir => resolved.push(Component::RootDir.as_os_str()),
            Component::Prefix(p) => resolved.push(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !resolved.pop() {
                    return false;
                }
            }
            Component::Normal(n) => {
                resolved.push(n);
                if !step_within(root, &mut resolved, hops + 1) {
                    return false;
                }
            }
        }
    }
    if !resolved.starts_with(root) {
        return false;
    }
    *cur = resolved;
    true
}

/// Whether that path is under the canonical `root` once every existing link is followed.
fn within(root: &Path, at: &Path) -> bool {
    let mut probe = at;
    loop {
        if let Ok(real) = probe.canonicalize() {
            return real.starts_with(root);
        }
        match probe.parent() {
            Some(parent) if parent != probe => probe = parent,
            _ => return false,
        }
    }
}

/// The refusal an `io::Error` is. §9.5 lists which each function may give.
fn why(e: &std::io::Error) -> Refusal {
    match e.kind() {
        std::io::ErrorKind::NotFound => Refusal::Absent,
        std::io::ErrorKind::PermissionDenied => Refusal::Denied,
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => Refusal::Exhausted,
        // Everything else is the machine being unable to say, which is what
        // `unreachable` means for a thing that is supposed to be here.
        _ => Refusal::Unreachable,
    }
}

/// The one `Str` argument every §9.5 function starts with.
fn path_of(store: &Store, call: &Call) -> Option<String> {
    match store.get(*call.args.first()?) {
        Ok(Stored::Value(Value::Str(s))) => Some(s),
        _ => None,
    }
}

impl Provider for Disk {
    fn capability(&self) -> Capability {
        if self.writable { Capability::DiskWrite } else { Capability::Disk }
    }

    fn answers(&self, function: &str) -> bool {
        match function {
            "read" | "list" | "exists" => true,
            "write" | "remove" => self.writable,
            _ => false,
        }
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        let stratum = match call.function.as_str() {
            "write" | "remove" => Depth::DISK_WRITE,
            _ => Depth::DISK,
        };
        // A call whose argument is not a finished string is a call §6.3 says
        // cannot have become a hole, and §04 types the argument, so reaching
        // here at all means a program the checker should have rejected. The
        // refusal is wrapped even for `exists`, because there is no plain
        // `Bool` that is not also an answer to a question nobody asked.
        let Some(path) = path_of(into.store(), call) else {
            if call.function == "exists" {
                return Err(Refuse::Collapse("`exists` was not given a path".into()));
            }
            return into.record(call, stratum, span, &refused(Refusal::Malformed));
        };
        let said = match self.resolve(&path) {
            Err(_) if call.function == "exists" => given(&call.function, Value::Bool(false)),
            Err(no) => refused(no),
            Ok(at) => Self::act(&call.function, &at, call, into),
        };
        into.record(call, stratum, span, &said)
    }
}

impl Disk {
    /// Touch the world, once, and say what it said.
    fn act(function: &str, at: &Path, call: &Call, into: &Recorder) -> Value {
        match function {
            "read" => Self::read(at),
            "exists" => given(function, Value::Bool(at.try_exists().unwrap_or(false))),
            "list" => match std::fs::read_dir(at) {
                Err(e) => refused(why(&e)),
                Ok(entries) => {
                    // Sorted, because a directory has no order and a trace
                    // that depended on the filesystem's would not replay.
                    let mut names = Vec::new();
                    for entry in entries {
                        let Ok(entry) = entry else {
                            return refused(Refusal::Unreachable);
                        };
                        let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                            return refused(Refusal::Malformed);
                        };
                        names.push(name);
                    }
                    names.sort();
                    given(function, Value::Array(names.into_iter().map(Value::Str).collect()))
                }
            },
            "remove" => std::fs::remove_file(at)
                .map_or_else(|e| refused(why(&e)), |()| given(function, Value::Unit)),
            "write" => Self::write(at, call, into),
            _ => refused(Refusal::Malformed),
        }
    }

    fn read(at: &Path) -> Value {
        let meta = match std::fs::metadata(at) {
            Ok(m) => m,
            Err(e) => return refused(why(&e)),
        };
        if !meta.is_file() {
            return refused(Refusal::Denied);
        }
        if meta.len() > MOST {
            return refused(Refusal::Exhausted);
        }
        let file = match std::fs::File::open(at) {
            Ok(f) => f,
            Err(e) => return refused(why(&e)),
        };
        let mut bytes = Vec::new();
        if let Err(e) = file.take(MOST + 1).read_to_end(&mut bytes) {
            return refused(why(&e));
        }
        if bytes.len() as u64 > MOST {
            return refused(Refusal::Exhausted);
        }
        given("read", Value::Bytes(bytes))
    }

    /// §9.5: the only prelude function whose witness records something the
    /// ledger cannot later reproduce on its own.
    fn write(at: &Path, call: &Call, into: &Recorder) -> Value {
        let Some(Ok(Stored::Value(Value::Bytes(bytes)))) =
            call.args.get(1).map(|c| into.store().get(*c))
        else {
            return refused(Refusal::Malformed);
        };
        if let Some(parent) = at.parent() {
            if let Err(e) = std::fs::create_dir_all(parent) {
                return refused(why(&e));
            }
        }
        std::fs::write(at, &bytes)
            .map_or_else(|e| refused(why(&e)), |()| given("write", Value::Unit))
    }
}
