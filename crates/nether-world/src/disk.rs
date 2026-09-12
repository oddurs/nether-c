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

use std::path::{Component, Path, PathBuf};

use nether_core::{Capability, Depth};
use nether_ledger::{AnswerOf, Call, Refusal, Span, Store, StoreError, Stored, Value};

use crate::provider::Provider;
use crate::recorder::{Recorded, Recorder};

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
    /// `..` is resolved textually rather than by asking the filesystem,
    /// because asking would follow a symbolic link back out of the root.
    fn resolve(&self, path: &str) -> Result<PathBuf, Refusal> {
        let mut out = self.root.clone();
        for part in Path::new(path).components() {
            match part {
                Component::Normal(name) => out.push(name),
                Component::CurDir => {}
                // Out of the root, or an absolute path, or a drive letter.
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Refusal::Denied);
                }
            }
        }
        Ok(out)
    }
}

/// What the world said, before it is written down.
fn given(v: Value) -> Value {
    Value::Answer(Box::new(AnswerOf::Given(v)))
}

/// Which no it was. §5.1.1's closed set of six.
fn refused(r: Refusal) -> Value {
    Value::Answer(Box::new(AnswerOf::Refused(r)))
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

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, StoreError> {
        let stratum = match call.function.as_str() {
            "write" | "remove" => Depth::DISK_WRITE,
            _ => Depth::DISK,
        };
        // A call whose argument is not a finished string is a call §6.3 says
        // cannot have become a hole. Refusing is the only honest answer and it
        // is recorded like any other.
        let Some(path) = path_of(into.store(), call) else {
            return into.record(call, stratum, span, &refused(Refusal::Malformed));
        };
        let said = match self.resolve(&path) {
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
            "read" => {
                std::fs::read(at).map_or_else(|e| refused(why(&e)), |b| given(Value::Bytes(b)))
            }
            "exists" => given(Value::Bool(at.exists())),
            "list" => match std::fs::read_dir(at) {
                Err(e) => refused(why(&e)),
                Ok(entries) => {
                    // Sorted, because a directory has no order and a trace
                    // that depended on the filesystem's would not replay.
                    let mut names: Vec<String> = entries
                        .filter_map(|e| Some(e.ok()?.file_name().to_str()?.to_owned()))
                        .collect();
                    names.sort();
                    given(Value::Array(names.into_iter().map(Value::Str).collect()))
                }
            },
            "remove" => {
                std::fs::remove_file(at).map_or_else(|e| refused(why(&e)), |()| given(Value::Unit))
            }
            "write" => Self::write(at, call, into),
            _ => refused(Refusal::Malformed),
        }
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
        std::fs::write(at, &bytes).map_or_else(|e| refused(why(&e)), |()| given(Value::Unit))
    }
}
