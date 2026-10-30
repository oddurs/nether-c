//! What `bury` remembers, outside the ledger.
//!
//! `spec/08-rites.md` §8.2.1. One file per burial, named by the pair it is
//! about, holding the trace that burial wrote and what it cost. It holds no
//! fact the store does not, so deleting the directory loses time and nothing
//! else.

use std::path::PathBuf;

use nether_ledger::{Cairn, Node, Store, Stored};

/// A burial this implementation already did.
pub struct Remembered {
    pub trace: Cairn,
    pub steps: u64,
    pub nodes: usize,
}

/// Where one burial's entry lives, or nothing if this build cannot name itself.
pub struct Memo(PathBuf);

impl Memo {
    /// §8.2.1: keyed on the implementation as well as the source, and the
    /// implementation is its own executable, which changes whenever what it
    /// would bury could.
    pub fn of(source: Cairn) -> Option<Self> {
        let exe = std::fs::read(std::env::current_exe().ok()?).ok()?;
        let mut key = Cairn::of_encoded(&exe).as_bytes().to_vec();
        key.extend_from_slice(source.as_bytes());
        Some(Self(crate::store_root().join("memo").join(Cairn::of_encoded(&key).to_string())))
    }

    /// The entry, if it is one §8.2.1 lets the rite honour: the trace is there,
    /// buried from this source, answered by nobody, and within this budget.
    pub fn recall(&self, store: &Store, source: Cairn, fuel: u64) -> Option<Remembered> {
        let text = std::fs::read_to_string(&self.0).ok()?;
        let mut words = text.split_whitespace();
        let trace: Cairn = words.next()?.parse().ok()?;
        let steps: u64 = words.next()?.parse().ok()?;
        let nodes: usize = words.next()?.parse().ok()?;
        let Ok(Stored::Node(Node::Trace { source: s, witnesses, .. })) = store.get(trace) else {
            return None;
        };
        (s == source && witnesses.is_empty() && steps <= fuel).then_some(Remembered {
            trace,
            steps,
            nodes,
        })
    }

    /// Write the entry, beside it and then over it, so a reader sees a whole
    /// one or none. A memo that will not take one costs the next burial its
    /// time, which is all a memo is for, so failing here is not an error, and
    /// a temporary left behind is never read as an entry.
    pub fn record(&self, r: &Remembered) {
        let Some(dir) = self.0.parent() else { return };
        let tmp = self.0.with_extension(format!("{}.tmp", std::process::id()));
        let _ = std::fs::create_dir_all(dir)
            .and_then(|()| std::fs::write(&tmp, format!("{} {} {}\n", r.trace, r.steps, r.nodes)))
            .and_then(|()| std::fs::rename(&tmp, &self.0));
    }
}
