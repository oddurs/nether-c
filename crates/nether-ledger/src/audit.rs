//! Read-only whole-store audit (`spec/07-ledger.md` §7.5.2).

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::io;
use std::path::PathBuf;

use crate::cairn::Cairn;
use crate::codec::scan_stored;
use crate::store::{Store, StoreError, for_each_ref, read_bounded, verify_with};

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

struct VerifiedEntry {
    is_node: bool,
    is_trace: bool,
    refs: Vec<Cairn>,
    nodes: HashSet<Cairn>,
}

fn audit_trace(
    trace: Cairn,
    entry: &VerifiedEntry,
    verified: &BTreeMap<Cairn, VerifiedEntry>,
    corrupt: &BTreeSet<Cairn>,
) -> Option<AffectedTrace> {
    let (mut bad_c, mut bad_m, mut unknowable) = (BTreeSet::new(), BTreeSet::new(), false);
    let mut seen = BTreeSet::new();
    let mut todo: Vec<(Cairn, bool)> =
        entry.refs.iter().map(|&r| (r, entry.nodes.contains(&r))).collect();
    while let Some((r, is_node)) = todo.pop() {
        if !seen.insert((r, is_node)) {
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
        None
    } else {
        Some(AffectedTrace {
            trace,
            corrupt: bad_c.into_iter().collect(),
            missing: bad_m.into_iter().collect(),
            unknowable,
        })
    }
}

impl Store {
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
        let (mut malformed, mut verified, mut corrupt) =
            (BTreeSet::new(), BTreeMap::<Cairn, VerifiedEntry>::new(), BTreeSet::new());
        for (cairn, path) in self.scan_area("objects", &mut malformed)? {
            match read_bounded(&path, Some(cairn)) {
                Ok(bytes) => {
                    if let Ok(scanned) = verify_with(cairn, &bytes, scan_stored) {
                        verified.insert(
                            cairn,
                            VerifiedEntry {
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
            match audit_trace(trace, entry, &verified, &corrupt) {
                Some(affected) => affected_traces.push(affected),
                None => healthy_traces.push(trace),
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
}
