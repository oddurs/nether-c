//! The store.
//!
//! A set of `(cairn, bytes)` pairs on disk, where the bytes are the canonical
//! encoding of a value or a node. Writing is idempotent because the name is
//! the content: two burials that compute the same value write the same bytes
//! to the same place, and need no coordination beyond each write being atomic.
//!
//! There is no garbage collection and there is not going to be one. See
//! `spec/07-ledger.md` §7.6, which also admits that this is a real cost.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::fs;
use std::io::{self, Read as _, Seek as _, SeekFrom, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::cairn::Cairn;
use crate::codec::{DecodeError, Stored, decode_stored, scan_stored};
use crate::node::Node;
use crate::value::Value;

/// Why the store could not answer.
#[derive(Debug)]
pub enum StoreError {
    /// The filesystem said no.
    Io(io::Error),
    /// Nothing of that name is here.
    Absent(Cairn),
    /// Bytes are here, and they are not what the name says they are.
    ///
    /// A store that serves these quietly is worse than one that has lost them.
    Corrupt {
        /// The name that was asked for.
        asked: Cairn,
        /// What the bytes actually hash to, if they decoded at all.
        found: Option<Cairn>,
        /// Why they did not decode, if they did not.
        why: Option<DecodeError>,
    },
    /// A short cairn matched more than one object.
    ///
    /// Picking one would be worse than refusing. `spec/07-ledger.md` §7.2.
    Ambiguous {
        /// What was asked for.
        prefix: String,
        /// A lower bound on the matches. The search stops as soon as a second
        /// one turns up, so this is what is known rather than the true total.
        at_least: usize,
    },
    /// A well-formed prefix that names nothing here.
    NoMatch(String),
    /// A prefix that is not lowercase hexadecimal, or is empty.
    BadPrefix(String),
    /// The reverse index for a cairn is not a whole number of records.
    ///
    /// A thirty-two byte append to an `O_APPEND` handle is atomic on every
    /// filesystem anybody runs, and is not atomic by guarantee. §7.5.1 requires
    /// a partial record to be detected rather than read past: reading past one
    /// would misalign every record after it and turn them into cairns nobody
    /// wrote.
    Ragged {
        /// Whose index it is.
        of: Cairn,
        /// How many bytes it holds, which is not a multiple of thirty-two.
        bytes: usize,
    },
    /// Something was handed to `put` that `get` could never read back.
    ///
    /// `Value` and `Node` can express states the encoding forbids — an unnamed
    /// struct, a stratum below the lattice, a backwards span. Writing one
    /// produces an object that is permanently unreadable and, worse, is
    /// reported afterwards as [`StoreError::Corrupt`], which is the store's
    /// signal for bit rot. The store refuses at the door instead.
    Invalid(DecodeError),
    /// A portable bundle (`spec/07-ledger.md` §7.5.4) failed framing or verification.
    Bundle(&'static str),
}

impl std::fmt::Display for StoreError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Io(e) => write!(f, "{e}"),
            Self::Absent(c) => write!(f, "nothing named {c} is in this store"),
            Self::Corrupt { asked, found: Some(found), .. } => {
                write!(f, "{asked} holds bytes that are named {found}")
            }
            Self::Corrupt { asked, why: Some(why), .. } => {
                write!(f, "{asked} holds bytes that do not decode: {why}")
            }
            Self::Corrupt { asked, .. } => write!(f, "{asked} holds bytes it should not"),
            Self::Ambiguous { prefix, at_least } => {
                write!(f, "{prefix} names at least {at_least} objects; say more of it")
            }
            Self::NoMatch(p) => write!(f, "nothing in this store starts with {p}"),
            Self::BadPrefix(p) => write!(f, "{p:?} is not the start of a cairn"),
            Self::Ragged { of, bytes } => {
                write!(f, "the reverse index for {of} is {bytes} bytes, which is not whole records")
            }
            Self::Invalid(why) => {
                write!(f, "refusing to write bytes that cannot be read back: {why}")
            }
            Self::Bundle(why) => write!(f, "invalid store bundle: {why}"),
        }
    }
}

impl std::error::Error for StoreError {}

impl From<io::Error> for StoreError {
    fn from(e: io::Error) -> Self {
        Self::Io(e)
    }
}

/// A content-addressed store rooted at a directory.
///
/// ```text
/// <root>/objects/ab/cdef...   the bytes, named by their cairn
/// <root>/refs/ab/cdef...      who points at that cairn
/// <root>/tmp/                 partial writes, never read
/// ```
///
/// The two-character fan-out is the index: resolving a short cairn of two or
/// more characters reads one directory rather than scanning the store.
#[derive(Debug, Clone)]
pub struct Store {
    root: PathBuf,
    /// What this process has already read and checked, shared by every clone.
    seen: Arc<Mutex<Seen>>,
}

/// Objects read, hashed and decoded once, kept for the next `get`. 0201.
///
/// Sound for free: the bytes behind a cairn cannot change, so a value checked
/// once is the value forever, and a later corruption on disk cannot reach a
/// reader who already has it. Only what checked is kept. An absence or a
/// corruption is asked about again, because either may be repaired.
#[derive(Debug, Default)]
struct Seen {
    held: HashMap<Cairn, Stored>,
    bytes: usize,
    reads: u64,
}

/// How many estimated heap bytes `Seen` holds before it starts again.
const SEEN_BYTES: usize = 64 << 20;
const BUNDLE_MAGIC: &[u8] = b"netherc/cairn/v4\x00bundle\x00";

/// Maximum encoded size of a single stored or bundled object (64 MiB payload +
/// 64 KiB framing). Shared across `Store::put`, `Store::export`, `Store::import`,
/// and all world providers so every provider output round-trips through bundles.
pub const MAX_OBJECT_BYTES: usize = 64 * 1024 * 1024 + 65_536;

/// Estimate the in-memory footprint of a decoded `Stored` object so dense
/// `Value::Array` trees (e.g. millions of 1-byte `Value::Unit` elements) cannot
/// pin gigabytes inside `Seen` under a raw encoded-byte budget.
fn stored_weight(stored: &Stored, encoded_len: usize) -> usize {
    let mut weight = core::mem::size_of::<Stored>();
    let mut stack: Vec<&Value> = Vec::new();
    match stored {
        Stored::Value(v) => stack.push(v),
        Stored::Node(n) => match n {
            Node::Literal(v) => stack.push(v),
            Node::Apply { args, .. } => {
                weight = weight.saturating_add(args.len().saturating_mul(32));
            }
            Node::Hole { call, .. } | Node::Witness { call, .. } => {
                weight = weight
                    .saturating_add(call.function.len())
                    .saturating_add(call.args.len().saturating_mul(32));
            }
            Node::Deposit { .. } => {}
            Node::Trace { holes, witnesses, deposits, .. } => {
                let n_refs =
                    holes.len().saturating_add(witnesses.len()).saturating_add(deposits.len());
                weight = weight.saturating_add(n_refs.saturating_mul(32));
            }
        },
    }
    let val_sz = core::mem::size_of::<Value>();
    while let Some(v) = stack.pop() {
        weight = weight.saturating_add(val_sz);
        match v {
            Value::Bytes(b) => weight = weight.saturating_add(b.len()),
            Value::Str(s) => weight = weight.saturating_add(s.len()),
            Value::Struct { name, fields } => {
                weight = weight.saturating_add(name.len());
                stack.extend(fields);
            }
            Value::Array(items) => stack.extend(items),
            Value::Answer(ans) => {
                if let crate::AnswerOf::Given(inner) = ans.as_ref() {
                    stack.push(inner);
                }
            }
            Value::Unit
            | Value::Bool(_)
            | Value::Int(_)
            | Value::Cairn(_)
            | Value::Shade { .. }
            | Value::Refusal(_) => {}
        }
    }
    weight.max(encoded_len)
}

/// `O_NONBLOCK` per Unix OS/architecture so opening a FIFO in `objects/` or
/// `refs/` fails immediately instead of blocking forever.
#[cfg(target_os = "linux")]
const OPEN_NONBLOCK: i32 = if cfg!(any(target_arch = "mips", target_arch = "mips64")) {
    0x0080
} else if cfg!(any(target_arch = "sparc", target_arch = "sparc64")) {
    0x4000
} else {
    0x0800
};
#[cfg(any(
    target_os = "macos",
    target_os = "ios",
    target_os = "freebsd",
    target_os = "openbsd",
    target_os = "netbsd"
))]
const OPEN_NONBLOCK: i32 = 0x0004;
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
compile_error!("verify O_NONBLOCK for this Unix target");

/// Removes a temporary staging file in `tmp/` when dropped.
struct TempFileGuard(PathBuf);

impl Drop for TempFileGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

/// On-disk record of an in-flight stratum-8 foreign call in `<store>/tmp/`.
///
/// Written before invoking foreign code so a callee that crashes or hangs
/// leaves an auditable record of what was attempted (`spec/09-prelude.md`
/// §9.8.1) without polluting `objects/` or `refs/` with a phantom witness.
/// Dropping the guard after the call returns removes the temporary record.
pub struct AttemptGuard(PathBuf);

impl AttemptGuard {
    /// Path of the active attempt record in `tmp/`.
    #[must_use]
    pub fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for AttemptGuard {
    fn drop(&mut self) {
        let _ = fs::remove_file(&self.0);
    }
}

fn open_regular_nonblock(path: &Path) -> io::Result<(fs::File, fs::Metadata)> {
    let mut opts = fs::OpenOptions::new();
    opts.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt as _;
        opts.custom_flags(OPEN_NONBLOCK);
    }
    let file = opts.open(path)?;
    let meta = file.metadata()?;
    if !meta.is_file() {
        return Err(io::Error::new(io::ErrorKind::InvalidInput, "not a regular file"));
    }
    Ok((file, meta))
}

fn read_bounded(path: &Path, asked: Option<Cairn>) -> Result<Vec<u8>, StoreError> {
    let too_large = |bytes: usize| match asked {
        Some(asked) => StoreError::Corrupt {
            asked,
            found: None,
            why: Some(DecodeError::TooLarge { bytes, limit: MAX_OBJECT_BYTES }),
        },
        None => StoreError::Bundle("bundle object exceeds size bound"),
    };
    let (file, meta) = match open_regular_nonblock(path) {
        Ok(ok) => ok,
        Err(e) => match (e.kind(), asked) {
            (io::ErrorKind::NotFound, Some(asked)) => return Err(StoreError::Absent(asked)),
            (io::ErrorKind::InvalidInput, Some(asked)) => {
                return Err(StoreError::Corrupt { asked, found: None, why: None });
            }
            (io::ErrorKind::InvalidInput, None) => {
                return Err(StoreError::Bundle("non-regular file in objects/"));
            }
            _ => return Err(StoreError::Io(e)),
        },
    };
    if meta.len() > MAX_OBJECT_BYTES as u64 {
        return Err(too_large(usize::try_from(meta.len()).unwrap_or(usize::MAX)));
    }
    let mut bytes = Vec::with_capacity(usize::try_from(meta.len()).unwrap_or(0).min(64 * 1024));
    file.take(MAX_OBJECT_BYTES as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > MAX_OBJECT_BYTES {
        return Err(too_large(bytes.len()));
    }
    Ok(bytes)
}

fn for_each_ref(
    path: &Path,
    of: Cairn,
    mut visit: impl FnMut([u8; 32]),
) -> Result<bool, StoreError> {
    let (mut file, meta) = match open_regular_nonblock(path) {
        Ok(ok) => ok,
        Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(false),
        Err(e) if e.kind() == io::ErrorKind::InvalidInput => {
            return Err(StoreError::Corrupt { asked: of, found: None, why: None });
        }
        Err(e) => return Err(StoreError::Io(e)),
    };
    if meta.len() % 32 != 0 {
        return Err(StoreError::Ragged {
            of,
            bytes: usize::try_from(meta.len()).unwrap_or(usize::MAX),
        });
    }
    let mut buf = [0u8; 8192];
    let mut carry = [0u8; 32];
    let mut carry_len = 0usize;
    let mut total = 0usize;
    loop {
        let n = match file.read(&mut buf) {
            Ok(0) => break,
            Ok(n) => n,
            Err(e) if e.kind() == io::ErrorKind::Interrupted => continue,
            Err(e) => return Err(StoreError::Io(e)),
        };
        total = total.saturating_add(n);
        let mut slice = &buf[..n];
        if carry_len > 0 {
            let need = 32 - carry_len;
            if slice.len() < need {
                carry[carry_len..carry_len + slice.len()].copy_from_slice(slice);
                carry_len += slice.len();
                continue;
            }
            carry[carry_len..32].copy_from_slice(&slice[..need]);
            visit(carry);
            carry_len = 0;
            slice = &slice[need..];
        }
        let chunks = slice.chunks_exact(32);
        let rem = chunks.remainder();
        for chunk in chunks {
            let mut rec = [0u8; 32];
            rec.copy_from_slice(chunk);
            visit(rec);
        }
        if !rem.is_empty() {
            carry[..rem.len()].copy_from_slice(rem);
            carry_len = rem.len();
        }
    }
    if carry_len != 0 {
        return Err(StoreError::Ragged { of, bytes: total });
    }
    Ok(true)
}

fn verify_with<T>(
    asked: Cairn,
    bytes: &[u8],
    parse: fn(&[u8]) -> Result<T, DecodeError>,
) -> Result<T, StoreError> {
    let parsed =
        parse(bytes).map_err(|why| StoreError::Corrupt { asked, found: None, why: Some(why) })?;
    let actual = Cairn::of_encoded(bytes);
    if actual != asked {
        return Err(StoreError::Corrupt { asked, found: Some(actual), why: None });
    }
    Ok(parsed)
}

impl Store {
    /// Opens, creating the layout if it is not there.
    ///
    /// # Errors
    ///
    /// If the directories cannot be created.
    pub fn open(root: impl Into<PathBuf>) -> io::Result<Self> {
        let root = root.into();
        for dir in ["objects", "refs", "tmp"] {
            fs::create_dir_all(root.join(dir))?;
        }
        Ok(Self { root, seen: Arc::default() })
    }

    /// Opens an existing store without creating any directories on disk.
    #[must_use]
    pub fn open_readonly(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into(), seen: Arc::default() }
    }

    fn fanned(&self, area: &str, cairn: Cairn) -> PathBuf {
        let hex = cairn.to_string();
        self.root.join(area).join(&hex[..2]).join(&hex[2..])
    }

    /// Whether this store holds that name.
    #[must_use]
    pub fn has(&self, cairn: Cairn) -> bool {
        self.fanned("objects", cairn).is_file()
    }

    /// Writes a value or a node, and returns its name.
    ///
    /// Idempotent: putting the same thing twice is one object and one set of
    /// reverse edges.
    ///
    /// # Errors
    ///
    /// [`StoreError::Invalid`] if the bytes would not decode or exceed
    /// [`MAX_OBJECT_BYTES`] — the store will not write something it could never
    /// read back or export — or [`StoreError::Io`] if they cannot be written.
    pub fn put(&self, stored: &Stored) -> Result<Cairn, StoreError> {
        let bytes = stored.try_encode().map_err(StoreError::Invalid)?;
        if bytes.len() > MAX_OBJECT_BYTES {
            return Err(StoreError::Invalid(DecodeError::TooLarge {
                bytes: bytes.len(),
                limit: MAX_OBJECT_BYTES,
            }));
        }

        // The round trip, before anything reaches disk. Streaming validation
        // checks the canonical grammar and extracts references without
        // materializing a second `Value` tree in memory.
        let scanned = scan_stored(&bytes).map_err(StoreError::Invalid)?;

        let cairn = Cairn::of_encoded(&bytes);
        self.put_verified(cairn, &scanned.references, &bytes)
    }

    fn create_temp(&self, hint: &str, read: bool) -> io::Result<(PathBuf, fs::File)> {
        fs::create_dir_all(self.root.join("tmp"))?;
        loop {
            let candidate = self.temp_path(hint);
            match fs::OpenOptions::new().read(read).write(true).create_new(true).open(&candidate) {
                Ok(f) => return Ok((candidate, f)),
                Err(e) if e.kind() == io::ErrorKind::AlreadyExists => {}
                Err(e) => return Err(e),
            }
        }
    }

    /// Writes a temporary stratum-8 foreign-call attempt record under `tmp/`
    /// before entering foreign code, returning a guard that deletes it after
    /// the call returns cleanly (`spec/09-prelude.md` §9.8.1).
    ///
    /// # Errors
    ///
    /// Returns [`io::Error`] if the temporary attempt file cannot be written.
    pub fn begin_attempt(&self, bytes: &[u8]) -> io::Result<AttemptGuard> {
        let (path, mut file) = self.create_temp("attempt", false)?;
        if let Err(e) = file.write_all(bytes).and_then(|()| file.flush()) {
            drop(file);
            let _ = fs::remove_file(&path);
            return Err(e);
        }
        drop(file);
        Ok(AttemptGuard(path))
    }

    fn has_raw_ref(&self, target: Cairn, referrer: Cairn) -> Result<bool, StoreError> {
        let path = self.fanned("refs", target);
        let mut found = false;
        let want = *referrer.as_bytes();
        let _ = for_each_ref(&path, target, |rec| {
            if rec == want {
                found = true;
            }
        })?;
        Ok(found)
    }

    fn put_verified(
        &self,
        cairn: Cairn,
        references: &[Cairn],
        bytes: &[u8],
    ) -> Result<Cairn, StoreError> {
        let path = self.fanned("objects", cairn);
        if let Ok(existing) = read_bounded(&path, Some(cairn))
            && verify_with(cairn, &existing, scan_stored).is_ok()
        {
            let mut targets = BTreeSet::new();
            for &target in references {
                if targets.insert(target) && !self.has_raw_ref(target, cairn)? {
                    self.add_ref(target, cairn)?;
                }
            }
            return Ok(cairn);
        }

        // Reverse edges first, object second.
        //
        // Crashing between them leaves an edge pointing at an object that is
        // not there, which `referrers` filters and a later `put` repairs. The
        // other order leaves an object whose edges were never recorded, and
        // nothing afterwards can tell that provenance is now incomplete. A
        // visible inconsistency beats a silent one.
        let mut targets = BTreeSet::new();
        for &target in references {
            if targets.insert(target) {
                self.add_ref(target, cairn)?;
            }
        }

        self.write_atomically(&path, bytes, &cairn.to_string())?;
        Ok(cairn)
    }

    /// A path nothing else will choose.
    ///
    /// PID, nanosecond timestamp, and an atomic per-process counter, paired
    /// with `create_new(true)` (`O_EXCL`) so neither threads nor recycled PIDs
    /// can collide on a temporary file.
    fn temp_path(&self, hint: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let ts = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
        self.root.join("tmp").join(format!("{hint}.{}.{ts}.{n}", std::process::id()))
    }

    fn write_atomically(&self, path: &Path, bytes: &[u8], hint: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let (temp, mut file) = self.create_temp(hint, false)?;
        if let Err(e) = file.write_all(bytes) {
            drop(file);
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        drop(file);
        if let Err(e) = fs::rename(&temp, path) {
            // Do not leave a partial behind for a reaper to puzzle over.
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
        Ok(())
    }

    /// Records that `referrer` names `target`.
    ///
    /// An append of one record, not a read-modify-write. Two writers adding an
    /// edge to the same target used to each read the old file and write their
    /// own one-entry result, so whichever renamed last erased the other's edge
    /// permanently — and nothing repaired it, because `put` early-returns on an
    /// object it already has and never reaches this code again. That is exactly
    /// the silent incompleteness the ordering in `put` is meant to avoid.
    ///
    /// Appending means a duplicate is possible where a read-modify-write would
    /// have collapsed it. `referrers` deduplicates on the way out, which is the
    /// cheap half of the trade.
    ///
    /// One record is thirty-two bytes, which every filesystem anybody runs
    /// appends atomically and none of them promises to. `referrers` checks the
    /// length rather than trusting it, so the day one is torn it is reported
    /// instead of read as a cairn nobody wrote. §7.5.1.
    fn add_ref(&self, target: Cairn, referrer: Cairn) -> io::Result<()> {
        let path = self.fanned("refs", target);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut file = fs::OpenOptions::new().create(true).append(true).open(&path)?;
        file.write_all(referrer.as_bytes())
    }

    /// Reads back what that name holds.
    ///
    /// Verifies that the bytes are what the name says they are. Bit rot in a
    /// content-addressed store is detectable for free, so not detecting it
    /// would be a choice.
    ///
    /// # Errors
    ///
    /// [`StoreError::Absent`] if nothing is there, [`StoreError::Corrupt`] if
    /// the bytes do not decode or do not hash to the name asked for.
    pub fn get(&self, cairn: Cairn) -> Result<Stored, StoreError> {
        // Held to look and to keep, not across the read: two threads reading
        // two objects should not queue for one lock while the disk works.
        {
            let mut seen = self.seen();
            if let Some(stored) = seen.held.get(&cairn) {
                return Ok(stored.clone());
            }
            seen.reads += 1;
        }
        let path = self.fanned("objects", cairn);
        let bytes = read_bounded(&path, Some(cairn))?;
        let stored = verify_with(cairn, &bytes, decode_stored)?;
        let weight = stored_weight(&stored, bytes.len());
        if weight <= SEEN_BYTES {
            let mut seen = self.seen();
            if seen.bytes.saturating_add(weight) > SEEN_BYTES {
                *seen = Seen { reads: seen.reads, ..Seen::default() };
            }
            if seen.held.insert(cairn, stored.clone()).is_none() {
                seen.bytes = seen.bytes.saturating_add(weight);
            }
        }
        Ok(stored)
    }

    /// How many times `get` has gone to the disk, in this process. Here so
    /// 0201's claim is measured rather than argued.
    #[must_use]
    pub fn disk_reads(&self) -> u64 {
        self.seen().reads
    }

    /// The lock, whether or not a thread holding it once panicked: what it
    /// guards is only ever values that checked, so nothing in it is half made.
    fn seen(&self) -> std::sync::MutexGuard<'_, Seen> {
        self.seen.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Every node that names this cairn.
    ///
    /// This is the reverse index `spec/07-ledger.md` §7.4 requires: the cost is
    /// proportional to the answer rather than to the size of the store, which
    /// is what makes walking provenance usable.
    ///
    /// # Errors
    ///
    /// [`StoreError::Io`] if the index cannot be read, and
    /// [`StoreError::Ragged`] if it is not a whole number of records.
    pub fn referrers(&self, cairn: Cairn) -> Result<Vec<Cairn>, StoreError> {
        let path = self.fanned("refs", cairn);
        let mut seen = HashSet::new();
        let mut candidates = Vec::new();
        let _ = for_each_ref(&path, cairn, |rec| {
            let referrer = Cairn::from_bytes(rec);
            // Appending admits duplicates; collapse them here.
            if seen.insert(referrer) {
                candidates.push(referrer);
            }
        })?;
        // The index is a claim, and anybody who can write the store can
        // write one. An edge counts only if its node is there, checks, and
        // names this cairn: that covers an edge that outlived its object in
        // a crash and an edge a hostile writer added. 0217.
        Ok(candidates
            .into_iter()
            .filter(
                |c| matches!(self.get(*c), Ok(Stored::Node(n)) if n.references().contains(&cairn)),
            )
            .collect())
    }

    /// Expands a short cairn to the whole one.
    ///
    /// # Errors
    ///
    /// [`StoreError::BadPrefix`] if it is not lowercase hexadecimal,
    /// [`StoreError::NoMatch`] if nothing matches, and
    /// [`StoreError::Ambiguous`] if more than one does. An ambiguous prefix is
    /// refused rather than resolved to whichever came first.
    pub fn resolve(&self, prefix: &str) -> Result<Cairn, StoreError> {
        if prefix.is_empty()
            || prefix.len() > 64
            || !prefix.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
        {
            return Err(StoreError::BadPrefix(prefix.to_owned()));
        }

        if prefix.len() == 64 {
            let cairn =
                prefix.parse::<Cairn>().map_err(|_| StoreError::BadPrefix(prefix.to_owned()))?;
            let path = self.fanned("objects", cairn);
            return match fs::metadata(&path) {
                Ok(meta) if meta.is_file() => Ok(cairn),
                Ok(_) => Err(StoreError::NoMatch(prefix.to_owned())),
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    Err(StoreError::NoMatch(prefix.to_owned()))
                }
                Err(e) => Err(StoreError::Io(e)),
            };
        }

        let objects = self.root.join("objects");
        let mut found = Vec::new();
        let buckets: Vec<PathBuf> = if prefix.len() >= 2 {
            vec![objects.join(&prefix[..2])]
        } else {
            let mut list = Vec::new();
            let entries = match fs::read_dir(&objects) {
                Ok(entries) => entries,
                Err(e) if e.kind() == io::ErrorKind::NotFound => {
                    return Err(StoreError::NoMatch(prefix.to_owned()));
                }
                Err(e) => return Err(StoreError::Io(e)),
            };
            for entry in entries {
                let entry = entry?;
                if !entry.file_type()?.is_dir() {
                    continue;
                }
                let path = entry.path();
                if path.file_name().and_then(|n| n.to_str()).is_some_and(|n| {
                    n.len() == 2
                        && n.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
                        && n.starts_with(prefix)
                }) {
                    list.push(path);
                }
            }
            list
        };

        for bucket in buckets {
            let Some(head) = bucket.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            let entries = match fs::read_dir(&bucket) {
                Ok(entries) => entries,
                Err(e) if e.kind() == io::ErrorKind::NotFound => continue,
                Err(e) => return Err(StoreError::Io(e)),
            };
            for entry in entries {
                let entry = entry?;
                if !entry.file_type()?.is_file() {
                    continue;
                }
                let Some(tail) = entry.file_name().to_str().map(str::to_owned) else { continue };
                let whole = format!("{head}{tail}");
                if whole.starts_with(prefix) {
                    if let Ok(cairn) = whole.parse::<Cairn>() {
                        found.push(cairn);
                    }
                }
                if found.len() > 1 {
                    return Err(StoreError::Ambiguous { prefix: prefix.to_owned(), at_least: 2 });
                }
            }
        }

        found.first().copied().ok_or_else(|| StoreError::NoMatch(prefix.to_owned()))
    }

    fn scan_area(
        &self,
        area: &str,
        malformed: &mut BTreeSet<PathBuf>,
    ) -> io::Result<Vec<(Cairn, PathBuf)>> {
        let mut files = Vec::new();
        for b in fs::read_dir(self.root.join(area))? {
            let b = b?;
            let path = b.path();
            let h = b.file_name().into_string().ok().filter(|s| {
                s.len() == 2 && s.bytes().all(|c| matches!(c, b'0'..=b'9' | b'a'..=b'f'))
            });
            let Some(h) = h.filter(|_| b.file_type().is_ok_and(|t| t.is_dir())) else {
                malformed.insert(path);
                continue;
            };
            for e in fs::read_dir(&path)? {
                let e = e?;
                let ep = e.path();
                let cairn = e
                    .file_name()
                    .to_str()
                    .filter(|s| s.len() == 62)
                    .and_then(|t| format!("{h}{t}").parse::<Cairn>().ok());
                if let Some(cairn) = cairn.filter(|_| e.file_type().is_ok_and(|t| t.is_file())) {
                    files.push((cairn, ep));
                } else {
                    malformed.insert(ep);
                }
            }
        }
        Ok(files)
    }

    fn audit_refs(
        &self,
        expected: BTreeMap<Cairn, BTreeSet<Cairn>>,
        malformed: &mut BTreeSet<PathBuf>,
    ) -> io::Result<Vec<IndexError>> {
        let mut index = BTreeSet::new();
        let mut recorded: BTreeMap<Cairn, BTreeSet<Cairn>> = BTreeMap::new();
        for (cairn, path) in self.scan_area("refs", malformed)? {
            let mut set = BTreeSet::new();
            match for_each_ref(&path, cairn, |rec| {
                set.insert(Cairn::from_bytes(rec));
            }) {
                Ok(true) => {
                    recorded.insert(cairn, set);
                }
                Ok(false) => {}
                Err(StoreError::Ragged { of, bytes }) => {
                    index.insert(IndexError::Ragged { of, bytes });
                }
                Err(StoreError::Io(e)) => return Err(e),
                Err(_) => {
                    malformed.insert(path);
                }
            }
        }
        for (target, referrers) in expected {
            for referrer in referrers {
                if !recorded.get(&target).is_some_and(|s| s.contains(&referrer)) {
                    index.insert(IndexError::Missing { target, referrer });
                }
            }
        }
        Ok(index.into_iter().collect())
    }

    /// Audits the store on disk without modifying it. `spec/07-ledger.md` §7.5.2.
    ///
    /// # Errors
    ///
    /// [`io::Error`] if the store directories or files cannot be read.
    pub fn audit(&self) -> io::Result<Audit> {
        struct Entry {
            is_node: bool,
            is_trace: bool,
            refs: Vec<Cairn>,
            nodes: HashSet<Cairn>,
        }
        let (mut malformed, mut verified, mut corrupt) =
            (BTreeSet::new(), BTreeMap::<Cairn, Entry>::new(), BTreeSet::new());
        for (cairn, path) in self.scan_area("objects", &mut malformed)? {
            match read_bounded(&path, Some(cairn)) {
                Ok(bytes) => {
                    if let Ok(scanned) = verify_with(cairn, &bytes, scan_stored) {
                        verified.insert(
                            cairn,
                            Entry {
                                is_node: scanned.is_node,
                                is_trace: scanned.is_trace,
                                refs: scanned.references,
                                nodes: scanned.nodes.into_iter().collect(),
                            },
                        );
                    } else {
                        corrupt.insert(cairn);
                    }
                }
                Err(StoreError::Io(e)) => return Err(e),
                Err(_) => {
                    corrupt.insert(cairn);
                }
            }
        }
        let mut missing = BTreeSet::new();
        let mut expected: BTreeMap<Cairn, BTreeSet<Cairn>> = BTreeMap::new();
        for (&referrer, entry) in &verified {
            if entry.is_node {
                for &target in &entry.refs {
                    expected.entry(target).or_default().insert(referrer);
                    if !verified.contains_key(&target) && !corrupt.contains(&target) {
                        missing.insert((referrer, target));
                    }
                }
            }
        }
        let (mut healthy_traces, mut affected_traces) = (Vec::new(), Vec::new());
        for (&trace, entry) in &verified {
            if !entry.is_trace {
                continue;
            }
            let (mut bad_c, mut bad_m, mut unknowable) = (BTreeSet::new(), BTreeSet::new(), false);
            let mut seen_r = BTreeSet::new();
            let mut todo: Vec<(Cairn, bool)> =
                entry.refs.iter().map(|&r| (r, entry.nodes.contains(&r))).collect();
            while let Some((r, is_node)) = todo.pop() {
                if !seen_r.insert((r, is_node)) {
                    continue;
                }
                match verified.get(&r) {
                    Some(child) if child.is_node => {
                        todo.extend(child.refs.iter().map(|&cr| (cr, child.nodes.contains(&cr))));
                    }
                    Some(_) if !is_node => {}
                    Some(_) => {
                        bad_c.insert(r);
                        unknowable = true;
                    }
                    None => {
                        if corrupt.contains(&r) {
                            bad_c.insert(r);
                        } else {
                            bad_m.insert(r);
                        }
                        unknowable |= is_node;
                    }
                }
            }
            if bad_c.is_empty() && bad_m.is_empty() {
                healthy_traces.push(trace);
            } else {
                affected_traces.push(AffectedTrace {
                    trace,
                    corrupt: bad_c.into_iter().collect(),
                    missing: bad_m.into_iter().collect(),
                    unknowable,
                });
            }
        }
        let index = self.audit_refs(expected, &mut malformed)?;
        Ok(Audit {
            corrupt: corrupt.into_iter().collect(),
            missing: missing.into_iter().collect(),
            malformed: malformed.into_iter().collect(),
            index,
            healthy_traces,
            affected_traces,
        })
    }

    /// Exports every verified object in topological dependency order as a
    /// portable bundle (`spec/07-ledger.md` §7.5.4), returning the count.
    ///
    /// # Errors
    ///
    /// [`StoreError::Corrupt`] if any object on disk fails verification,
    /// [`StoreError::Bundle`] if `objects/` contains a malformed filename, or
    /// [`StoreError::Io`] if reading or writing fails.
    pub fn export(&self, out: &mut impl io::Write) -> Result<usize, StoreError> {
        const MAX_BUNDLE_OBJECTS: usize = 1_000_000;
        const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;

        let mut malformed = BTreeSet::new();
        let files = self.scan_area("objects", &mut malformed)?;
        if !malformed.is_empty() {
            return Err(StoreError::Bundle("malformed path in objects/"));
        }
        if files.len() > MAX_BUNDLE_OBJECTS {
            return Err(StoreError::Bundle("bundle count exceeds bound"));
        }
        // What each object references inside this store, and its on-disk path.
        // Payload bytes are streamed from disk during emission rather than held
        // in RAM all at once.
        let mut found: BTreeMap<Cairn, (Vec<Cairn>, PathBuf)> = BTreeMap::new();
        let mut total_bytes: usize = 0;
        for (cairn, path) in files {
            let bytes = read_bounded(&path, None)?;
            total_bytes = total_bytes
                .checked_add(bytes.len())
                .filter(|&t| t <= MAX_BUNDLE_BYTES)
                .ok_or(StoreError::Bundle("bundle exceeds total size bound"))?;
            let scanned = verify_with(cairn, &bytes, scan_stored)?;
            found.insert(cairn, (scanned.references, path));
        }
        // An object's height is 0 if it references nothing here, and one more
        // than the highest thing it references otherwise. Sorting by height and
        // then by cairn is §7.5.4's order: everything an object references
        // comes before it. Walked with a stack, because a chain of traces is as
        // deep as the store is long.
        let mut height: BTreeMap<Cairn, usize> = BTreeMap::new();
        let mut open = BTreeSet::new();
        for &start in found.keys() {
            let mut todo = vec![start];
            while let Some(&c) = todo.last() {
                if height.contains_key(&c) {
                    todo.pop();
                    continue;
                }
                let refs = found[&c].0.iter().filter(|r| found.contains_key(r));
                if open.insert(c) {
                    for &r in refs {
                        // §7.3: a cairn names its references, so a cycle would
                        // be a hash that contains itself.
                        if open.contains(&r) {
                            return Err(StoreError::Bundle("a cycle in objects/"));
                        }
                        if !height.contains_key(&r) {
                            todo.push(r);
                        }
                    }
                    continue;
                }
                let h = refs.map(|r| height[r] + 1).max().unwrap_or(0);
                height.insert(c, h);
                open.remove(&c);
                todo.pop();
            }
        }
        let mut entries: Vec<(Cairn, PathBuf)> =
            found.into_iter().map(|(c, (_, path))| (c, path)).collect();
        entries.sort_by_key(|(c, _)| (height[c], *c));
        let mut hasher = blake3::Hasher::new();
        let mut send = |slice: &[u8]| -> io::Result<()> {
            hasher.update(slice);
            out.write_all(slice)
        };
        send(BUNDLE_MAGIC)?;
        send(&(entries.len() as u64).to_be_bytes())?;
        for (cairn, path) in &entries {
            let bytes = read_bounded(path, None)?;
            verify_with(*cairn, &bytes, scan_stored)?;
            send(cairn.as_bytes())?;
            send(&(bytes.len() as u64).to_be_bytes())?;
            send(&bytes)?;
        }
        out.write_all(hasher.finalize().as_bytes())?;
        Ok(entries.len())
    }

    fn create_staging_file(&self) -> io::Result<(TempFileGuard, fs::File)> {
        let (path, file) = self.create_temp("import", true)?;
        Ok((TempFileGuard(path), file))
    }

    /// Imports a portable bundle (`spec/07-ledger.md` §7.5.4), staging verified
    /// raw entries in `tmp/` until the whole-stream trailer passes before
    /// committing objects and rebuilding `refs/`, returning the count.
    ///
    /// # Errors
    ///
    /// [`StoreError::Bundle`] if framing, bounds, ordering or trailer fail,
    /// [`StoreError::Corrupt`] if an entry or existing destination object fails
    /// verification, or [`StoreError::Io`] on I/O failure.
    pub fn import(&self, src: &mut impl io::Read) -> Result<usize, StoreError> {
        const MAX_BUNDLE_OBJECTS: usize = 1_000_000;
        const MAX_BUNDLE_BYTES: usize = 512 * 1024 * 1024;

        let mut hasher = blake3::Hasher::new();
        let mut hdr = [0u8; 24];
        read_hashed(src, &mut hasher, &mut hdr)?;
        if hdr != BUNDLE_MAGIC {
            return Err(StoreError::Bundle(
                if hdr.starts_with(b"netherc/cairn/") && hdr.ends_with(b"\x00bundle\x00") {
                    "incompatible bundle domain version"
                } else {
                    "not a store bundle"
                },
            ));
        }
        let mut u8s = [0u8; 8];
        read_hashed(src, &mut hasher, &mut u8s)?;
        let count = usize::try_from(u64::from_be_bytes(u8s))
            .ok()
            .filter(|&n| n <= MAX_BUNDLE_OBJECTS)
            .ok_or(StoreError::Bundle("bundle count exceeds bound"))?;
        let (_guard, stage_file) = self.create_staging_file()?;
        let mut stage_out = io::BufWriter::new(stage_file);
        let mut seen = BTreeSet::new();
        let mut expected_before = BTreeSet::new();
        let mut total_bytes: usize = 0;
        for _ in 0..count {
            Self::stage_bundle_entry(
                src,
                &mut hasher,
                &mut stage_out,
                MAX_BUNDLE_BYTES,
                &mut total_bytes,
                &mut seen,
                &mut expected_before,
            )?;
        }
        let mut trailer = [0u8; 32];
        src.read_exact(&mut trailer).map_err(|e| {
            if e.kind() == io::ErrorKind::UnexpectedEof {
                StoreError::Bundle("truncated bundle")
            } else {
                StoreError::Io(e)
            }
        })?;
        if trailer != *hasher.finalize().as_bytes() {
            return Err(StoreError::Bundle("bundle trailer mismatch"));
        }
        loop {
            match src.read(&mut [0u8; 1]) {
                Ok(0) => break,
                Ok(_) => return Err(StoreError::Bundle("trailing bytes after bundle")),
                Err(e) if e.kind() == io::ErrorKind::Interrupted => {}
                Err(e) => return Err(StoreError::Io(e)),
            }
        }
        stage_out.flush()?;
        let mut stage_file = stage_out.into_inner().map_err(io::IntoInnerError::into_error)?;
        stage_file.seek(SeekFrom::Start(0))?;
        self.commit_staged_bundle(&mut io::BufReader::new(stage_file), count)?;
        Ok(count)
    }

    fn commit_staged_bundle(
        &self,
        staged: &mut impl io::Read,
        count: usize,
    ) -> Result<(), StoreError> {
        let mut ref_cache: HashMap<Cairn, HashSet<Cairn>> = HashMap::new();
        for _ in 0..count {
            let mut raw = [0u8; 32];
            staged.read_exact(&mut raw)?;
            let cairn = Cairn::from_bytes(raw);
            let mut len_buf = [0u8; 8];
            staged.read_exact(&mut len_buf)?;
            let len = usize::try_from(u64::from_be_bytes(len_buf))
                .map_err(|_| StoreError::Bundle("bundle object exceeds size bound"))?;
            let mut bytes = vec![0u8; len];
            staged.read_exact(&mut bytes)?;
            let scanned = verify_with(cairn, &bytes, scan_stored)?;
            if self.has(cairn) {
                // §7.5.4: verify the on-disk object directly rather than relying
                // on the in-memory `self.seen` cache.
                let existing = read_bounded(&self.fanned("objects", cairn), Some(cairn))?;
                verify_with(cairn, &existing, scan_stored)?;
                for &target in &scanned.references {
                    let known = match ref_cache.entry(target) {
                        std::collections::hash_map::Entry::Occupied(e) => e.into_mut(),
                        std::collections::hash_map::Entry::Vacant(e) => {
                            e.insert(self.referrers(target)?.into_iter().collect())
                        }
                    };
                    if known.insert(cairn) {
                        self.add_ref(target, cairn)?;
                    }
                }
            } else {
                self.put_verified(cairn, &scanned.references, &bytes)?;
            }
        }
        Ok(())
    }

    fn stage_bundle_entry(
        src: &mut impl io::Read,
        hasher: &mut blake3::Hasher,
        stage_out: &mut impl io::Write,
        max_bundle_bytes: usize,
        total_bytes: &mut usize,
        seen: &mut BTreeSet<Cairn>,
        expected_before: &mut BTreeSet<Cairn>,
    ) -> Result<(), StoreError> {
        let mut raw = [0u8; 32];
        read_hashed(src, hasher, &mut raw)?;
        let cairn = Cairn::from_bytes(raw);
        if !seen.insert(cairn) {
            return Err(StoreError::Bundle("duplicate cairn in bundle"));
        }
        if expected_before.contains(&cairn) {
            return Err(StoreError::Bundle("bundle is not in topological order"));
        }
        let mut u8s = [0u8; 8];
        read_hashed(src, hasher, &mut u8s)?;
        let len = usize::try_from(u64::from_be_bytes(u8s))
            .ok()
            .filter(|&n| n <= MAX_OBJECT_BYTES)
            .ok_or(StoreError::Bundle("bundle object exceeds size bound"))?;
        *total_bytes = total_bytes
            .checked_add(len)
            .filter(|&t| t <= max_bundle_bytes)
            .ok_or(StoreError::Bundle("bundle exceeds total size bound"))?;
        // Read incrementally so a truncated stream cannot force a 64 MiB
        // pre-allocation before hitting EOF.
        let mut bytes = Vec::with_capacity(len.min(64 * 1024));
        src.by_ref().take(len as u64).read_to_end(&mut bytes)?;
        if bytes.len() != len {
            return Err(StoreError::Bundle("truncated bundle"));
        }
        hasher.update(&bytes);
        let scanned = verify_with(cairn, &bytes, scan_stored)?;
        for target in scanned.references {
            if !seen.contains(&target) {
                expected_before.insert(target);
            }
        }
        stage_out.write_all(cairn.as_bytes())?;
        stage_out.write_all(&(len as u64).to_be_bytes())?;
        stage_out.write_all(&bytes)?;
        Ok(())
    }
}

fn read_hashed(
    src: &mut impl io::Read,
    hasher: &mut blake3::Hasher,
    buf: &mut [u8],
) -> Result<(), StoreError> {
    src.read_exact(buf).map_err(|e| {
        if e.kind() == io::ErrorKind::UnexpectedEof {
            StoreError::Bundle("truncated bundle")
        } else {
            StoreError::Io(e)
        }
    })?;
    hasher.update(buf);
    Ok(())
}

/// What is wrong with a reverse-index file in `refs/`. `spec/07-ledger.md` §7.5.2.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub enum IndexError {
    /// A file in `refs/` whose length is not a multiple of 32 bytes.
    Ragged {
        /// Whose index it is.
        of: Cairn,
        /// The byte length on disk.
        bytes: usize,
    },
    /// A verified node `referrer` names `target` and `refs/<target>` does not
    /// record `referrer`.
    Missing {
        /// The referenced cairn.
        target: Cairn,
        /// The verified node that names it.
        referrer: Cairn,
    },
}

/// A verified `Trace` node whose forward provenance chain is damaged. §7.5.2.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AffectedTrace {
    /// The `Trace` node's cairn.
    pub trace: Cairn,
    /// Reachable cairns from `trace` whose objects are corrupt.
    pub corrupt: Vec<Cairn>,
    /// Reachable cairns from `trace` that are absent from `objects/`.
    pub missing: Vec<Cairn>,
    /// `true` when one of `trace`'s node references (`holes`, `witnesses` or
    /// `deposits`) is corrupt or absent, so what that node referenced in turn
    /// cannot be known.
    pub unknowable: bool,
}

/// A read-only whole-store audit. `spec/07-ledger.md` §7.5.2.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Audit {
    /// Objects in `objects/` whose bytes do not decode or do not match their name.
    pub corrupt: Vec<Cairn>,
    /// Forward edges `(referrer, target)` from verified nodes to absent cairns.
    pub missing: Vec<(Cairn, Cairn)>,
    /// Paths under `objects/` or `refs/` that are not valid fanned cairn files.
    pub malformed: Vec<PathBuf>,
    /// Errors in the derived `refs/` reverse index.
    pub index: Vec<IndexError>,
    /// Verified `Trace` nodes whose reachable subgraph is complete and valid.
    pub healthy_traces: Vec<Cairn>,
    /// Verified `Trace` nodes with corrupt or absent dependencies.
    pub affected_traces: Vec<AffectedTrace>,
}

impl Audit {
    /// Whether the store has no corrupt objects, missing references, malformed
    /// filenames or reverse-index errors.
    #[must_use]
    pub fn is_clean(&self) -> bool {
        self.corrupt.is_empty()
            && self.missing.is_empty()
            && self.malformed.is_empty()
            && self.index.is_empty()
            && self.affected_traces.is_empty()
    }
}

/// The domain version string a current package declares (`spec/07-ledger.md` §7.5.5).
pub const PACKAGE_DOMAIN: &str = "netherc/cairn/v4";

/// Why a package could not be written or resolved offline (`spec/07-ledger.md` §7.5.5).
#[derive(Debug)]
pub enum PackageError {
    /// A package or entrypoint object was absent or corrupt in the store.
    Store(StoreError),
    /// A package `(cairn, domain)` declared a domain version other than [`PACKAGE_DOMAIN`].
    Incompatible(Cairn, String),
    /// A package or entrypoint `(cairn, why)` violated §7.5.5's structural rules.
    Malformed(Cairn, &'static str),
}

impl core::fmt::Display for PackageError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Store(e) => write!(f, "{e}"),
            Self::Incompatible(c, domain) => {
                write!(f, "package {} uses incompatible domain `{domain}`", c.short())
            }
            Self::Malformed(c, why) => write!(f, "malformed package {}: {why}", c.short()),
        }
    }
}

impl core::error::Error for PackageError {
    fn source(&self) -> Option<&(dyn core::error::Error + 'static)> {
        match self {
            Self::Store(e) => Some(e),
            Self::Incompatible(..) | Self::Malformed(..) => None,
        }
    }
}

impl From<StoreError> for PackageError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// An offline-resolved package closure (`spec/07-ledger.md` §7.5.5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedPackage {
    /// The root package cairn.
    pub root: Cairn,
    /// The root package's `entrypoint` `Bytes` cairn.
    pub entrypoint: Cairn,
    /// Direct alias-to-package-cairn bindings declared on `root`.
    pub aliases: BTreeMap<String, Cairn>,
    /// Distinct package cairns in topological post-order (dependencies first, `root` last).
    pub closure: Vec<Cairn>,
    /// Concatenated UTF-8 `entrypoint` bytes in `closure` order.
    pub unit_source: Vec<u8>,
    /// Content address of `Value::Bytes(unit_source)`.
    pub unit_cairn: Cairn,
}

impl ResolvedPackage {
    /// Resolves a direct alias declared on `root` to its exact package cairn.
    #[must_use]
    pub fn alias(&self, name: &str) -> Option<Cairn> {
        self.aliases.get(name).copied()
    }
}

/// Cairn-addressed package storage and offline dependency resolution (`spec/07-ledger.md` §7.5.5).
pub struct Package;

/// The decoded entrypoint cairn and sorted dependency list of a single
/// `Package` record. Entrypoint bytes are loaded lazily in post-order during
/// [`Package::walk`] so ancestor sources are never pinned on the DFS stack.
type LoadedPkg = (Cairn, Vec<(String, Cairn)>);

impl Package {
    /// Writes `entrypoint_source` and a canonical `"Package"` struct with
    /// sorted `deps` `(alias, package_cairn)` into `store`, returning the
    /// package cairn.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError::Malformed`] if `entrypoint_source` is not UTF-8
    /// or if `deps` has an empty or duplicate alias, or [`PackageError::Store`]
    /// if writing to `store` fails.
    pub fn put(
        store: &Store,
        entrypoint_source: &[u8],
        deps: &[(&str, Cairn)],
    ) -> Result<Cairn, PackageError> {
        let ep_val = Value::Bytes(entrypoint_source.to_vec());
        if core::str::from_utf8(entrypoint_source).is_err() {
            return Err(PackageError::Malformed(ep_val.cairn(), "entrypoint is not UTF-8"));
        }
        let mut sorted = BTreeMap::new();
        for &(alias, dep) in deps {
            if alias.is_empty() || sorted.insert(alias, dep).is_some() {
                return Err(PackageError::Malformed(ep_val.cairn(), "invalid or duplicate alias"));
            }
        }
        let entrypoint = store.put(&Stored::Value(ep_val))?;
        let dep_items = sorted
            .into_iter()
            .map(|(alias, cairn)| Value::Struct {
                name: "Dep".into(),
                fields: vec![Value::Str(alias.into()), Value::Cairn(cairn)],
            })
            .collect();
        let pkg = Value::Struct {
            name: "Package".into(),
            fields: vec![
                Value::Str(PACKAGE_DOMAIN.into()),
                Value::Cairn(entrypoint),
                Value::Array(dep_items),
            ],
        };
        Ok(store.put(&Stored::Value(pkg))?)
    }

    /// Resolves `root` and its transitive dependency closure offline in `store`.
    ///
    /// # Errors
    ///
    /// Returns [`PackageError`] if any reachable package or entrypoint object
    /// is absent, corrupt, incompatible or malformed.
    pub fn resolve(store: &Store, root: Cairn) -> Result<ResolvedPackage, PackageError> {
        let (mut visiting, mut visited) = (BTreeSet::new(), BTreeSet::new());
        let (mut closure, mut unit_source) = (Vec::new(), Vec::new());
        let (entrypoint, root_deps) =
            Self::walk(store, root, &mut visiting, &mut visited, &mut closure, &mut unit_source)?;
        let unit_cairn = Cairn::of_bytes_value(&unit_source);
        Ok(ResolvedPackage {
            root,
            entrypoint,
            aliases: root_deps.into_iter().collect(),
            closure,
            unit_source,
            unit_cairn,
        })
    }

    fn load_pkg(store: &Store, pkg: Cairn) -> Result<LoadedPkg, PackageError> {
        let Stored::Value(Value::Struct { name, fields }) = store.get(pkg)? else {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        };
        if name != "Package" {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        }
        if let Some(Value::Str(domain)) = fields.first()
            && domain != PACKAGE_DOMAIN
        {
            return Err(PackageError::Incompatible(pkg, domain.clone()));
        }
        let [Value::Str(_), Value::Cairn(ep), Value::Array(raw_deps)] = &fields[..] else {
            return Err(PackageError::Malformed(pkg, "wrong Package fields"));
        };
        let mut deps = Vec::with_capacity(raw_deps.len());
        let mut prev = "";
        for item in raw_deps {
            let Value::Struct { name: dname, fields: dfields } = item else {
                return Err(PackageError::Malformed(pkg, "not a Dep struct"));
            };
            let [Value::Str(alias), Value::Cairn(target)] = &dfields[..] else {
                return Err(PackageError::Malformed(pkg, "wrong Dep fields"));
            };
            if dname != "Dep" || alias.is_empty() || alias.as_str() <= prev || *target == pkg {
                return Err(PackageError::Malformed(pkg, "invalid or unsorted Dep"));
            }
            prev = alias;
            deps.push((alias.clone(), *target));
        }
        Ok((*ep, deps))
    }

    fn load_entrypoint(store: &Store, ep: Cairn) -> Result<Vec<u8>, PackageError> {
        let Stored::Value(Value::Bytes(bytes)) = store.get(ep)? else {
            return Err(PackageError::Malformed(ep, "entrypoint is not Bytes"));
        };
        if core::str::from_utf8(&bytes).is_err() {
            return Err(PackageError::Malformed(ep, "entrypoint is not UTF-8"));
        }
        Ok(bytes)
    }

    fn walk(
        store: &Store,
        root: Cairn,
        visiting: &mut BTreeSet<Cairn>,
        visited: &mut BTreeSet<Cairn>,
        closure: &mut Vec<Cairn>,
        unit_source: &mut Vec<u8>,
    ) -> Result<(Cairn, Vec<(String, Cairn)>), PackageError> {
        struct Frame {
            pkg: Cairn,
            ep: Cairn,
            deps: Vec<(String, Cairn)>,
            next_dep: usize,
        }
        let (root_ep, root_deps) = Self::load_pkg(store, root)?;
        visiting.insert(root);
        let mut stack =
            vec![Frame { pkg: root, ep: root_ep, deps: root_deps.clone(), next_dep: 0 }];
        while let Some(top) = stack.last_mut() {
            if top.next_dep < top.deps.len() {
                let dep = top.deps[top.next_dep].1;
                top.next_dep += 1;
                if visited.contains(&dep) {
                    continue;
                }
                if !visiting.insert(dep) {
                    return Err(PackageError::Malformed(dep, "dependency cycle"));
                }
                let (ep, deps) = Self::load_pkg(store, dep)?;
                stack.push(Frame { pkg: dep, ep, deps, next_dep: 0 });
            } else {
                let Some(done) = stack.pop() else { break };
                visiting.remove(&done.pkg);
                if visited.insert(done.pkg) {
                    let bytes = Self::load_entrypoint(store, done.ep)?;
                    let needs_nl = !bytes.is_empty()
                        && !unit_source.is_empty()
                        && !unit_source.ends_with(b"\n");
                    let next_len = unit_source
                        .len()
                        .saturating_add(usize::from(needs_nl))
                        .saturating_add(bytes.len());
                    if next_len > MAX_OBJECT_BYTES {
                        return Err(PackageError::Malformed(
                            done.pkg,
                            "resolved package source exceeds size bound",
                        ));
                    }
                    closure.push(done.pkg);
                    if needs_nl {
                        unit_source.push(b'\n');
                    }
                    unit_source.extend_from_slice(&bytes);
                }
            }
        }
        Ok((root_ep, root_deps))
    }
}

#[cfg(test)]
mod tests {
    use super::{PACKAGE_DOMAIN, Package, PackageError, Store, StoreError};
    use crate::cairn::Cairn;
    use crate::codec::Stored;
    use crate::node::{Call, Node, Span};
    use crate::value::Value;
    use std::sync::atomic::{AtomicU64, Ordering};

    /// A directory of our own, removed on drop. No dependency needed for this.
    struct Scratch(std::path::PathBuf);

    impl Scratch {
        fn new() -> Self {
            static N: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "nether-store-{}-{}",
                std::process::id(),
                N.fetch_add(1, Ordering::Relaxed)
            ));
            std::fs::create_dir_all(&path).unwrap();
            Self(path)
        }
        fn store(&self) -> Store {
            Store::open(&self.0).unwrap()
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = std::fs::remove_dir_all(&self.0);
        }
    }

    fn value(n: i64) -> Stored {
        Stored::Value(Value::Int(n))
    }

    #[test]
    fn what_goes_in_comes_out() {
        let scratch = Scratch::new();
        let store = scratch.store();
        for n in 0..64 {
            let stored = value(n);
            let cairn = store.put(&stored).unwrap();
            assert!(store.has(cairn));
            assert_eq!(store.get(cairn).unwrap(), stored);
        }
    }

    #[test]
    fn putting_is_idempotent() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let a = store.put(&value(1)).unwrap();
        let b = store.put(&value(1)).unwrap();
        assert_eq!(a, b);
    }

    #[test]
    fn the_name_is_the_content() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(42)).unwrap();
        assert_eq!(cairn, value(42).cairn());
    }

    /// Found by review. `Value` and `Node` can express states the encoding
    /// forbids; writing one produced an object `get` could never read, and the
    /// complaint it produced afterwards was `Corrupt`, which is supposed to
    /// mean the disk lied.
    #[test]
    fn the_store_refuses_what_it_could_never_read_back() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let c = Cairn::of_encoded(b"x");

        let unwritable = [
            Stored::Value(Value::Struct { name: String::new(), fields: vec![] }),
            Stored::Value(Value::Shade { origin: 200, value: c }),
            Stored::Node(Node::Witness {
                stratum: 9,
                call: Call { function: "read".to_owned(), args: vec![] },
                answer: c,
                span: Span { source: c, start: 0, end: 1 },
            }),
            Stored::Node(Node::Hole {
                call: Call { function: String::new(), args: vec![] },
                stratum: 3,
                span: Span { source: c, start: 0, end: 1 },
            }),
            Stored::Node(Node::Deposit { value: c, span: Span { source: c, start: 9, end: 4 } }),
        ];

        for stored in unwritable {
            let refused = store.put(&stored);
            assert!(
                matches!(refused, Err(StoreError::Invalid(_))),
                "the store accepted {stored:?}, which it cannot read back: {refused:?}"
            );
            assert!(!store.has(stored.cairn()), "{stored:?} reached the disk anyway");
        }
    }

    #[test]
    fn absence_is_not_an_error_in_disguise() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let never = Cairn::of_encoded(b"never stored");
        assert!(!store.has(never));
        assert!(matches!(store.get(never), Err(StoreError::Absent(_))));
    }

    /// Bit rot is detectable for free here, so not detecting it is a choice.
    #[test]
    fn tampering_is_caught_rather_than_served() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(7)).unwrap();

        let hex = cairn.to_string();
        let path = scratch.0.join("objects").join(&hex[..2]).join(&hex[2..]);
        let mut bytes = std::fs::read(&path).unwrap();
        *bytes.last_mut().unwrap() ^= 0xff;
        std::fs::write(&path, &bytes).unwrap();

        match store.get(cairn) {
            Err(StoreError::Corrupt { asked, found: Some(found), .. }) => {
                assert_eq!(asked, cairn);
                assert_ne!(found, cairn);
            }
            other => panic!("tampering was not caught: {other:?}"),
        }
    }

    // ── the reverse index ───────────────────────────────────────────────────

    fn node_pointing_at(target: Cairn) -> Stored {
        Stored::Node(Node::Apply {
            function: Cairn::of_encoded(b"f"),
            args: vec![target],
            result: Cairn::of_encoded(b"r"),
        })
    }

    /// §7.5.1: a partial record is detected rather than read past.
    ///
    /// One record is thirty-two bytes, and appending that many is atomic on
    /// every filesystem anybody runs and promised by none of them. Reading
    /// past a torn one would misalign every record after it and hand back
    /// cairns nobody wrote — a wrong edge in a provenance walk, which is worse
    /// than a missing one.
    #[test]
    fn a_torn_reverse_edge_is_reported_and_not_read_past() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let target = store.put(&value(1)).unwrap();
        let node = store
            .put(&Stored::Node(Node::Apply {
                function: target,
                args: vec![target],
                result: target,
            }))
            .unwrap();
        assert_eq!(store.referrers(target).unwrap(), vec![node]);

        let hex = target.to_string();
        let refs = scratch.0.join("refs").join(&hex[..2]).join(&hex[2..]);
        let mut bytes = std::fs::read(&refs).unwrap();
        bytes.truncate(bytes.len() - 7);
        std::fs::write(&refs, &bytes).unwrap();

        match store.referrers(target) {
            Err(StoreError::Ragged { of, bytes: n }) => {
                assert_eq!(of, target);
                assert_eq!(n % 32, 25);
            }
            other => panic!("a torn record was not reported: {other:?}"),
        }
    }

    #[test]
    fn referrers_finds_who_points_here() {
        let scratch = Scratch::new();
        let store = scratch.store();

        let target = store.put(&value(1)).unwrap();
        assert!(store.referrers(target).unwrap().is_empty());

        let referrer = store.put(&node_pointing_at(target)).unwrap();
        assert_eq!(store.referrers(target).unwrap(), vec![referrer]);
    }

    #[test]
    fn putting_twice_does_not_duplicate_an_edge() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let target = store.put(&value(1)).unwrap();
        let node = node_pointing_at(target);
        store.put(&node).unwrap();
        store.put(&node).unwrap();
        assert_eq!(store.referrers(target).unwrap().len(), 1);
    }

    /// An edge written just before a crash can outlive its object.
    #[test]
    fn an_edge_to_a_missing_object_is_not_reported() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let target = store.put(&value(1)).unwrap();
        let referrer = store.put(&node_pointing_at(target)).unwrap();

        let hex = referrer.to_string();
        std::fs::remove_file(scratch.0.join("objects").join(&hex[..2]).join(&hex[2..])).unwrap();

        assert!(store.referrers(target).unwrap().is_empty());
    }

    // ── 0217: a writer with the whole store cannot make a reader see a lie ──

    /// A forged edge says a node produced this cairn. The node exists and
    /// checks, and does not name it, so it is not believed.
    #[test]
    fn a_forged_edge_is_not_a_referrer() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let target = store.put(&value(1)).unwrap();
        let elsewhere = store.put(&node_pointing_at(store.put(&value(2)).unwrap())).unwrap();
        store.add_ref(target, elsewhere).unwrap();
        assert!(store.referrers(target).unwrap().is_empty());
    }

    /// Bytes replaced under their name, and a name written for bytes it does
    /// not hold, are refusals: a writer can make a read fail, and cannot make
    /// it succeed with the wrong thing.
    #[test]
    fn misnamed_bytes_are_refused_however_they_were_found() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let real = store.put(&value(3)).unwrap();
        std::fs::write(store.fanned("objects", real), value(4).encode()).unwrap();
        assert!(matches!(store.get(real), Err(StoreError::Corrupt { .. })));

        // A file named for a cairn nobody wrote, holding somebody else's
        // bytes. `resolve` reads names, so it finds it; `get` reads bytes.
        let named = Stored::Value(Value::Int(5)).cairn();
        let path = store.fanned("objects", named);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, value(6).encode()).unwrap();
        let found = store.resolve(&named.to_string()[..12]).unwrap();
        assert!(matches!(store.get(found), Err(StoreError::Corrupt { .. })));
    }

    /// A reader racing a writer that keeps replacing the object sees the
    /// object or a refusal, and never another value.
    #[test]
    fn a_reader_racing_a_replacing_writer_never_sees_another_value() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(7)).unwrap();
        let path = store.fanned("objects", cairn);
        let (good, bad) = (value(7).encode(), value(8).encode());
        let done = std::sync::Arc::new(std::sync::atomic::AtomicBool::new(false));
        let writer = {
            let (path, done, good, bad) = (path.clone(), done.clone(), good.clone(), bad.clone());
            std::thread::spawn(move || {
                for i in 0..2000 {
                    let tmp = path.with_extension(format!("w{i}"));
                    std::fs::write(&tmp, if i % 2 == 0 { &bad } else { &good }).unwrap();
                    std::fs::rename(&tmp, &path).unwrap();
                }
                done.store(true, Ordering::Relaxed);
            })
        };
        while !done.load(Ordering::Relaxed) {
            // A fresh store each time, so nothing is remembered between reads.
            match Store::open(&scratch.0).unwrap().get(cairn) {
                Ok(v) => assert_eq!(v, value(7), "a read returned another value"),
                Err(StoreError::Corrupt { .. } | StoreError::Absent(_)) => {}
                Err(e) => panic!("{e}"),
            }
        }
        writer.join().unwrap();
    }

    #[test]
    fn a_node_registers_every_cairn_it_names() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let source = store.put(&value(0)).unwrap();
        let arg = store.put(&value(1)).unwrap();

        let hole = Node::Hole {
            call: Call { function: "read".to_owned(), args: vec![arg] },
            stratum: 3,
            span: Span { source, start: 0, end: 1 },
        };
        let cairn = store.put(&Stored::Node(hole)).unwrap();

        assert_eq!(store.referrers(arg).unwrap(), vec![cairn]);
        assert_eq!(store.referrers(source).unwrap(), vec![cairn]);
    }

    // ── concurrency ─────────────────────────────────────────────────────────
    //
    // Spec 7.5: two burials that compute the same value write the same bytes to
    // the same place and need no coordination beyond each write being atomic.
    // Both of these failed before review found them.

    /// Sixteen writers, one shared target. Every edge must survive.
    #[test]
    fn concurrent_writers_do_not_lose_reverse_edges() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let target = store.put(&value(0)).unwrap();

        let mut expected = Vec::new();
        let handles: Vec<_> = (0..16)
            .map(|i| {
                let store = store.clone();
                let node = Stored::Node(Node::Apply {
                    function: Cairn::of_encoded(format!("f{i}").as_bytes()),
                    args: vec![target],
                    result: Cairn::of_encoded(b"r"),
                });
                expected.push(node.cairn());
                std::thread::spawn(move || store.put(&node).expect("a concurrent put failed"))
            })
            .collect();

        for handle in handles {
            handle.join().expect("a writer panicked");
        }

        let found = store.referrers(target).unwrap();
        for cairn in &expected {
            assert!(found.contains(cairn), "edge from {cairn:?} was lost");
        }
        assert_eq!(found.len(), expected.len(), "duplicate edges survived");
    }

    /// Sixteen writers, all storing the identical object.
    #[test]
    fn concurrent_writers_of_one_object_all_succeed() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let handles: Vec<_> = (0..16)
            .map(|_| {
                let store = store.clone();
                std::thread::spawn(move || store.put(&value(99)).expect("a concurrent put failed"))
            })
            .collect();

        let names: Vec<Cairn> = handles.into_iter().map(|h| h.join().unwrap()).collect();
        assert!(names.windows(2).all(|w| w[0] == w[1]), "one object, one name");
        assert_eq!(store.get(names[0]).unwrap(), value(99));
    }

    /// A handle outlives the directory when something clears `/tmp`.
    #[test]
    fn a_cleared_tmp_directory_does_not_break_the_handle() {
        let scratch = Scratch::new();
        let store = scratch.store();
        store.put(&value(1)).unwrap();

        std::fs::remove_dir_all(scratch.0.join("tmp")).unwrap();

        let cairn = store.put(&value(2)).expect("put failed after tmp was cleared");
        assert_eq!(store.get(cairn).unwrap(), value(2));
    }

    /// Nothing is left in `tmp` once a write has landed.
    #[test]
    fn writes_do_not_litter() {
        let scratch = Scratch::new();
        let store = scratch.store();
        for n in 0..8 {
            store.put(&node_pointing_at(store.put(&value(n)).unwrap())).unwrap();
        }
        let left: Vec<_> = std::fs::read_dir(scratch.0.join("tmp")).unwrap().collect();
        assert!(left.is_empty(), "{} files left in tmp", left.len());
    }

    // ── resolving a short cairn ─────────────────────────────────────────────

    #[test]
    fn a_short_cairn_expands() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(5)).unwrap();
        assert_eq!(store.resolve(&cairn.short()).unwrap(), cairn);
        assert_eq!(store.resolve(&cairn.to_string()).unwrap(), cairn);
    }

    #[test]
    fn an_ambiguous_prefix_is_refused_not_guessed() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let mut first = None;
        for n in 0..256 {
            let cairn = store.put(&value(n)).unwrap();
            first.get_or_insert(cairn);
        }
        // One hex character over 256 objects is ambiguous with near certainty.
        let head = &first.unwrap().to_string()[..1];
        match store.resolve(head) {
            Err(StoreError::Ambiguous { at_least, .. }) => assert!(at_least >= 2),
            other => panic!("a one-character prefix resolved to {other:?}"),
        }
    }

    #[test]
    fn a_prefix_that_names_nothing_says_so() {
        let scratch = Scratch::new();
        let store = scratch.store();
        store.put(&value(1)).unwrap();
        assert!(matches!(store.resolve("deadbeef"), Err(StoreError::NoMatch(_))));
    }

    #[test]
    fn a_prefix_that_is_not_a_prefix_is_rejected() {
        let scratch = Scratch::new();
        let store = scratch.store();
        for bad in ["", "XYZ", "ABCD", "zz", "12 34"] {
            assert!(
                matches!(store.resolve(bad), Err(StoreError::BadPrefix(_))),
                "accepted {bad:?}"
            );
        }
    }

    // ── 0201: what `get` keeps ──────────────────────────────────────────────

    #[test]
    fn a_thousand_gets_of_one_object_read_the_disk_once() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(7)).unwrap();
        for _ in 0..1000 {
            assert_eq!(store.get(cairn).unwrap(), value(7));
        }
        assert_eq!(store.disk_reads(), 1);
        // A clone is the same store and shares what it has read.
        assert_eq!(store.clone().get(cairn).unwrap(), value(7));
        assert_eq!(store.disk_reads(), 1);
    }

    #[test]
    fn a_failure_is_asked_again_and_not_remembered() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = Stored::Value(Value::Int(9)).cairn();
        assert!(matches!(store.get(cairn), Err(StoreError::Absent(_))));
        store.put(&value(9)).unwrap();
        assert_eq!(store.get(cairn).unwrap(), value(9), "an absence was kept");

        // A corrupt first read, repaired, reads as the object.
        let other = store.put(&value(10)).unwrap();
        let path = store.fanned("objects", other);
        let good = std::fs::read(&path).unwrap();
        std::fs::write(&path, b"not it").unwrap();
        assert!(matches!(store.get(other), Err(StoreError::Corrupt { .. })));
        std::fs::write(&path, good).unwrap();
        assert_eq!(store.get(other).unwrap(), value(10), "a corruption was kept");
    }

    #[test]
    fn what_was_read_is_what_is_returned_whatever_the_disk_says_later() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let cairn = store.put(&value(11)).unwrap();
        assert_eq!(store.get(cairn).unwrap(), value(11));
        // Replaced on disk by something else: the name is the content, and the
        // content was checked, so the reader keeps what it checked.
        std::fs::write(store.fanned("objects", cairn), value(12).encode()).unwrap();
        assert_eq!(store.get(cairn).unwrap(), value(11));
    }

    #[test]
    fn what_is_kept_stays_inside_its_bound() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let big = |b: u8| Stored::Value(Value::Bytes(vec![b; super::SEEN_BYTES / 2 + 1]));
        let (a, b) = (store.put(&big(1)).unwrap(), store.put(&big(2)).unwrap());
        store.get(a).unwrap();
        store.get(b).unwrap();
        assert!(store.seen().bytes <= super::SEEN_BYTES, "{}", store.seen().bytes);
        assert_eq!(store.get(b).unwrap(), big(2));
        assert_eq!(store.disk_reads(), 2, "the newest object was not the one kept");
    }

    // ── 0214: what a damaged store can still tell you ───────────────────────

    fn stored_trace(store: &Store, tag: i64) -> (Cairn, Cairn, Cairn, Cairn) {
        let src = store.put(&Stored::Value(Value::Bytes(format!("s{tag}").into_bytes()))).unwrap();
        let res = store.put(&Stored::Value(Value::Bytes(format!("r{tag}").into_bytes()))).unwrap();
        let val = store.put(&value(tag)).unwrap();
        let span = Span { source: src, start: 0, end: 1 };
        let dep = store.put(&Stored::Node(Node::Deposit { value: val, span })).unwrap();
        let wit = store
            .put(&Stored::Node(Node::Witness {
                stratum: 3,
                call: Call { function: "read".to_owned(), args: vec![val] },
                answer: val,
                span,
            }))
            .unwrap();
        let trace = store
            .put(&Stored::Node(Node::Trace {
                residue: res,
                holes: Vec::new(),
                witnesses: vec![wit],
                deposits: vec![dep],
                source: src,
                depth: 3,
                unrecorded: false,
            }))
            .unwrap();
        (trace, val, dep, wit)
    }

    #[test]
    fn n_corrupt_objects_are_reported_and_affected_traces_are_named() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let (healthy, ok_val, _, _) = stored_trace(&store, 1);
        let (leaf_bad, bad_val, _, _) = stored_trace(&store, 2);
        let (node_bad, _, bad_dep, bad_wit) = stored_trace(&store, 3);
        assert!(store.audit().unwrap().is_clean());

        for c in [bad_val, bad_dep, bad_wit] {
            std::fs::write(store.fanned("objects", c), b"tampered").unwrap();
        }

        let audit = store.audit().unwrap();
        let mut expected_corrupt = vec![bad_val, bad_dep, bad_wit];
        expected_corrupt.sort();
        assert_eq!(audit.corrupt, expected_corrupt);
        assert!(audit.missing.is_empty());
        assert!(audit.malformed.is_empty());
        assert_eq!(audit.healthy_traces, vec![healthy]);
        assert_eq!(store.get(ok_val).unwrap(), value(1));

        let by_leaf = audit.affected_traces.iter().find(|a| a.trace == leaf_bad).expect("leaf_bad");
        assert_eq!(by_leaf.corrupt, vec![bad_val]);
        assert!(!by_leaf.unknowable, "a corrupt leaf value has no outgoing references to hide");

        let by_node = audit.affected_traces.iter().find(|a| a.trace == node_bad).expect("node_bad");
        let mut node_corrupt = vec![bad_dep, bad_wit];
        node_corrupt.sort();
        assert_eq!(by_node.corrupt, node_corrupt);
        assert!(by_node.unknowable, "corrupt deposit/witness nodes have unknowable references");
    }

    #[test]
    fn broken_indexes_malformed_names_and_absent_children_are_distinguished() {
        let scratch = Scratch::new();
        let store = scratch.store();
        let (healthy, ok_val, _, _) = stored_trace(&store, 10);
        let (missing_node, _, _, gone_wit) = stored_trace(&store, 20);

        // Break the reverse index of `healthy` and remove `gone_wit` from `objects/`.
        std::fs::write(store.fanned("refs", ok_val), [0u8; 7]).unwrap();
        std::fs::remove_file(store.fanned("objects", gone_wit)).unwrap();
        let stray_dir = scratch.0.join("objects").join("zz");
        std::fs::create_dir_all(&stray_dir).unwrap();
        let stray_obj = stray_dir.join("not-hex");
        std::fs::write(&stray_obj, b"x").unwrap();

        let audit = store.audit().unwrap();
        assert!(audit.corrupt.is_empty(), "{audit:?}");
        // `healthy` is still healthy: affected traces come from verified forward edges, not `refs/`.
        assert_eq!(audit.healthy_traces, vec![healthy]);
        assert_eq!(audit.missing, vec![(missing_node, gone_wit)]);
        assert_eq!(audit.malformed, vec![stray_dir]);
        assert!(
            audit.index.contains(&super::IndexError::Ragged { of: ok_val, bytes: 7 }),
            "{:?}",
            audit.index
        );
        let affected =
            audit.affected_traces.iter().find(|a| a.trace == missing_node).expect("missing_node");
        assert_eq!(affected.missing, vec![gone_wit]);
        assert!(affected.unknowable);
    }

    // ── 0213: a store that outlives its machine ─────────────────────────────

    #[test]
    fn an_exported_store_imported_into_an_empty_one_rebuilds_refs_and_answers_every_cairn() {
        let src_dir = Scratch::new();
        let src = src_dir.store();
        let (t1, v1, d1, w1) = stored_trace(&src, 101);
        let (t2, v2, d2, w2) = stored_trace(&src, 202);
        let hole = src
            .put(&Stored::Node(Node::Hole {
                call: Call { function: "read".to_owned(), args: vec![v1] },
                stratum: 3,
                span: Span { source: v1, start: 0, end: 1 },
            }))
            .unwrap();
        let staged = src
            .put(&Stored::Node(Node::Trace {
                residue: v2,
                holes: vec![hole],
                witnesses: vec![w1],
                deposits: vec![d1],
                source: v1,
                depth: 3,
                unrecorded: false,
            }))
            .unwrap();

        let mut bundle = Vec::new();
        let exported = src.export(&mut bundle).unwrap();
        assert_eq!(exported, 14);

        let dst_dir = Scratch::new();
        let dst = dst_dir.store();
        let imported = dst.import(&mut &bundle[..]).unwrap();
        assert_eq!(imported, exported);
        assert!(dst.audit().unwrap().is_clean());
        assert_eq!(dst.audit().unwrap().healthy_traces, src.audit().unwrap().healthy_traces);

        for c in [t1, v1, d1, w1, t2, v2, d2, w2, hole, staged] {
            assert_eq!(dst.get(c).unwrap(), src.get(c).unwrap());
            assert_eq!(dst.resolve(&c.short()).unwrap(), c);
            let mut dst_refs = dst.referrers(c).unwrap();
            let mut src_refs = src.referrers(c).unwrap();
            dst_refs.sort();
            src_refs.sort();
            assert_eq!(dst_refs, src_refs, "referrers differed for {c}");
        }

        // Remove a derived `refs/` file in `dst`, re-import the same bundle,
        // and verify that existing objects are kept and `refs/` is repaired.
        std::fs::remove_file(dst.fanned("refs", v1)).unwrap();
        assert!(!dst.audit().unwrap().is_clean());
        assert_eq!(dst.import(&mut &bundle[..]).unwrap(), exported);
        assert!(dst.audit().unwrap().is_clean());
    }

    #[test]
    fn truncated_corrupt_and_incompatible_bundles_are_refused_and_partial_imports_stay_clean() {
        let src_dir = Scratch::new();
        let src = src_dir.store();
        let (trace, val, _, _) = stored_trace(&src, 77);
        let mut bundle = Vec::new();
        let total = src.export(&mut bundle).unwrap();

        // Every truncated prefix fails with `Bundle("truncated bundle")`, and
        // because `export` emits leaves before referrers, whatever prefix was
        // committed before EOF is dependency-closed and `audit` is clean.
        for cut in [0, 10, 24, 31, 40, bundle.len() / 2, bundle.len() - 33, bundle.len() - 1] {
            let dst_dir = Scratch::new();
            let dst = dst_dir.store();
            assert!(
                matches!(
                    dst.import(&mut &bundle[..cut]),
                    Err(StoreError::Bundle("truncated bundle"))
                ),
                "cut at {cut} was not refused as truncated"
            );
            assert!(dst.audit().unwrap().is_clean(), "cut at {cut} left an unclean store");
            // Resuming with the full bundle finishes the import cleanly.
            assert_eq!(dst.import(&mut &bundle[..]).unwrap(), total);
            assert!(dst.audit().unwrap().is_clean());
            assert!(dst.audit().unwrap().healthy_traces.contains(&trace));
        }

        // Wrong domain version vs non-bundle magic.
        let dst_dir = Scratch::new();
        let dst = dst_dir.store();
        let mut v3 = bundle.clone();
        v3[15] = b'3';
        assert!(matches!(
            dst.import(&mut &v3[..]),
            Err(StoreError::Bundle("incompatible bundle domain version"))
        ));
        assert!(matches!(
            dst.import(&mut &b"not-a-netherc-bundle-at-all"[..]),
            Err(StoreError::Bundle("not a store bundle"))
        ));

        // Corrupt payload bytes inside the first entry are caught before any
        // object is written to the destination store.
        let mut bad_payload = bundle.clone();
        bad_payload[24 + 8 + 32 + 8] ^= 0xff;
        assert!(matches!(dst.import(&mut &bad_payload[..]), Err(StoreError::Corrupt { .. })));
        assert!(!dst.has(val));

        // Tampered trailer and trailing garbage.
        let mut bad_trailer = bundle.clone();
        *bad_trailer.last_mut().unwrap() ^= 0x01;
        let t_dir = Scratch::new();
        assert!(matches!(
            t_dir.store().import(&mut &bad_trailer[..]),
            Err(StoreError::Bundle("bundle trailer mismatch"))
        ));
        let mut trailing = bundle.clone();
        trailing.push(0x00);
        let g_dir = Scratch::new();
        assert!(matches!(
            g_dir.store().import(&mut &trailing[..]),
            Err(StoreError::Bundle("trailing bytes after bundle"))
        ));

        // Exporting a store with a corrupt object is refused.
        std::fs::write(src.fanned("objects", val), b"corrupt").unwrap();
        assert!(matches!(src.export(&mut Vec::new()), Err(StoreError::Corrupt { .. })));
    }

    #[test]
    fn two_disconnected_stores_resolve_the_same_package_closure_to_identical_cairns_offline() {
        let dir_a = Scratch::new();
        let dir_b = Scratch::new();
        let store_a = dir_a.store();
        let store_b = dir_b.store();

        let base_src = b"I64 base_step(I64 x) { return x + 10; }\n";
        let math_src = b"I64 math_dbl(I64 x) { return base_step(x) * 2; }\n";
        let fmt_src = b"I64 fmt_tag(I64 x) { return base_step(x) + 1; }\n";
        let app_src = b"U0 main_pkg() { math_dbl(3) + fmt_tag(4); }\ndemand main_pkg();\n";

        // Machine A populates in bottom-up order with deps given as [fmt, math].
        let base_a = Package::put(&store_a, base_src, &[]).unwrap();
        let math_a = Package::put(&store_a, math_src, &[("core", base_a)]).unwrap();
        let fmt_a = Package::put(&store_a, fmt_src, &[("base", base_a)]).unwrap();
        let app_a = Package::put(&store_a, app_src, &[("fmt", fmt_a), ("math", math_a)]).unwrap();

        // Machine B populates independently (with unrelated objects and reversed
        // dep slice order [math, fmt]) without ever speaking to Machine A.
        store_b.put(&Stored::Value(Value::Int(999))).unwrap();
        let base_b = Package::put(&store_b, base_src, &[]).unwrap();
        let fmt_b = Package::put(&store_b, fmt_src, &[("base", base_b)]).unwrap();
        let math_b = Package::put(&store_b, math_src, &[("core", base_b)]).unwrap();
        let app_b = Package::put(&store_b, app_src, &[("math", math_b), ("fmt", fmt_b)]).unwrap();

        assert_eq!(base_a, base_b);
        assert_eq!(math_a, math_b);
        assert_eq!(fmt_a, fmt_b);
        assert_eq!(app_a, app_b);

        let res_a = Package::resolve(&store_a, app_a).unwrap();
        let res_b = Package::resolve(&store_b, app_b).unwrap();
        assert_eq!(res_a, res_b);
        assert_eq!(res_a.alias("math"), Some(math_a));
        assert_eq!(res_a.alias("fmt"), Some(fmt_a));
        assert_eq!(res_a.alias("unknown"), None);
        assert_eq!(res_a.closure, vec![base_a, fmt_a, math_a, app_a]);

        // Also verify resolution after portable bundle transfer into a third store.
        let mut bundle = Vec::new();
        store_a.export(&mut bundle).unwrap();
        let dir_c = Scratch::new();
        let store_c = dir_c.store();
        store_c.import(&mut &bundle[..]).unwrap();
        assert_eq!(Package::resolve(&store_c, app_a).unwrap(), res_a);
    }

    #[test]
    fn missing_corrupt_incompatible_and_malformed_packages_fail_explicitly() {
        let dir = Scratch::new();
        let store = dir.store();

        let base = Package::put(&store, b"I64 f() { return 1; }\n", &[]).unwrap();
        let missing_dep = Cairn::of_encoded(b"absent-package");
        let with_missing =
            Package::put(&store, b"I64 g() { return f(); }\n", &[("missing", missing_dep)])
                .unwrap();

        // Missing dependency package cairn fails explicitly with Absent(missing_dep).
        assert!(matches!(
            Package::resolve(&store, with_missing),
            Err(PackageError::Store(StoreError::Absent(c))) if c == missing_dep
        ));

        // Incompatible domain version in a dependency fails with Incompatible.
        let ep =
            store.put(&Stored::Value(Value::Bytes(b"I64 h() { return 2; }\n".to_vec()))).unwrap();
        let v3_pkg = store
            .put(&Stored::Value(Value::Struct {
                name: "Package".into(),
                fields: vec![
                    Value::Str("netherc/cairn/v3".into()),
                    Value::Cairn(ep),
                    Value::Array(vec![]),
                ],
            }))
            .unwrap();
        let root_v3 = Package::put(&store, b"I64 k() { return 3; }\n", &[("old", v3_pkg)]).unwrap();
        assert!(matches!(
            Package::resolve(&store, root_v3),
            Err(PackageError::Incompatible(c, ref d)) if c == v3_pkg && d == "netherc/cairn/v3"
        ));

        // Unsorted Dep entries and non-UTF-8 entrypoints are rejected.
        assert!(matches!(
            Package::put(&store, &[0xff, 0xfe], &[]),
            Err(PackageError::Malformed(_, "entrypoint is not UTF-8"))
        ));
        assert!(matches!(
            Package::put(&store, b"I64 x() { return 0; }\n", &[("dup", base), ("dup", base)]),
            Err(PackageError::Malformed(_, "invalid or duplicate alias"))
        ));
        let unsorted_pkg = store
            .put(&Stored::Value(Value::Struct {
                name: "Package".into(),
                fields: vec![
                    Value::Str(PACKAGE_DOMAIN.into()),
                    Value::Cairn(ep),
                    Value::Array(vec![
                        Value::Struct {
                            name: "Dep".into(),
                            fields: vec![Value::Str("z".into()), Value::Cairn(base)],
                        },
                        Value::Struct {
                            name: "Dep".into(),
                            fields: vec![Value::Str("a".into()), Value::Cairn(base)],
                        },
                    ]),
                ],
            }))
            .unwrap();
        assert!(matches!(
            Package::resolve(&store, unsorted_pkg),
            Err(PackageError::Malformed(c, "invalid or unsorted Dep")) if c == unsorted_pkg
        ));

        // Corrupting the base package on disk fails with Corrupt(base).
        std::fs::write(store.fanned("objects", base), b"damaged").unwrap();
        let top = Package::put(&store, b"I64 top() { return 0; }\n", &[("b", base)]).unwrap();
        assert!(matches!(
            Package::resolve(&store, top),
            Err(PackageError::Store(StoreError::Corrupt { asked, .. })) if asked == base
        ));
    }

    #[test]
    fn import_enforces_topological_order_and_put_repairs_corrupt_object_and_deep_packages_resolve()
    {
        let src_dir = Scratch::new();
        let src = src_dir.store();
        let val = src.put(&Stored::Value(Value::Int(42))).unwrap();
        let dep = src
            .put(&Stored::Node(Node::Deposit {
                value: val,
                span: Span { source: val, start: 0, end: 1 },
            }))
            .unwrap();

        // Build a bundle with the referrer (`dep`) before its leaf (`val`).
        let mut reversed = Vec::new();
        let mut hasher = blake3::Hasher::new();
        let mut push = |slice: &[u8]| {
            hasher.update(slice);
            reversed.extend_from_slice(slice);
        };
        push(super::BUNDLE_MAGIC);
        push(&2u64.to_be_bytes());
        for c in [dep, val] {
            let bytes = src.get(c).unwrap().encode();
            push(c.as_bytes());
            push(&(bytes.len() as u64).to_be_bytes());
            push(&bytes);
        }
        reversed.extend_from_slice(hasher.finalize().as_bytes());

        let dst_dir = Scratch::new();
        let dst = dst_dir.store();
        assert!(matches!(
            dst.import(&mut &reversed[..]),
            Err(StoreError::Bundle("bundle is not in topological order"))
        ));
        assert!(dst.audit().unwrap().is_clean(), "rejected bundle must not leave dangling objects");

        // Package with incompatible domain version and non-3 field shape returns Incompatible.
        let v3_short = src
            .put(&Stored::Value(Value::Struct {
                name: "Package".into(),
                fields: vec![Value::Str("netherc/cairn/v3".into())],
            }))
            .unwrap();
        assert!(matches!(
            Package::resolve(&src, v3_short),
            Err(PackageError::Incompatible(c, ref d)) if c == v3_short && d == "netherc/cairn/v3"
        ));

        // Stray files or non-hex directories under objects/ do not break 1-char resolve.
        let prefix = val.to_string();
        std::fs::write(src.root.join("objects").join("a_stray_file"), b"junk").unwrap();
        std::fs::create_dir_all(src.root.join("objects").join("a_non_hex_dir")).unwrap();
        assert!(src.resolve(&prefix[..2]).is_ok());

        // Putting a valid value overwrites a corrupt on-disk file under that cairn.
        std::fs::write(src.fanned("objects", val), b"corrupt").unwrap();
        let fresh = src_dir.store();
        assert!(matches!(fresh.get(val), Err(StoreError::Corrupt { .. })));
        assert_eq!(fresh.put(&Stored::Value(Value::Int(42))).unwrap(), val);
        assert_eq!(fresh.get(val).unwrap(), Stored::Value(Value::Int(42)));

        // Package::resolve inserts a newline separator when an entrypoint lacks one
        // and handles deep chains iteratively without stack overflow.
        let pkg_dir = Scratch::new();
        let pkg_store = pkg_dir.store();
        let mut cur = Package::put(&pkg_store, b"I64 p0() { 0 }", &[]).unwrap();
        for i in 1..256 {
            cur = Package::put(
                &pkg_store,
                format!("I64 p{i}() {{ p{}() }}", i - 1).as_bytes(),
                &[("prev", cur)],
            )
            .unwrap();
        }
        let resolved = Package::resolve(&pkg_store, cur).unwrap();
        assert_eq!(resolved.closure.len(), 256);
        let text = std::str::from_utf8(&resolved.unit_source).unwrap();
        assert!(text.contains("I64 p0() { 0 }\nI64 p1()"), "{text}");

        // Audit::is_clean is false when affected_traces is non-empty.
        let mut audit = super::Audit::default();
        assert!(audit.is_clean());
        audit.affected_traces.push(super::AffectedTrace {
            trace: val,
            corrupt: vec![val],
            missing: vec![],
            unknowable: false,
        });
        assert!(!audit.is_clean());
    }

    #[test]
    fn bundle_partial_store_cache_collision_and_export_size_bounds() {
        // Partial-store bundles produced by `export` (nodes referencing cairns
        // not present in the source store) must round-trip through `import`
        // while truncated bundles still leave the destination store untouched.
        let partial_src_dir = Scratch::new();
        let partial_src = partial_src_dir.store();
        let leaf = partial_src.put(&Stored::Value(Value::Int(7))).unwrap();
        let apply = partial_src
            .put(&Stored::Node(Node::Apply {
                function: Cairn::of_encoded(b"external-fn"),
                args: vec![leaf],
                result: Cairn::of_encoded(b"external-res"),
            }))
            .unwrap();
        let mut bundle = Vec::new();
        assert_eq!(partial_src.export(&mut bundle).unwrap(), 2);

        let trunc_dst_dir = Scratch::new();
        let trunc_dst = trunc_dst_dir.store();
        assert!(matches!(
            trunc_dst.import(&mut &bundle[..bundle.len() - 4]),
            Err(StoreError::Bundle("truncated bundle"))
        ));
        assert!(trunc_dst.audit().unwrap().is_clean(), "truncated bundle must commit nothing");
        assert!(!trunc_dst.has(leaf));

        let partial_dst_dir = Scratch::new();
        let partial_dst = partial_dst_dir.store();
        assert_eq!(partial_dst.import(&mut &bundle[..]).unwrap(), 2);
        assert!(partial_dst.has(leaf));
        assert!(partial_dst.has(apply));

        // Corrupting an already-cached destination object on disk must be caught
        // by `import` rather than hidden by the in-memory `cache`.
        let cached_dst_dir = Scratch::new();
        let cached_dst = cached_dst_dir.store();
        assert_eq!(cached_dst.put(&Stored::Value(Value::Int(7))).unwrap(), leaf);
        assert_eq!(cached_dst.get(leaf).unwrap(), Stored::Value(Value::Int(7)));
        std::fs::write(cached_dst.fanned("objects", leaf), b"tampered").unwrap();
        assert!(matches!(
            cached_dst.import(&mut &bundle[..]),
            Err(StoreError::Corrupt { asked, .. }) if asked == leaf
        ));

        // Oversized objects are rejected by `export` with a bundle error.
        let big_dir = Scratch::new();
        let big = big_dir.store();
        let huge = Cairn::of_encoded(b"huge");
        let huge_path = big.fanned("objects", huge);
        std::fs::create_dir_all(huge_path.parent().unwrap()).unwrap();
        let f = std::fs::File::create(&huge_path).unwrap();
        f.set_len(super::MAX_OBJECT_BYTES as u64 + 1).unwrap();
        let mut sink = Vec::new();
        assert!(matches!(big.export(&mut sink), Err(StoreError::Bundle(_))));
    }

    #[test]
    fn ledger_hardening_c8_h11_h25_m13_m15_and_l8() {
        // H25: try_encode rejects invalid states before encoding.
        assert!(matches!(
            Value::Shade { origin: 9, value: Cairn::of_encoded(b"x") }.try_encode(),
            Err(crate::DecodeError::NoSuchStratum(9))
        ));
        assert!(matches!(
            Value::Struct { name: String::new(), fields: vec![] }.try_encode(),
            Err(crate::DecodeError::EmptyStructName)
        ));
        assert!(matches!(
            Node::Deposit {
                value: Cairn::of_encoded(b"x"),
                span: Span { source: Cairn::of_encoded(b"x"), start: 5, end: 2 }
            }
            .try_encode(),
            Err(crate::DecodeError::BackwardsSpan { start: 5, end: 2 })
        ));

        // H11: objects > 16 MiB (e.g. 17 MiB) round-trip cleanly through export/import,
        // while objects > MAX_OBJECT_BYTES are refused by Store::put.
        let src_dir = Scratch::new();
        let src = src_dir.store();
        let big_val = Stored::Value(Value::Bytes(vec![0xAB; 17 * 1024 * 1024]));
        let big_c = src.put(&big_val).unwrap();
        let mut bundle = Vec::new();
        assert_eq!(src.export(&mut bundle).unwrap(), 1);
        let dst_dir = Scratch::new();
        let dst = dst_dir.store();
        assert_eq!(dst.import(&mut &bundle[..]).unwrap(), 1);
        assert!(dst.has(big_c));
        // C8: tmp/ staging file is removed after import.
        assert_eq!(std::fs::read_dir(dst.root.join("tmp")).unwrap().count(), 0);

        let too_big = Stored::Value(Value::Bytes(vec![0u8; super::MAX_OBJECT_BYTES]));
        assert!(matches!(
            src.put(&too_big),
            Err(StoreError::Invalid(crate::DecodeError::TooLarge { .. }))
        ));

        // M13: Package::resolve does not insert a stray `\n` before an empty entrypoint.
        let pkg_dir = Scratch::new();
        let pkg_store = pkg_dir.store();
        let dep = Package::put(&pkg_store, b"I64 a = 1;", &[]).unwrap();
        let root = Package::put(&pkg_store, b"", &[("dep", dep)]).unwrap();
        let resolved = Package::resolve(&pkg_store, root).unwrap();
        assert_eq!(resolved.unit_source, b"I64 a = 1;");

        // M15: Store::put deduplicates repeated references and repairs missing refs/.
        let ref_dir = Scratch::new();
        let ref_store = ref_dir.store();
        let v = ref_store.put(&Stored::Value(Value::Int(1))).unwrap();
        let trace_node = Stored::Node(Node::Trace {
            residue: v,
            holes: vec![],
            witnesses: vec![],
            deposits: vec![],
            source: v,
            depth: 0,
            unrecorded: false,
        });
        let tc = ref_store.put(&trace_node).unwrap();
        // `v` appears as both `residue` and `source` — should write only one 32-byte ref record.
        assert_eq!(std::fs::metadata(ref_store.fanned("refs", v)).unwrap().len(), 32);
        std::fs::remove_file(ref_store.fanned("refs", v)).unwrap();
        assert_eq!(ref_store.put(&trace_node).unwrap(), tc);
        assert_eq!(ref_store.referrers(v).unwrap(), vec![tc]);

        // L8: Store::has returns false for a directory; resolve ignores uppercase buckets.
        let fake_c = Cairn::of_encoded(b"dir-not-file");
        std::fs::create_dir_all(ref_store.fanned("objects", fake_c)).unwrap();
        assert!(!ref_store.has(fake_c));
        let upper_dir = ref_store.root.join("objects").join("AB");
        std::fs::create_dir_all(&upper_dir).unwrap();
        std::fs::write(upper_dir.join("0".repeat(62)), b"x").unwrap();
        assert!(matches!(ref_store.resolve("a"), Err(StoreError::NoMatch(_)) | Ok(_)));
        // Ensure the fake AB000... file is never returned for prefix "a".
        if let Ok(found) = ref_store.resolve("a") {
            assert_ne!(found.to_string(), format!("ab{}", "0".repeat(62)));
        }
    }

    #[test]
    fn round2_store_bounds_fifo_safety_max_depth_and_package_cap() {
        // H32: Store::put accepts MAX_DEPTH and rejects MAX_DEPTH + 1.
        let dir = Scratch::new();
        let store = dir.store();
        let mut at_limit = Value::Unit;
        for _ in 0..crate::MAX_DEPTH {
            at_limit = Value::Array(vec![at_limit]);
        }
        let c = store.put(&Stored::Value(at_limit.clone())).expect("MAX_DEPTH must succeed");
        assert_eq!(store.get(c).unwrap(), Stored::Value(at_limit.clone()));
        let over_limit = Value::Array(vec![at_limit]);
        assert!(matches!(
            store.put(&Stored::Value(over_limit)),
            Err(StoreError::Invalid(crate::DecodeError::TooDeep))
        ));

        // H16: Oversized sparse file in objects/ is rejected by get() and audit()
        // without allocating past MAX_OBJECT_BYTES.
        let huge = Cairn::of_encoded(b"sparse-oversized-object");
        let huge_path = store.fanned("objects", huge);
        std::fs::create_dir_all(huge_path.parent().unwrap()).unwrap();
        let f = std::fs::File::create(&huge_path).unwrap();
        f.set_len(super::MAX_OBJECT_BYTES as u64 + 1024).unwrap();
        assert!(matches!(
            store.get(huge),
            Err(StoreError::Corrupt { why: Some(crate::DecodeError::TooLarge { .. }), .. })
        ));
        let audit = store.audit().unwrap();
        assert!(audit.corrupt.contains(&huge));

        // H16: FIFO in objects/ or refs/ fails immediately without hanging.
        #[cfg(unix)]
        {
            let fifo_obj = Cairn::of_encoded(b"fifo-obj");
            let fifo_obj_path = store.fanned("objects", fifo_obj);
            std::fs::create_dir_all(fifo_obj_path.parent().unwrap()).unwrap();
            if std::process::Command::new("mkfifo")
                .arg(&fifo_obj_path)
                .status()
                .is_ok_and(|s| s.success())
            {
                assert!(matches!(store.get(fifo_obj), Err(StoreError::Corrupt { .. })));
            }
            let fifo_ref = Cairn::of_encoded(b"fifo-ref");
            let fifo_ref_path = store.fanned("refs", fifo_ref);
            std::fs::create_dir_all(fifo_ref_path.parent().unwrap()).unwrap();
            if std::process::Command::new("mkfifo")
                .arg(&fifo_ref_path)
                .status()
                .is_ok_and(|s| s.success())
            {
                assert!(matches!(store.referrers(fifo_ref), Err(StoreError::Corrupt { .. })));
            }
        }

        // H17: Package::resolve caps total unit_source at MAX_OBJECT_BYTES and
        // hashes unit_cairn identically to Value::Bytes(unit_source).cairn().
        let pdir = Scratch::new();
        let pstore = pdir.store();
        let chunk = vec![b'a'; 33 * 1024 * 1024];
        let p1 = Package::put(&pstore, &chunk, &[]).unwrap();
        let p2 = Package::put(&pstore, &chunk, &[("a", p1)]).unwrap();
        let res_small = Package::resolve(&pstore, p1).unwrap();
        assert_eq!(res_small.unit_cairn, Value::Bytes(res_small.unit_source.clone()).cairn());
        assert!(matches!(
            Package::resolve(&pstore, p2),
            Err(PackageError::Malformed(_, "resolved package source exceeds size bound"))
        ));
    }
}
