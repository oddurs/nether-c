//! The store.
//!
//! A set of `(cairn, bytes)` pairs on disk, where the bytes are the canonical
//! encoding of a value or a node. Writing is idempotent because the name is
//! the content: two burials that compute the same value write the same bytes
//! to the same place, and need no coordination beyond each write being atomic.
//!
//! There is no garbage collection and there is not going to be one. See
//! `spec/07-ledger.md` §7.6, which also admits that this is a real cost.

use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::fs;
use std::io::{self, Write as _};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use crate::cairn::Cairn;
use crate::codec::{DecodeError, Stored, decode_stored};
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

/// How many encoded bytes `Seen` holds before it starts again. Starting again
/// is the whole eviction policy: a burial reads its working set, and a bound
/// that is simple to state is one that is simple to trust.
const SEEN_BYTES: usize = 64 << 20;
const BUNDLE_MAGIC: &[u8] = b"netherc/cairn/v4\x00bundle\x00";
const MAX_BUNDLE_OBJECT: usize = 16 << 20;

fn verify_bytes(asked: Cairn, bytes: &[u8]) -> Result<Stored, StoreError> {
    let stored = decode_stored(bytes).map_err(|why| StoreError::Corrupt {
        asked,
        found: None,
        why: Some(why),
    })?;
    let actual = Cairn::of_encoded(bytes);
    if actual != asked {
        return Err(StoreError::Corrupt { asked, found: Some(actual), why: None });
    }
    Ok(stored)
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

    fn fanned(&self, area: &str, cairn: Cairn) -> PathBuf {
        let hex = cairn.to_string();
        self.root.join(area).join(&hex[..2]).join(&hex[2..])
    }

    /// Whether this store holds that name.
    #[must_use]
    pub fn has(&self, cairn: Cairn) -> bool {
        self.fanned("objects", cairn).exists()
    }

    /// Writes a value or a node, and returns its name.
    ///
    /// Idempotent: putting the same thing twice is one object and one set of
    /// reverse edges.
    ///
    /// # Errors
    ///
    /// [`StoreError::Invalid`] if the bytes would not decode — the store will
    /// not write something it could never read back — or [`StoreError::Io`] if
    /// they cannot be written.
    pub fn put(&self, stored: &Stored) -> Result<Cairn, StoreError> {
        let bytes = stored.encode();

        // The round trip, before anything reaches disk. It costs a decode per
        // put, which is the same work `get` does anyway, and it is the only
        // thing standing between an upstream bug and an object that is
        // permanently unreadable and reported forever as disk corruption.
        decode_stored(&bytes).map_err(StoreError::Invalid)?;

        let cairn = Cairn::of_encoded(&bytes);
        if self.has(cairn) {
            return Ok(cairn);
        }

        // Reverse edges first, object second.
        //
        // Crashing between them leaves an edge pointing at an object that is
        // not there, which `referrers` filters and a later `put` repairs. The
        // other order leaves an object whose edges were never recorded, and
        // nothing afterwards can tell that provenance is now incomplete. A
        // visible inconsistency beats a silent one.
        if let Stored::Node(node) = stored {
            for target in node.references() {
                self.add_ref(target, cairn)?;
            }
        }

        let path = self.fanned("objects", cairn);
        self.write_atomically(&path, &bytes, &cairn.to_string())?;
        Ok(cairn)
    }

    /// A path nothing else will choose.
    ///
    /// The process id alone is not enough: two threads in one process writing
    /// the same object picked the same path, both wrote it, the first renamed
    /// it away, and the second's rename failed with ENOENT. The counter makes
    /// it per write rather than per process.
    fn temp_path(&self, hint: &str) -> PathBuf {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        self.root.join("tmp").join(format!("{hint}.{}.{n}", std::process::id()))
    }

    fn write_atomically(&self, path: &Path, bytes: &[u8], hint: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // `open` creates this, but a handle outlives the directory if an
        // operator or a tmp reaper clears it, and every write after that
        // failed with ENOENT.
        let tmp_dir = self.root.join("tmp");
        fs::create_dir_all(&tmp_dir)?;

        let temp = self.temp_path(hint);
        if let Err(e) = fs::write(&temp, bytes) {
            let _ = fs::remove_file(&temp);
            return Err(e);
        }
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
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(StoreError::Absent(cairn)),
            Err(e) => return Err(StoreError::Io(e)),
        };
        let stored = verify_bytes(cairn, &bytes)?;
        let mut seen = self.seen();
        if seen.bytes + bytes.len() > SEEN_BYTES {
            *seen = Seen { reads: seen.reads, ..Seen::default() };
        }
        seen.bytes += bytes.len();
        seen.held.insert(cairn, stored.clone());
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
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(StoreError::Io(e)),
        };
        // §7.5.1. A record half written is one that would otherwise be read as
        // a different cairn, and every record after it would be misaligned.
        if bytes.len() % 32 != 0 {
            return Err(StoreError::Ragged { of: cairn, bytes: bytes.len() });
        }
        let mut seen = std::collections::HashSet::new();
        Ok(bytes
            .chunks_exact(32)
            .filter_map(|c| <[u8; 32]>::try_from(c).ok())
            .map(Cairn::from_bytes)
            // Appending admits duplicates; collapse them here.
            .filter(|c| seen.insert(*c))
            // The index is a claim, and anybody who can write the store can
            // write one. An edge counts only if its node is there, checks, and
            // names this cairn: that covers an edge that outlived its object in
            // a crash and an edge a hostile writer added. 0217.
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

        let objects = self.root.join("objects");
        let mut found = Vec::new();
        let buckets: Vec<PathBuf> = if prefix.len() >= 2 {
            vec![objects.join(&prefix[..2])]
        } else {
            fs::read_dir(&objects)?
                .filter_map(Result::ok)
                .map(|e| e.path())
                .filter(|p| {
                    p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with(prefix))
                })
                .collect()
        };

        for bucket in buckets {
            let Some(head) = bucket.file_name().and_then(|n| n.to_str()).map(str::to_owned) else {
                continue;
            };
            let Ok(entries) = fs::read_dir(&bucket) else { continue };
            for entry in entries.filter_map(Result::ok) {
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

    /// Audits the store on disk without modifying it. `spec/07-ledger.md` §7.5.2.
    ///
    /// # Errors
    ///
    /// [`io::Error`] if the store directories or files cannot be read.
    pub fn audit(&self) -> io::Result<Audit> {
        let (mut malformed, mut verified, mut corrupt) =
            (BTreeSet::new(), BTreeMap::new(), BTreeSet::new());
        for (cairn, path) in self.scan_area("objects", &mut malformed)? {
            let bytes = fs::read(path)?;
            if let Some(Ok(s)) = (Cairn::of_encoded(&bytes) == cairn).then(|| decode_stored(&bytes))
            {
                verified.insert(cairn, s);
            } else {
                corrupt.insert(cairn);
            }
        }
        let mut missing = BTreeSet::new();
        let mut expected: BTreeMap<Cairn, BTreeSet<Cairn>> = BTreeMap::new();
        for (&referrer, stored) in &verified {
            if let Stored::Node(node) = stored {
                for target in node.references() {
                    expected.entry(target).or_default().insert(referrer);
                    if !verified.contains_key(&target) && !corrupt.contains(&target) {
                        missing.insert((referrer, target));
                    }
                }
            }
        }
        let (mut healthy_traces, mut affected_traces) = (Vec::new(), Vec::new());
        for (&trace, stored) in &verified {
            let Stored::Node(node @ Node::Trace { .. }) = stored else { continue };
            let (mut bad_c, mut bad_m, mut unknowable) = (BTreeSet::new(), BTreeSet::new(), false);
            let mut seen_r = BTreeSet::new();
            let nodes = node.nodes();
            let mut todo: Vec<(Cairn, bool)> =
                node.references().into_iter().map(|r| (r, nodes.contains(&r))).collect();
            while let Some((r, is_node)) = todo.pop() {
                if !seen_r.insert((r, is_node)) {
                    continue;
                }
                match verified.get(&r) {
                    Some(Stored::Node(child)) => {
                        let cn = child.nodes();
                        todo.extend(
                            child.references().into_iter().map(|cr| (cr, cn.contains(&cr))),
                        );
                    }
                    Some(Stored::Value(_)) if !is_node => {}
                    Some(Stored::Value(_)) => {
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
        let mut index = BTreeSet::new();
        let mut recorded: BTreeMap<Cairn, BTreeSet<Cairn>> = BTreeMap::new();
        for (cairn, path) in self.scan_area("refs", &mut malformed)? {
            let bytes = fs::read(path)?;
            if bytes.len() % 32 != 0 {
                index.insert(IndexError::Ragged { of: cairn, bytes: bytes.len() });
            } else {
                let set = bytes
                    .chunks_exact(32)
                    .filter_map(|c| <[u8; 32]>::try_from(c).ok())
                    .map(Cairn::from_bytes)
                    .collect();
                recorded.insert(cairn, set);
            }
        }
        for (target, referrers) in expected {
            for referrer in referrers {
                if !recorded.get(&target).is_some_and(|s| s.contains(&referrer)) {
                    index.insert(IndexError::Missing { target, referrer });
                }
            }
        }
        Ok(Audit {
            corrupt: corrupt.into_iter().collect(),
            missing: missing.into_iter().collect(),
            malformed: malformed.into_iter().collect(),
            index: index.into_iter().collect(),
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
        let mut malformed = BTreeSet::new();
        let files = self.scan_area("objects", &mut malformed)?;
        if !malformed.is_empty() {
            return Err(StoreError::Bundle("malformed path in objects/"));
        }
        let mut pending = BTreeMap::new();
        for (cairn, path) in files {
            let bytes = fs::read(path)?;
            let stored = verify_bytes(cairn, &bytes)?;
            pending.insert(cairn, (stored, bytes));
        }
        let mut entries = Vec::with_capacity(pending.len());
        while !pending.is_empty() {
            let ready: Vec<Cairn> = pending
                .iter()
                .filter(|(_, (s, _))| {
                    !matches!(s, Stored::Node(n) if n.references().iter().any(|r| pending.contains_key(r)))
                })
                .map(|(&c, _)| c)
                .collect();
            let take = if ready.is_empty() {
                pending.keys().next().copied().into_iter().collect()
            } else {
                ready
            };
            for c in take {
                if let Some((_, bytes)) = pending.remove(&c) {
                    entries.push((c, bytes));
                }
            }
        }
        let mut hasher = blake3::Hasher::new();
        let mut send = |slice: &[u8]| -> io::Result<()> {
            hasher.update(slice);
            out.write_all(slice)
        };
        send(BUNDLE_MAGIC)?;
        send(&(entries.len() as u64).to_be_bytes())?;
        for (cairn, bytes) in &entries {
            send(cairn.as_bytes())?;
            send(&(bytes.len() as u64).to_be_bytes())?;
            send(bytes)?;
        }
        out.write_all(hasher.finalize().as_bytes())?;
        Ok(entries.len())
    }

    /// Imports a portable bundle (`spec/07-ledger.md` §7.5.4), verifying each
    /// object before committing it and rebuilding `refs/`, returning the count.
    ///
    /// # Errors
    ///
    /// [`StoreError::Bundle`] if framing, bounds, ordering or trailer fail,
    /// [`StoreError::Corrupt`] if an entry or existing destination object fails
    /// verification, or [`StoreError::Io`] on I/O failure.
    pub fn import(&self, src: &mut impl io::Read) -> Result<usize, StoreError> {
        let mut hasher = blake3::Hasher::new();
        let mut take = |buf: &mut [u8]| -> Result<(), StoreError> {
            src.read_exact(buf).map_err(|e| {
                if e.kind() == io::ErrorKind::UnexpectedEof {
                    StoreError::Bundle("truncated bundle")
                } else {
                    StoreError::Io(e)
                }
            })?;
            hasher.update(buf);
            Ok(())
        };
        let mut hdr = [0u8; 24];
        take(&mut hdr)?;
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
        take(&mut u8s)?;
        let count = usize::try_from(u64::from_be_bytes(u8s))
            .map_err(|_| StoreError::Bundle("bundle count overflows usize"))?;
        let mut seen = BTreeSet::new();
        for _ in 0..count {
            let mut raw = [0u8; 32];
            take(&mut raw)?;
            let cairn = Cairn::from_bytes(raw);
            if !seen.insert(cairn) {
                return Err(StoreError::Bundle("duplicate cairn in bundle"));
            }
            take(&mut u8s)?;
            let len = usize::try_from(u64::from_be_bytes(u8s))
                .ok()
                .filter(|&n| n <= MAX_BUNDLE_OBJECT)
                .ok_or(StoreError::Bundle("bundle object exceeds size bound"))?;
            let mut bytes = vec![0u8; len];
            take(&mut bytes)?;
            let stored = verify_bytes(cairn, &bytes)?;
            if self.has(cairn) {
                self.get(cairn)?;
                if let Stored::Node(node) = &stored {
                    for target in node.references() {
                        if !self.referrers(target)?.contains(&cairn) {
                            self.add_ref(target, cairn)?;
                        }
                    }
                }
            } else {
                self.put(&stored)?;
            }
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
        if src.read(&mut [0u8; 1])? != 0 {
            return Err(StoreError::Bundle("trailing bytes after bundle"));
        }
        Ok(count)
    }
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
        let unit_cairn = Value::Bytes(unit_source.clone()).cairn();
        Ok(ResolvedPackage {
            root,
            entrypoint,
            aliases: root_deps.into_iter().collect(),
            closure,
            unit_source,
            unit_cairn,
        })
    }

    fn walk(
        store: &Store,
        pkg: Cairn,
        visiting: &mut BTreeSet<Cairn>,
        visited: &mut BTreeSet<Cairn>,
        closure: &mut Vec<Cairn>,
        unit_source: &mut Vec<u8>,
    ) -> Result<(Cairn, Vec<(String, Cairn)>), PackageError> {
        if !visiting.insert(pkg) {
            return Err(PackageError::Malformed(pkg, "dependency cycle"));
        }
        let Stored::Value(Value::Struct { name, fields }) = store.get(pkg)? else {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        };
        let [Value::Str(domain), Value::Cairn(ep), Value::Array(raw_deps)] = &fields[..] else {
            return Err(PackageError::Malformed(pkg, "wrong Package fields"));
        };
        if name != "Package" {
            return Err(PackageError::Malformed(pkg, "not a Package struct"));
        }
        if domain != PACKAGE_DOMAIN {
            return Err(PackageError::Incompatible(pkg, domain.clone()));
        }
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
        let Stored::Value(Value::Bytes(bytes)) = store.get(*ep)? else {
            return Err(PackageError::Malformed(*ep, "entrypoint is not Bytes"));
        };
        if core::str::from_utf8(&bytes).is_err() {
            return Err(PackageError::Malformed(*ep, "entrypoint is not UTF-8"));
        }
        for (_, dep) in &deps {
            if !visited.contains(dep) {
                Self::walk(store, *dep, visiting, visited, closure, unit_source)?;
            }
        }
        visiting.remove(&pkg);
        if visited.insert(pkg) {
            closure.push(pkg);
            unit_source.extend_from_slice(&bytes);
        }
        Ok((*ep, deps))
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
}
