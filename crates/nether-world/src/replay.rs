//! Answering from the ledger, and from nowhere else.

use std::collections::HashMap;

use nether_core::{Depth, Prim};
use nether_ledger::{Cairn, Call, Node, Span, Store, StoreError, Stored, Stratum, Value};

pub(crate) fn valid_call_stratum(call: &Call, stratum: Stratum) -> bool {
    Prim::from_name(call.function()).is_some_and(|p| {
        p.latent() > Depth::PURE
            && p.latent().get() == stratum.as_u8()
            && call.args().len() == p.arity()
    })
}

enum WitnessCheckError {
    Store(StoreError),
    BadWitness(Cairn),
    BadAnswer { witness: Cairn, answer: Cairn },
}

fn check_base_witnesses(
    store: &Store,
    witnesses: &[Cairn],
    mut replay: Option<&mut Replay>,
) -> Result<HashMap<Call, (Cairn, Cairn)>, WitnessCheckError> {
    let mut reads: HashMap<Call, (Cairn, Cairn)> = HashMap::new();
    for &w in witnesses {
        let Stored::Node(Node::Witness { stratum, call, answer, .. }) =
            store.get(w).map_err(WitnessCheckError::Store)?
        else {
            return Err(WitnessCheckError::BadWitness(w));
        };
        if !valid_call_stratum(&call, stratum) {
            return Err(WitnessCheckError::BadWitness(w));
        }
        let Stored::Value(v) = store.get(answer).map_err(WitnessCheckError::Store)? else {
            return Err(WitnessCheckError::BadAnswer { witness: w, answer });
        };
        if Depth::new(stratum.as_u8()).is_some_and(Depth::reads) {
            if let Some(&(_, prev_ans)) = reads.get(&call) {
                if prev_ans != answer {
                    return Err(WitnessCheckError::BadWitness(w));
                }
            } else {
                reads.insert(call.clone(), (w, answer));
            }
        }
        if let Some(out) = replay.as_deref_mut() {
            out.push_answer(call, v);
        }
    }
    Ok(reads)
}

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
    /// Distinct calls in first-seen insertion order so [`Replay::all`] is
    /// deterministic across runs rather than dependent on `HashMap` seed.
    order: Vec<Call>,
}

impl Replay {
    fn push_answer(&mut self, call: Call, value: Value) {
        if !self.said.contains_key(&call) {
            self.order.push(call.clone());
        }
        self.said.entry(call).or_default().push(value);
    }

    /// What those witnesses recorded.
    ///
    /// Takes the answers rather than the store they came from, because a
    /// `Replay` that kept a store could be asked to read one more thing.
    #[must_use]
    pub fn of(answers: &[(Call, Value)]) -> Self {
        let mut replay = Self::default();
        for (call, value) in answers {
            replay.push_answer(call.clone(), value.clone());
        }
        replay
    }

    /// Every witness a trace names, read out of the ledger.
    ///
    /// The store is used here and not kept: what comes back is a `Replay` that
    /// has already been told everything it will ever know.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if any witness or its recorded answer is absent,
    /// corrupt, or has the wrong object kind.
    pub fn of_trace(store: &Store, witnesses: &[Cairn]) -> Result<Self, StoreError> {
        // In the order the trace names them, because §6.3 merges a hole only
        // at a read stratum: one call can have been asked more than once, and
        // then the answers to it are told apart by nothing but their order.
        let mut replay = Self::default();
        check_base_witnesses(store, witnesses, Some(&mut replay)).map_err(|e| match e {
            WitnessCheckError::Store(err) => err,
            WitnessCheckError::BadWitness(asked)
            | WitnessCheckError::BadAnswer { answer: asked, .. } => {
                StoreError::Corrupt { asked, found: None, why: None }
            }
        })?;
        Ok(replay)
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
    /// Each call once per answer recorded for it, in deterministic order.
    pub fn all(&self) -> impl Iterator<Item = (&Call, &Value)> {
        self.order.iter().flat_map(|call| self.said[call].iter().map(move |v| (call, v)))
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
        let base_reads =
            check_base_witnesses(store, base_witnesses, None).map_err(|e| match e {
                WitnessCheckError::Store(err) => MergeError::Store(err),
                WitnessCheckError::BadWitness(w)
                | WitnessCheckError::BadAnswer { witness: w, .. } => MergeError::BaseWitness(w),
            })?;
        let mut open: Vec<(Cairn, Call, Stratum, Span)> = Vec::with_capacity(holes.len());
        for (slot, &h) in holes.iter().enumerate() {
            let Stored::Node(Node::Hole { call, stratum, span }) = store.get(h)? else {
                return Err(MergeError::Mismatched(slot, h, h));
            };
            if !valid_call_stratum(&call, stratum) {
                return Err(MergeError::Mismatched(slot, h, h));
            }
            open.push((h, call, stratum, span));
        }
        let mut chosen: Vec<ChosenSlot> = vec![None; holes.len()];
        // For each read-stratum call, the chosen slot lowest in the trace and
        // its witness and answer: what a later delivery has to agree with.
        let mut first: HashMap<&Call, (usize, Cairn, Cairn)> = HashMap::new();
        for &delivery in deliveries.iter().copied().flatten() {
            apply_delivery(store, &open, &base_reads, &mut first, &mut chosen, delivery)?;
        }
        let mut witnesses = base_witnesses.to_vec();
        let mut replay = Self::default();
        // How many slots before this one asked the same thing. Every one of
        // them has to be answered before this one can be.
        let mut earlier: HashMap<&Call, usize> = HashMap::new();
        let mut delivered: HashMap<&Call, usize> = HashMap::new();
        for (slot, got) in chosen.into_iter().enumerate() {
            let (hole, call, _, _) = &open[slot];
            let asked = earlier.entry(call).or_default();
            if let Some((witness, _, val)) = got {
                let seen = delivered.entry(call).or_default();
                if *seen != *asked {
                    return Err(MergeError::Mismatched(slot, *hole, witness));
                }
                *seen += 1;
                witnesses.push(witness);
                replay.push_answer(call.clone(), val);
            }
            *asked += 1;
        }
        Ok((witnesses, replay))
    }
}

type ChosenSlot = Option<(Cairn, Cairn, Value)>;

fn apply_delivery<'a>(
    store: &Store,
    open: &'a [(Cairn, Call, Stratum, Span)],
    base_reads: &HashMap<Call, (Cairn, Cairn)>,
    first: &mut HashMap<&'a Call, (usize, Cairn, Cairn)>,
    chosen: &mut [ChosenSlot],
    (slot, witness): (usize, Cairn),
) -> Result<(), MergeError> {
    let Some((hole, h_call, h_stratum, h_span)) = open.get(slot) else {
        return Err(MergeError::OutOfRange(slot, open.len()));
    };
    let hole = *hole;
    let Stored::Node(Node::Witness { stratum, call, answer, span }) = store.get(witness)? else {
        return Err(MergeError::Mismatched(slot, hole, witness));
    };
    let Stored::Value(val) = store.get(answer)? else {
        return Err(MergeError::Mismatched(slot, hole, answer));
    };
    if !valid_call_stratum(&call, stratum)
        || (stratum, &call, span) != (*h_stratum, h_call, *h_span)
    {
        return Err(MergeError::Mismatched(slot, hole, witness));
    }
    if let Some((prev, _, _)) = chosen[slot] {
        return if prev == witness {
            Ok(())
        } else {
            Err(MergeError::Conflict(slot, hole, prev, witness))
        };
    }
    if Depth::new(h_stratum.as_u8()).is_some_and(Depth::reads) {
        if let Some(&(base_w, base_ans)) = base_reads.get(h_call)
            && base_ans != answer
        {
            return Err(MergeError::Conflict(slot, hole, base_w, witness));
        }
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
    Ok(())
}
