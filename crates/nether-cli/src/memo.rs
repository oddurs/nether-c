//! What `bury` remembers, outside the ledger.
//!
//! `spec/08-rites.md` §8.2.1. One file per burial, named by the pair it is
//! about, holding the trace that burial wrote and what it cost. It holds no
//! fact the store does not, so deleting the directory loses time and nothing
//! else.

use std::collections::HashSet;
use std::io::Write as _;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{SystemTime, UNIX_EPOCH};

use nether_ledger::{AnswerOf, Cairn, Node, Store, Stored, Value};

static MEMO_NONCE: AtomicU64 = AtomicU64::new(0);

/// A burial this implementation already did.
pub struct Remembered {
    pub trace: Cairn,
    pub steps: u64,
    pub nodes: usize,
    pub extra: Vec<Cairn>,
}

/// Verify that every transitive shaded value inside `v` is present and valid in `store`.
fn value_complete(store: &Store, v: &Value, seen: &mut HashSet<Cairn>) -> bool {
    let mut stack: Vec<Value> = vec![v.clone()];
    while let Some(cur) = stack.pop() {
        match cur {
            Value::Shade { value, .. } => {
                if seen.insert(value) {
                    let Ok(Stored::Value(inner)) = store.get(value) else {
                        return false;
                    };
                    stack.push(inner);
                }
            }
            Value::Struct { fields, .. } => stack.extend(fields),
            Value::Array(items) => stack.extend(items),
            Value::Answer(ans) => {
                if let AnswerOf::Given(inner) = *ans {
                    stack.push(inner);
                }
            }
            _ => {}
        }
    }
    true
}

fn has_bytes(store: &Store, cairn: Cairn) -> bool {
    matches!(store.get(cairn), Ok(Stored::Value(Value::Bytes(_))))
}

fn value_ok(store: &Store, cairn: Cairn, seen: &mut HashSet<Cairn>) -> bool {
    matches!(store.get(cairn), Ok(Stored::Value(v)) if value_complete(store, &v, seen))
}

/// Where one burial's entry lives, or nothing if this build cannot name itself.
pub struct Memo {
    path: PathBuf,
    key: Cairn,
}

impl Memo {
    /// §8.2.1: keyed on the implementation as well as the source, and the
    /// implementation is its own executable, which changes whenever what it
    /// would bury could.
    pub fn of(source: Cairn) -> Option<Self> {
        let exe = std::fs::read(std::env::current_exe().ok()?).ok()?;
        let mut raw = Cairn::of_encoded(&exe).as_bytes().to_vec();
        raw.extend_from_slice(source.as_bytes());
        let key = Cairn::of_encoded(&raw);
        Some(Self { path: crate::store_root().join("memo").join(key.to_string()), key })
    }

    /// Integrity tag binding memo metadata (`trace`, `steps`, `nodes`, `extra`)
    /// to this binary and source key so tampered or torn memo entries miss.
    fn tag(&self, body: &str) -> Cairn {
        let mut buf = Vec::with_capacity(32 + body.len());
        buf.extend_from_slice(self.key.as_bytes());
        buf.extend_from_slice(body.as_bytes());
        Cairn::of_encoded(&buf)
    }

    fn parse_entry(&self, text: &str) -> Option<Remembered> {
        let mut lines = text.lines();
        let header = lines.next()?;
        let mut words = header.split_whitespace();
        let trace: Cairn = words.next()?.parse().ok()?;
        let steps: u64 = words.next()?.parse().ok()?;
        let nodes: usize = words.next()?.parse().ok()?;
        let saved_tag: Cairn = words.next()?.parse().ok()?;
        if words.next().is_some() {
            return None;
        }
        let mut extra = Vec::new();
        let mut extra_raw = String::new();
        if let Some(extra_line) = lines.next() {
            for tok in extra_line.split_whitespace() {
                extra.push(tok.parse::<Cairn>().ok()?);
            }
            extra_raw.push_str(extra_line);
            extra_raw.push('\n');
        }
        if lines.next().is_some() {
            return None;
        }
        let body = format!("{trace} {steps} {nodes}\n{extra_raw}");
        if self.tag(&body) != saved_tag {
            return None;
        }
        Some(Remembered { trace, steps, nodes, extra })
    }

    /// The entry, if it is one §8.2.1 lets the rite honour: the trace is there,
    /// buried from this source, answered by nobody, and within this budget.
    pub fn recall(&self, store: &Store, source: Cairn, fuel: u64) -> Option<Remembered> {
        let text = std::fs::read_to_string(&self.path).ok()?;
        let Remembered { trace, steps, nodes, extra } = self.parse_entry(&text)?;
        let Ok(Stored::Node(Node::Trace {
            residue,
            holes,
            deposits,
            source: s,
            witnesses,
            depth,
            unrecorded,
        })) = store.get(trace)
        else {
            return None;
        };
        if s != source
            || unrecorded
            || !witnesses.is_empty()
            || steps > fuel
            || nodes < 2
            || !has_bytes(store, source)
        {
            return None;
        }
        let Ok(Stored::Value(Value::Bytes(res_bytes))) = store.get(residue) else {
            return None;
        };
        let ast = nether_syntax::parse(&res_bytes).ok()?;
        let unit = nether_syntax::lower(&ast).ok()?;
        if !nether_core::check(&unit).is_empty() {
            return None;
        }
        let res_depth = unit
            .demands
            .iter()
            .map(|d| crate::closing::stratum(d.value.depth))
            .max()
            .unwrap_or(nether_ledger::Stratum::ZERO);
        if (!holes.is_empty() || !deposits.is_empty() || !unit.demands.is_empty()) && steps == 0 {
            return None;
        }
        let mut seen = HashSet::new();
        let mut max_hole_stratum = nether_ledger::Stratum::ZERO;
        for h in &holes {
            let Ok(Stored::Node(Node::Hole { call, stratum, span })) = store.get(*h) else {
                return None;
            };
            if !crate::closing::valid_stratum(&call, stratum)
                || !has_bytes(store, span.source())
                || !call.args().iter().all(|&a| value_ok(store, a, &mut seen))
            {
                return None;
            }
            max_hole_stratum = max_hole_stratum.max(stratum);
        }
        if depth < res_depth.max(max_hole_stratum) {
            return None;
        }
        for d in &deposits {
            let Ok(Stored::Node(Node::Deposit { value, span })) = store.get(*d) else {
                return None;
            };
            if !has_bytes(store, span.source()) || !value_ok(store, value, &mut seen) {
                return None;
            }
        }
        if !extra.iter().all(|&c| value_ok(store, c, &mut seen)) {
            return None;
        }
        Some(Remembered { trace, steps, nodes, extra })
    }

    /// Write the entry, beside it and then over it, so a reader sees a whole
    /// one or none. A memo that will not take one costs the next burial its
    /// time, which is all a memo is for, so failing here is not an error, and
    /// a temporary left behind is never read as an entry.
    pub fn record(&self, r: &Remembered) {
        let Some(dir) = self.path.parent() else { return };
        if std::fs::create_dir_all(dir).is_err() {
            return;
        }
        let mut extra_raw = String::new();
        if !r.extra.is_empty() {
            let mut seen = HashSet::new();
            let listed: Vec<String> = r
                .extra
                .iter()
                .copied()
                .filter(|c| seen.insert(*c))
                .map(|c| c.to_string())
                .collect();
            extra_raw.push_str(&listed.join(" "));
            extra_raw.push('\n');
        }
        let body = format!("{} {} {}\n{extra_raw}", r.trace, r.steps, r.nodes);
        let tag = self.tag(&body);
        let payload = format!("{} {} {} {tag}\n{extra_raw}", r.trace, r.steps, r.nodes);
        let pid = std::process::id();
        let (tmp, mut file) = loop {
            let n = MEMO_NONCE.fetch_add(1, Ordering::Relaxed);
            let nanos = SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_nanos());
            let candidate = self.path.with_extension(format!("{pid}.{nanos}.{n}.tmp"));
            match std::fs::OpenOptions::new().write(true).create_new(true).open(&candidate) {
                Ok(f) => break (candidate, f),
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(_) => return,
            }
        };
        let wrote = file.write_all(payload.as_bytes()).and_then(|()| {
            drop(file);
            std::fs::rename(&tmp, &self.path)
        });
        if wrote.is_err() {
            let _ = std::fs::remove_file(&tmp);
        }
    }
}
