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

use std::collections::VecDeque;
use std::ffi::OsString;
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

/// Per-OS/architecture `O_NOFOLLOW`, `O_NONBLOCK`, and `O_DIRECTORY` flags.
/// Unverified Unix targets fail to compile rather than silently disabling
/// `O_NOFOLLOW` / `O_NONBLOCK`.
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const OPEN_NOFOLLOW: i32 = 0x10000;
#[cfg(all(
    target_os = "linux",
    any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    )
))]
const OPEN_DIRECTORY: i32 = 0x4000;
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const OPEN_NOFOLLOW: i32 = 0x20000;
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "aarch64",
        target_arch = "arm",
        target_arch = "m68k",
        target_arch = "powerpc",
        target_arch = "powerpc64"
    ))
))]
const OPEN_DIRECTORY: i32 = 0x10000;
#[cfg(all(target_os = "linux", any(target_arch = "mips", target_arch = "mips64")))]
const OPEN_NONBLOCK: i32 = 0x0080;
#[cfg(all(target_os = "linux", any(target_arch = "sparc", target_arch = "sparc64")))]
const OPEN_NONBLOCK: i32 = 0x4000;
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    ))
))]
const OPEN_NONBLOCK: i32 = 0x0800;

#[cfg(any(target_os = "macos", target_os = "ios"))]
const OPEN_NOFOLLOW: i32 = 0x0100;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const OPEN_NONBLOCK: i32 = 0x0004;
#[cfg(any(target_os = "macos", target_os = "ios"))]
const OPEN_DIRECTORY: i32 = 0x0010_0000;

#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
const OPEN_NOFOLLOW: i32 = 0x0100;
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
const OPEN_NONBLOCK: i32 = 0x0004;
#[cfg(any(target_os = "freebsd", target_os = "openbsd"))]
const OPEN_DIRECTORY: i32 = 0x0002_0000;

#[cfg(target_os = "netbsd")]
const OPEN_NOFOLLOW: i32 = 0x0100;
#[cfg(target_os = "netbsd")]
const OPEN_NONBLOCK: i32 = 0x0004;
#[cfg(target_os = "netbsd")]
const OPEN_DIRECTORY: i32 = 0x0020_0000;

#[cfg(all(
    unix,
    not(any(
        target_os = "linux",
        target_os = "macos",
        target_os = "ios",
        target_os = "freebsd",
        target_os = "openbsd",
        target_os = "netbsd"
    ))
))]
compile_error!("verify O_NOFOLLOW, O_NONBLOCK, and O_DIRECTORY for this Unix target");

#[cfg(unix)]
const OPEN_NOFOLLOW_NONBLOCK: i32 = OPEN_NOFOLLOW | OPEN_NONBLOCK;
#[cfg(unix)]
const OPEN_DIR_FLAGS: i32 = OPEN_NOFOLLOW | OPEN_NONBLOCK | OPEN_DIRECTORY;

#[cfg(all(target_os = "linux", any(target_arch = "mips", target_arch = "mips64")))]
const ERRNO_LOOP: i32 = 90;
#[cfg(all(target_os = "linux", any(target_arch = "sparc", target_arch = "sparc64")))]
const ERRNO_LOOP: i32 = 62;
#[cfg(all(
    target_os = "linux",
    not(any(
        target_arch = "mips",
        target_arch = "mips64",
        target_arch = "sparc",
        target_arch = "sparc64"
    ))
))]
const ERRNO_LOOP: i32 = 40;
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const ERRNO_LOOP: i32 = 62;

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
                if resolved.starts_with(root) {
                    if !step_within(root, &mut resolved, hops) {
                        return false;
                    }
                } else if let Ok(m) = std::fs::symlink_metadata(&resolved)
                    && m.file_type().is_symlink()
                {
                    // Allow symlinked ancestors of `root` itself (e.g., macOS
                    // `/tmp -> /private/tmp`) while refusing symlinks outside
                    // `root` that point elsewhere.
                    if *hops >= 40 {
                        return false;
                    }
                    let Ok(canon) = resolved.canonicalize() else { return false };
                    if !root.starts_with(&canon) {
                        return false;
                    }
                    *hops += 1;
                    resolved = canon;
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
    #[cfg(unix)]
    if e.raw_os_error() == Some(ERRNO_LOOP) {
        return Refusal::Denied;
    }
    match e.kind() {
        std::io::ErrorKind::NotFound => Refusal::Absent,
        std::io::ErrorKind::PermissionDenied | std::io::ErrorKind::IsADirectory => Refusal::Denied,
        std::io::ErrorKind::StorageFull | std::io::ErrorKind::QuotaExceeded => Refusal::Exhausted,
        // Everything else is the machine being unable to say, which is what
        // `unreachable` means for a thing that is supposed to be here.
        _ => Refusal::Unreachable,
    }
}

#[cfg(unix)]
fn open_dir_nofollow(path: &Path) -> std::io::Result<std::fs::File> {
    use std::os::unix::fs::OpenOptionsExt as _;
    let mut opts = std::fs::OpenOptions::new();
    opts.read(true).custom_flags(OPEN_DIR_FLAGS);
    opts.open(path)
}

#[cfg(target_os = "linux")]
enum Step {
    Normal(OsString),
    Parent,
}

#[cfg(target_os = "linux")]
enum Pinned {
    Dir(std::fs::File),
    Child { parent: std::fs::File, name: OsString },
}

#[cfg(target_os = "linux")]
fn proc_fd_path(dir: &std::fs::File, name: Option<&std::ffi::OsStr>) -> PathBuf {
    use std::os::fd::AsRawFd as _;
    let mut p = PathBuf::from(format!("/proc/self/fd/{}", dir.as_raw_fd()));
    if let Some(n) = name {
        p.push(n);
    }
    p
}

#[cfg(target_os = "linux")]
fn strip_root_prefix(root: &Path, target: &Path, hops: &mut u8) -> Option<Vec<Step>> {
    let mut prefix = PathBuf::new();
    let mut iter = target.components();
    let mut matched = false;
    while !matched {
        let part = iter.next()?;
        match part {
            Component::RootDir => prefix.push(Component::RootDir.as_os_str()),
            Component::Prefix(p) => prefix.push(p.as_os_str()),
            Component::CurDir => {}
            Component::ParentDir => {
                if !prefix.pop() {
                    return None;
                }
            }
            Component::Normal(n) => {
                prefix.push(n);
                if prefix != root
                    && let Ok(m) = std::fs::symlink_metadata(&prefix)
                    && m.file_type().is_symlink()
                {
                    if *hops >= 40 {
                        return None;
                    }
                    let canon = prefix.canonicalize().ok()?;
                    if !root.starts_with(&canon) {
                        return None;
                    }
                    *hops += 1;
                    prefix = canon;
                }
            }
        }
        if prefix == root {
            matched = true;
        }
    }
    let mut rest = Vec::new();
    for part in iter {
        match part {
            Component::CurDir => {}
            Component::ParentDir => rest.push(Step::Parent),
            Component::Normal(n) => rest.push(Step::Normal(n.to_os_string())),
            Component::RootDir | Component::Prefix(_) => return None,
        }
    }
    Some(rest)
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
        #[cfg(target_os = "linux")]
        {
            let create_parents = call.function == "write";
            match self.resolve_pinned(&path, follow_final, create_parents) {
                Ok(Some(pinned)) => {
                    let said = Self::act_pinned(&call.function, pinned, call, into)?;
                    return into.record(call, stratum, span, &said);
                }
                Err(_) if call.function == "exists" => {
                    return into.record(
                        call,
                        stratum,
                        span,
                        &given(&call.function, Value::Bool(false)),
                    );
                }
                Err(no) => return into.record(call, stratum, span, &refused(no)),
                Ok(None) => {}
            }
        }
        let said = match self.resolve(&path, follow_final) {
            Err(_) if call.function == "exists" => given(&call.function, Value::Bool(false)),
            Err(no) => refused(no),
            Ok(at) => Self::act(&call.function, &at, call, into)?,
        };
        into.record(call, stratum, span, &said)
    }
}

impl Disk {
    #[cfg(target_os = "linux")]
    fn resolve_pinned(
        &self,
        path: &str,
        follow_final: bool,
        create_parents: bool,
    ) -> Result<Option<Pinned>, Refusal> {
        if path.is_empty() || path.contains('\0') {
            return Err(Refusal::Malformed);
        }
        let mut queue: VecDeque<Step> = VecDeque::new();
        for part in Path::new(path).components() {
            match part {
                Component::Normal(name) => queue.push_back(Step::Normal(name.to_os_string())),
                Component::CurDir => {}
                Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                    return Err(Refusal::Denied);
                }
            }
        }
        if !follow_final && queue.is_empty() {
            return Err(Refusal::Denied);
        }
        let Ok(canon_root) = self.root.canonicalize() else { return Err(Refusal::Denied) };
        let Ok(root_dir) = open_dir_nofollow(&canon_root) else { return Err(Refusal::Denied) };
        if std::fs::symlink_metadata(proc_fd_path(&root_dir, None)).is_err() {
            return Ok(None);
        }
        let mut dir_stack = vec![root_dir];
        let mut hops = 0u8;
        while let Some(step) = queue.pop_front() {
            match step {
                Step::Parent => {
                    if dir_stack.len() <= 1 {
                        return Err(Refusal::Denied);
                    }
                    dir_stack.pop();
                }
                Step::Normal(name) => {
                    let is_last = queue.is_empty();
                    let Some(top) = dir_stack.last() else { return Err(Refusal::Denied) };
                    let child = proc_fd_path(top, Some(&name));
                    match std::fs::symlink_metadata(&child) {
                        Ok(meta) if meta.file_type().is_symlink() && (!is_last || follow_final) => {
                            if hops >= 40 {
                                return Err(Refusal::Denied);
                            }
                            hops += 1;
                            let target = std::fs::read_link(&child).map_err(|_| Refusal::Denied)?;
                            let steps = if target.is_absolute() {
                                dir_stack.truncate(1);
                                strip_root_prefix(&canon_root, &target, &mut hops)
                                    .ok_or(Refusal::Denied)?
                            } else {
                                let mut rel = Vec::new();
                                for part in target.components() {
                                    match part {
                                        Component::CurDir => {}
                                        Component::ParentDir => rel.push(Step::Parent),
                                        Component::Normal(n) => {
                                            rel.push(Step::Normal(n.to_os_string()));
                                        }
                                        Component::RootDir | Component::Prefix(_) => {
                                            return Err(Refusal::Denied);
                                        }
                                    }
                                }
                                rel
                            };
                            for s in steps.into_iter().rev() {
                                queue.push_front(s);
                            }
                        }
                        Ok(_) | Err(_) if is_last => {
                            let Some(parent) = dir_stack.pop() else {
                                return Err(Refusal::Denied);
                            };
                            return Ok(Some(Pinned::Child { parent, name }));
                        }
                        Ok(_) => {
                            let dir = open_dir_nofollow(&child).map_err(|e| why(&e))?;
                            dir_stack.push(dir);
                        }
                        Err(e) if e.kind() == std::io::ErrorKind::NotFound && create_parents => {
                            match std::fs::create_dir(&child) {
                                Ok(()) => {}
                                Err(ce) if ce.kind() == std::io::ErrorKind::AlreadyExists => {}
                                Err(ce) => return Err(why(&ce)),
                            }
                            let dir = open_dir_nofollow(&child).map_err(|oe| why(&oe))?;
                            dir_stack.push(dir);
                        }
                        Err(e) => return Err(why(&e)),
                    }
                }
            }
        }
        if !follow_final {
            return Err(Refusal::Denied);
        }
        let Some(dir) = dir_stack.pop() else { return Err(Refusal::Denied) };
        Ok(Some(Pinned::Dir(dir)))
    }

    #[cfg(target_os = "linux")]
    fn act_pinned(
        function: &str,
        pinned: Pinned,
        call: &Call,
        into: &Recorder,
    ) -> Result<Value, Refuse> {
        Ok(match (function, pinned) {
            ("read", Pinned::Child { parent, name }) => {
                Self::read(&proc_fd_path(&parent, Some(&name)))
            }
            ("read" | "write", Pinned::Dir(_)) => refused(Refusal::Denied),
            ("exists", Pinned::Dir(_)) => given(function, Value::Bool(true)),
            ("exists", Pinned::Child { parent, name }) => {
                let exists = std::fs::symlink_metadata(proc_fd_path(&parent, Some(&name)))
                    .is_ok_and(|m| !m.file_type().is_symlink());
                given(function, Value::Bool(exists))
            }
            ("list", Pinned::Dir(dir)) => Self::list_open_dir(&proc_fd_path(&dir, None)),
            ("list", Pinned::Child { parent, name }) => {
                match open_dir_nofollow(&proc_fd_path(&parent, Some(&name))) {
                    Ok(dir) => Self::list_open_dir(&proc_fd_path(&dir, None)),
                    Err(e) => refused(why(&e)),
                }
            }
            ("remove", Pinned::Child { parent, name }) => {
                std::fs::remove_file(proc_fd_path(&parent, Some(&name)))
                    .map_or_else(|e| refused(why(&e)), |()| given(function, Value::Unit))
            }
            ("write", Pinned::Child { parent, name }) => {
                Self::write_in_dir(&proc_fd_path(&parent, None), &name, call, into)?
            }
            _ => refused(Refusal::Malformed),
        })
    }

    /// Touch the world, once, and say what it said.
    fn act(function: &str, at: &Path, call: &Call, into: &Recorder) -> Result<Value, Refuse> {
        Ok(match function {
            "read" => Self::read(at),
            "exists" => {
                let exists =
                    std::fs::symlink_metadata(at).is_ok_and(|m| !m.file_type().is_symlink());
                given(function, Value::Bool(exists))
            }
            "list" => Self::list(at),
            "remove" => std::fs::remove_file(at)
                .map_or_else(|e| refused(why(&e)), |()| given(function, Value::Unit)),
            "write" => Self::write(at, call, into)?,
            _ => refused(Refusal::Malformed),
        })
    }

    fn list_open_dir(at: &Path) -> Value {
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

    fn list(at: &Path) -> Value {
        #[cfg(unix)]
        {
            use std::os::unix::fs::MetadataExt as _;
            let dir = match open_dir_nofollow(at) {
                Ok(d) => d,
                Err(e) => return refused(why(&e)),
            };
            let Ok(dir_meta) = dir.metadata() else { return refused(Refusal::Denied) };
            let Ok(path_meta) = std::fs::symlink_metadata(at) else {
                return refused(Refusal::Denied);
            };
            if path_meta.file_type().is_symlink()
                || (dir_meta.dev(), dir_meta.ino()) != (path_meta.dev(), path_meta.ino())
            {
                return refused(Refusal::Denied);
            }
        }
        Self::list_open_dir(at)
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

    fn write_in_dir(
        parent: &Path,
        name: &std::ffi::OsStr,
        call: &Call,
        into: &Recorder,
    ) -> Result<Value, Refuse> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let Some(bytes) = into.arg_bytes(call, 1)? else {
            return Ok(refused(Refusal::Malformed));
        };
        let at = parent.join(name);
        if let Ok(meta) = std::fs::symlink_metadata(&at)
            && !meta.is_file()
        {
            return Ok(refused(Refusal::Denied));
        }
        let (temp, mut file) = loop {
            let n = NEXT.fetch_add(1, Ordering::Relaxed);
            let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let candidate = parent.join(format!(".nether-write.{}.{ts}.{n}", std::process::id()));
            let mut opts = std::fs::OpenOptions::new();
            opts.write(true).create_new(true);
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt as _;
                opts.custom_flags(OPEN_NOFOLLOW_NONBLOCK);
            }
            match opts.open(&candidate) {
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
        if let Err(e) = std::fs::rename(&temp, &at) {
            let _ = std::fs::remove_file(&temp);
            return Ok(refused(why(&e)));
        }
        Ok(given("write", Value::Unit))
    }

    /// §9.5: the only prelude function whose witness records something the
    /// ledger cannot later reproduce on its own. Writes via a same-directory
    /// temporary file and atomic `rename` so `ENOSPC` or a crash never
    /// truncates an existing file while recording `Refused(Exhausted)`.
    fn write(at: &Path, call: &Call, into: &Recorder) -> Result<Value, Refuse> {
        let Some(parent) = at.parent() else {
            return Ok(refused(Refusal::Denied));
        };
        let Some(name) = at.file_name() else {
            return Ok(refused(Refusal::Denied));
        };
        if let Err(e) = std::fs::create_dir_all(parent) {
            return Ok(refused(why(&e)));
        }
        Self::write_in_dir(parent, name, call, into)
    }
}
