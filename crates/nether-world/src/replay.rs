//! Answering from the ledger, and from nowhere else.

use std::collections::HashMap;

use nether_ledger::{Cairn, Call, Node, Span, Store, StoreError, Stored, Value};

/// Why worker deliveries could not be merged into one trace (`spec/06-evaluation.md` §6.6).
#[derive(Debug)]
pub enum MergeError {
    /// A hole, witness or answer object was absent or corrupt in the store.
    Store(StoreError),
    /// A delivery named `(slot, holes.len())` outside `0..holes.len()`.
    OutOfRange(usize, usize),
    /// A witness `(slot, hole, witness)` did not match its slot or left an earlier call open.
    Mismatched(usize, Cairn, Cairn),
    /// Two deliveries `(slot, hole, first, second)` disagreed on the same question.
    Conflict(usize, Cairn, Cairn, Cairn),
    /// A witness `(witness)` the trace already held was not a witness with a
    /// value. It answers no slot, so it is not named by one.
    BaseWitness(Cairn),
}

impl From<StoreError> for MergeError {
    fn from(e: StoreError) -> Self {
        Self::Store(e)
    }
}

/// What a trace already recorded, and nothing else.
///
/// §6.7: "The implementation MUST hold no capabilities at all during replay: it
/// does not prefer the ledger over the world, it cannot reach the world."
///
/// That is a claim about what this *is*, not about what it does. A `Replay`
/// holds a map from a question to what was said and nothing more — no
/// [`crate::World`], no [`crate::Provider`], no path, no socket — and there is
/// no method that takes one, so there is nothing here to reach the world with
/// and no way to hand it something that could.
///
/// Granting it anything does not compile:
///
/// ```compile_fail
/// # use nether_world::{Disk, Replay};
/// let replay = Replay::of(&[]);
/// let cheating = replay.granting(Box::new(Disk::reading(".")));
/// ```
#[derive(Debug, Clone, Default)]
pub struct Replay {
    said: HashMap<Call, Vec<Value>>,
}

impl Replay {
    /// What those witnesses recorded.
    ///
    /// Takes the answers rather than the store they came from, because a
    /// `Replay` that kept a store could be asked to read one more thing.
    #[must_use]
    pub fn of(answers: &[(Call, Value)]) -> Self {
        let mut said: HashMap<Call, Vec<Value>> = HashMap::new();
        for (call, value) in answers {
            said.entry(call.clone()).or_default().push(value.clone());
        }
        Self { said }
    }

    /// Every witness a trace names, read out of the ledger.
    ///
    /// The store is used here and not kept: what comes back is a `Replay` that
    /// has already been told everything it will ever know.
    #[must_use]
    pub fn of_trace(store: &Store, witnesses: &[Cairn]) -> Self {
        // In the order the trace names them, because §6.3 merges a hole only
        // at a read stratum: one call can have been asked more than once, and
        // then the answers to it are told apart by nothing but their order.
        let mut said: HashMap<Call, Vec<Value>> = HashMap::new();
        for w in witnesses {
            if let Ok(Stored::Node(Node::Witness { call, answer, .. })) = store.get(*w)
                && let Ok(Stored::Value(v)) = store.get(answer)
            {
                said.entry(call).or_default().push(v);
            }
        }
        Self { said }
    }

    /// What was recorded for that question, if anything was.
    ///
    /// There is no fallback. §6.7 requires replay to fail rather than reach the
    /// world for an answer it does not have, and the way that is guaranteed
    /// here is that there is nowhere else to look.
    #[must_use]
    pub fn answer(&self, asked: &Call) -> Option<&Value> {
        self.said.get(asked)?.first()
    }

    /// Everything it was told, for handing to a burial.
    ///
    /// Each call once per answer recorded for it, in the order they were
    /// recorded. A burial takes them the same way (§6.3).
    pub fn all(&self) -> impl Iterator<Item = (&Call, &Value)> {
        self.said.iter().flat_map(|(call, values)| values.iter().map(move |v| (call, v)))
    }

    /// How many answers it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.said.values().map(Vec::len).sum()
    }

    /// Whether it holds none, in which case it can answer nothing at all.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.said.is_empty()
    }

    /// Merge verified worker deliveries `(slot, witness_cairn)` for `holes`
    /// on top of `base_witnesses` in ascending slot order (`spec/06-evaluation.md` §6.6),
    /// holding no world capabilities.
    ///
    /// # Errors
    ///
    /// Returns [`MergeError`] if any referenced object is absent or corrupt in
    /// `store`, if a slot is out of range, if a witness does not match its hole
    /// slot's `(call, stratum, span)`, or if two deliveries disagree on the
    /// witness for a slot or on the answer to a read-stratum call.
    pub fn merge(
        store: &Store,
        base_witnesses: &[Cairn],
        holes: &[Cairn],
        deliveries: &[&[(usize, Cairn)]],
    ) -> Result<(Vec<Cairn>, Self), MergeError> {
        for &w in base_witnesses {
            let Stored::Node(Node::Witness { answer, .. }) = store.get(w)? else {
                return Err(MergeError::BaseWitness(w));
            };
            if !matches!(store.get(answer)?, Stored::Value(_)) {
                return Err(MergeError::BaseWitness(w));
            }
        }
        let mut open: Vec<(Call, u8, Span)> = Vec::with_capacity(holes.len());
        for (slot, &h) in holes.iter().enumerate() {
            let Stored::Node(Node::Hole { call, stratum, span }) = store.get(h)? else {
                return Err(MergeError::Mismatched(slot, h, h));
            };
            open.push((call, stratum, span));
        }
        let mut chosen: Vec<Option<(Cairn, Cairn, Value)>> = vec![None; holes.len()];
        // For each read-stratum call, the chosen slot lowest in the trace and
        // its witness and answer: what a later delivery has to agree with.
        let mut first: HashMap<&Call, (usize, Cairn, Cairn)> = HashMap::new();
        for &(slot, witness) in deliveries.iter().copied().flatten() {
            let Some((h_call, h_stratum, h_span)) = open.get(slot) else {
                return Err(MergeError::OutOfRange(slot, holes.len()));
            };
            let hole = holes[slot];
            let Stored::Node(Node::Witness { stratum, call, answer, span }) = store.get(witness)?
            else {
                return Err(MergeError::Mismatched(slot, hole, witness));
            };
            let Stored::Value(val) = store.get(answer)? else {
                return Err(MergeError::Mismatched(slot, hole, answer));
            };
            if (stratum, &call, span) != (*h_stratum, h_call, *h_span) {
                return Err(MergeError::Mismatched(slot, hole, witness));
            }
            if let Some((first, _, _)) = chosen[slot] {
                if first == witness {
                    continue;
                }
                return Err(MergeError::Conflict(slot, hole, first, witness));
            }
            if matches!(h_stratum, 1 | 2 | 3 | 5) {
                match first.get(h_call) {
                    Some(&(_, said, ans)) if ans != answer => {
                        return Err(MergeError::Conflict(slot, hole, said, witness));
                    }
                    Some(&(lowest, ..)) if lowest < slot => {}
                    _ => {
                        first.insert(h_call, (slot, witness, answer));
                    }
                }
            }
            chosen[slot] = Some((witness, answer, val));
        }
        let (mut witnesses, mut said) =
            (base_witnesses.to_vec(), HashMap::<Call, Vec<Value>>::new());
        // How many slots before this one asked the same thing. Every one of
        // them has to be answered before this one can be.
        let mut earlier: HashMap<&Call, usize> = HashMap::new();
        for (slot, got) in chosen.into_iter().enumerate() {
            let call = &open[slot].0;
            let asked = earlier.entry(call).or_default();
            if let Some((witness, _, val)) = got {
                if said.get(call).map_or(0, Vec::len) != *asked {
                    return Err(MergeError::Mismatched(slot, holes[slot], witness));
                }
                witnesses.push(witness);
                said.entry(call.clone()).or_default().push(val);
            }
            *asked += 1;
        }
        Ok((witnesses, Self { said }))
    }
}
