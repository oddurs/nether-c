//! The store.
//!
//! A set of `(cairn, bytes)` pairs on disk, where the bytes are the canonical
//! encoding of a value or a node. Writing is idempotent because the name is
//! the content: two burials that compute the same value write the same bytes
//! to the same place, and need no coordination beyond each write being atomic.
//!
//! There is no garbage collection and there is not going to be one. See
//! `spec/07-ledger.md` §7.6, which also admits that this is a real cost.

use std::fs;
use std::io;
use std::path::{Path, PathBuf};

use crate::cairn::Cairn;
use crate::codec::{DecodeError, Stored, decode_stored};

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
        Ok(Self { root })
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
    /// If the bytes cannot be written.
    pub fn put(&self, stored: &Stored) -> io::Result<Cairn> {
        let bytes = stored.encode();
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

    fn write_atomically(&self, path: &Path, bytes: &[u8], hint: &str) -> io::Result<()> {
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        // Unique per writer, so two processes storing the same object do not
        // share a partial file.
        let temp = self.root.join("tmp").join(format!("{hint}.{}", std::process::id()));
        fs::write(&temp, bytes)?;
        fs::rename(&temp, path)
    }

    fn add_ref(&self, target: Cairn, referrer: Cairn) -> io::Result<()> {
        let path = self.fanned("refs", target);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let mut existing = fs::read(&path).unwrap_or_default();
        if existing.chunks_exact(32).any(|c| c == referrer.as_bytes()) {
            return Ok(());
        }
        existing.extend_from_slice(referrer.as_bytes());
        let hint = format!("ref.{target}");
        self.write_atomically(&path, &existing, &hint)
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
        let path = self.fanned("objects", cairn);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Err(StoreError::Absent(cairn)),
            Err(e) => return Err(StoreError::Io(e)),
        };
        let actual = Cairn::of_encoded(&bytes);
        if actual != cairn {
            return Err(StoreError::Corrupt { asked: cairn, found: Some(actual), why: None });
        }
        decode_stored(&bytes).map_err(|why| StoreError::Corrupt {
            asked: cairn,
            found: None,
            why: Some(why),
        })
    }

    /// Every node that names this cairn.
    ///
    /// This is the reverse index `spec/07-ledger.md` §7.4 requires: the cost is
    /// proportional to the answer rather than to the size of the store, which
    /// is what makes walking provenance usable.
    ///
    /// # Errors
    ///
    /// If the index cannot be read.
    pub fn referrers(&self, cairn: Cairn) -> io::Result<Vec<Cairn>> {
        let path = self.fanned("refs", cairn);
        let bytes = match fs::read(&path) {
            Ok(b) => b,
            Err(e) if e.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(e) => return Err(e),
        };
        Ok(bytes
            .chunks_exact(32)
            .filter_map(|c| <[u8; 32]>::try_from(c).ok())
            .map(Cairn::from_bytes)
            // An edge written just before a crash can outlive its object.
            .filter(|c| self.has(*c))
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
}

#[cfg(test)]
mod tests {
    use super::{Store, StoreError};
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
            depends: vec![],
        };
        let cairn = store.put(&Stored::Node(hole)).unwrap();

        assert_eq!(store.referrers(arg).unwrap(), vec![cairn]);
        assert_eq!(store.referrers(source).unwrap(), vec![cairn]);
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
}
