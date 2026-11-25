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

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// How much of a file is read. Matches `net::MOST` and stays inside
/// `nether_ledger::MAX_OBJECT_BYTES` (64 MiB + 64 KiB framing).
const MOST: u64 = 64 * 1024 * 1024;

/// Maximum entries returned by `list` so a huge directory cannot OOM or exceed
/// `nether_ledger::MAX_OBJECT_BYTES`.
const MAX_LIST_ENTRIES: usize = 65_536;

/// `O_NOFOLLOW | O_NONBLOCK` for TOCTOU-safe, FIFO-safe `Disk::read`.
#[cfg(all(target_os = "linux", any(target_arch = "aarch64", target_arch = "arm")))]
const OPEN_NOFOLLOW_NONBLOCK: i32 = 0x10000 | 0x800;
#[cfg(all(target_os = "linux", not(any(target_arch = "aarch64", target_arch = "arm"))))]
const OPEN_NOFOLLOW_NONBLOCK: i32 = 0x20000 | 0x800;
#[cfg(target_os = "macos")]
const OPEN_NOFOLLOW_NONBLOCK: i32 = 0x0100 | 0x0004;
#[cfg(all(unix, not(any(target_os = "linux", target_os = "macos"))))]
const OPEN_NOFOLLOW_NONBLOCK: i32 = 0;

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

    /// Writing only: `write` and `remove`, at depth 4.
    ///
    /// §9.1 / §8.2: each capability grants its own stratum and no other;
    /// granting `disk!` does not grant stratum-3 `disk` reads.
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
    fn resolve(&self, path: &str, follow_final: bool) -> Result<PathBuf, Refusal> {
        if path.is_empty() || path.contains('\0') {
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
        if !follow_final && rel.is_empty() {
            return Err(Refusal::Denied);
        }
        let Ok(root) = self.root.canonicalize() else { return Err(Refusal::Denied) };
        let mut cur = root.clone();
        let mut hops = 0u8;
        let resolve_count = if follow_final { rel.len() } else { rel.len() - 1 };
        for name in &rel[..resolve_count] {
            cur.push(name);
            if !step_within(&root, &mut cur, &mut hops) {
                return Err(Refusal::Denied);
            }
        }
        if !within(&root, &cur) {
            return Err(Refusal::Denied);
        }
        if !follow_final {
            // Do not dereference the final component on `remove`, so removing a
            // symlink (including a dangling one) removes the link itself rather
            // than deleting its target.
            cur.push(rel[rel.len() - 1]);
            return Ok(cur);
        }
        Ok(cur)
    }
}

/// Resolve one path component while refusing any symlink — including a dangling
/// symlink — whose target escapes `root`, bounding total expansions across the
/// whole `resolve` call so cyclic or DAG symlinks cannot cause exponential work.
fn step_within(root: &Path, cur: &mut PathBuf, hops: &mut u8) -> bool {
    let meta = match std::fs::symlink_metadata(&*cur) {
        Ok(m) => m,
        // Not present yet (e.g. a new file or directory for `write`).
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return true,
        Err(_) => return false,
    };
    if !meta.file_type().is_symlink() {
        return true;
    }
    if *hops >= 40 {
        return false;
    }
    *hops += 1;
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
                if !step_within(root, &mut resolved, hops) {
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
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::IsADirectory => Refusal::Denied,
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => Refusal::Exhausted,
        // Everything else is the machine being unable to say, which is what
        // `unreachable` means for a thing that is supposed to be here.
        _ => Refusal::Unreachable,
    }
}

impl Provider for Disk {
    fn capability(&self) -> Capability {
        if self.writable { Capability::DiskWrite } else { Capability::Disk }
    }

    fn answers(&self, function: &str) -> bool {
        match function {
            "read" | "list" | "exists" => !self.writable,
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
        let Some(path) = into.arg_str(call, 0)? else {
            if call.function == "exists" {
                return Err(Refuse::Collapse("`exists` was not given a path".into()));
            }
            return into.record(call, stratum, span, &refused(Refusal::Malformed));
        };
        let follow_final = call.function != "remove";
        let said = match self.resolve(&path, follow_final) {
            Err(_) if call.function == "exists" => given(&call.function, Value::Bool(false)),
            Err(no) => refused(no),
            Ok(at) => Self::act(&call.function, &at, call, into)?,
        };
        into.record(call, stratum, span, &said)
    }
}

impl Disk {
    /// Touch the world, once, and say what it said.
    fn act(function: &str, at: &Path, call: &Call, into: &Recorder) -> Result<Value, Refuse> {
        Ok(match function {
            "read" => Self::read(at),
            "exists" => given(function, Value::Bool(at.try_exists().unwrap_or(false))),
            "list" => Self::list(at),
            "remove" => std::fs::remove_file(at)
                .map_or_else(|e| refused(why(&e)), |()| given(function, Value::Unit)),
            "write" => Self::write(at, call, into)?,
            _ => refused(Refusal::Malformed),
        })
    }

    fn list(at: &Path) -> Value {
        let entries = match std::fs::read_dir(at) {
            Err(e) => return refused(why(&e)),
            Ok(entries) => entries,
        };
        // Sorted, because a directory has no order and a trace that depended
        // on the filesystem's would not replay. Bounded by count and encoded
        // size so huge directories stay within `MAX_OBJECT_BYTES`.
        let mut names = Vec::new();
        let mut encoded: usize = 9;
        for entry in entries {
            let Ok(entry) = entry else {
                return refused(Refusal::Unreachable);
            };
            let Some(name) = entry.file_name().to_str().map(str::to_owned) else {
                return refused(Refusal::Malformed);
            };
            encoded = encoded.saturating_add(9).saturating_add(name.len());
            if names.len() >= MAX_LIST_ENTRIES || encoded as u64 > MOST {
                return refused(Refusal::Exhausted);
            }
            names.push(name);
        }
        names.sort();
        given("list", Value::Array(names.into_iter().map(Value::Str).collect()))
    }

    fn read(at: &Path) -> Value {
        let mut opts = std::fs::OpenOptions::new();
        opts.read(true);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt as _;
            opts.custom_flags(OPEN_NOFOLLOW_NONBLOCK);
        }
        let file = match opts.open(at) {
            Ok(f) => f,
            Err(e) => return refused(why(&e)),
        };
        let meta = match file.metadata() {
            Ok(m) => m,
            Err(e) => return refused(why(&e)),
        };
        if !meta.is_file() {
            return refused(Refusal::Denied);
        }
        if meta.len() > MOST {
            return refused(Refusal::Exhausted);
        }
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
    /// ledger cannot later reproduce on its own. Writes via a same-directory
    /// temporary file and atomic `rename` so `ENOSPC` or a crash never
    /// truncates an existing file while recording `Refused(Exhausted)`.
    fn write(at: &Path, call: &Call, into: &Recorder) -> Result<Value, Refuse> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let Some(bytes) = into.arg_bytes(call, 1)? else {
            return Ok(refused(Refusal::Malformed));
        };
        if let Ok(meta) = std::fs::symlink_metadata(at)
            && !meta.is_file()
        {
            return Ok(refused(Refusal::Denied));
        }
        let Some(parent) = at.parent() else {
            return Ok(refused(Refusal::Denied));
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Ok(refused(why(&e)));
        }
        let (temp, mut file) = loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let candidate = parent.join(format!(".nether-write.{}.{ts}.{n}", std::process::id()));
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(f) => break (candidate, f),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(e) => return Ok(refused(why(&e))),
            }
        };
        if let Err(e) = file.write_all(&bytes) {
            drop(file);
            let _ = std::fs::remove_file(&temp);
            return Ok(refused(why(&e)));
        }
        drop(file);
        if let Err(e) = std::fs::rename(&temp, at) {
            let _ = std::fs::remove_file(&temp);
            return Ok(refused(why(&e)));
        }
        Ok(given("write", Value::Unit))
    }
}
