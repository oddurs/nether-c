//! Who answers what, and what a grant is.

use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Mutex, PoisonError};

use nether_core::{Capability, Depth, Prim};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Refusal, Span, StoreError, Stored, Value};

use crate::recorder::{Recorded, Recorder};

/// The most read-stratum holes [`World::ask_all`] puts in flight at once.
const MOST_IN_FLIGHT: usize = 8;

/// Something that can answer one of §09's world-touching prelude functions.
///
/// The signature is the discipline. `answer` returns a [`Recorded`], the only
/// way to make one is [`Recorder::record`], and that writes before it returns —
/// so a provider cannot hand back a value it has not first written to the
/// ledger. §1.4 is a fact about this trait rather than a thing to remember.
pub trait Provider: Sync {
    /// The capability this provider is. §9.1 fixes the names.
    fn capability(&self) -> Capability;

    /// Whether it answers that prelude function.
    fn answers(&self, function: &str) -> bool;

    /// Answer one question, having written the answer down.
    ///
    /// A refusal is an answer (§9.9) and is recorded the same way. The error is
    /// for a ledger that would not take the answer, which is not the world
    /// saying no — it is this machine failing, and the program is told nothing.
    ///
    /// # Errors
    ///
    /// [`Refuse`] if the answer could not be written, or if the program asked
    /// something §9.9 makes a collapse rather than a refusal.
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse>;
}

/// What a provider could not do. §9.9's one, and the machine's two.
///
/// None of them is the world saying no: a refusal is an answer and comes back
/// as an ordinary [`Recorded`].
#[derive(Debug)]
pub enum Refuse {
    /// The ledger would not take the answer, so the program was told nothing.
    NotRecorded(StoreError),
    /// The program asked something that has no answer and never will — `env`
    /// on a name that was never declared (§9.4), `draw` of fewer than no
    /// bytes. Not catchable, by §9.9, so the rite reports it and stops.
    Collapse(String),
    /// This machine could not answer at all, and the question was good.
    ///
    /// Not §9.9's either: the program did nothing wrong and the world said
    /// nothing. It is what is left when a provider promised a source at the
    /// grant and does not have one at the call.
    Unavailable(String),
}

impl From<StoreError> for Refuse {
    fn from(e: StoreError) -> Self {
        Self::NotRecorded(e)
    }
}

/// What has been granted, and nothing else.
///
/// §9.1: a capability is acquired by `descend`, lexically scoped, never
/// ambient. A `World` holds the providers a rite was given and refuses every
/// question it has no provider for — which is what makes "no `--grant`" mean
/// something rather than mean "everything".
#[derive(Default)]
pub struct World {
    granted: Vec<Box<dyn Provider>>,
}

/// Why a question went unanswered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Unanswered {
    /// Nothing granted answers that function at that stratum.
    Ungranted {
        /// What was asked.
        function: String,
        /// The capability that would have answered it, if §9.1 has one.
        wanted: Option<Capability>,
    },
    /// The ledger would not take the answer, so the program was told nothing.
    NotRecorded(String),
    /// The program asked something that has no answer and never will. §9.9.
    Collapsed(String),
    /// This machine could not answer, and the question was good.
    Unavailable(String),
}

impl core::fmt::Display for Unanswered {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::Ungranted { function, wanted: Some(cap) } => {
                write!(f, "`{function}` needs `descend {cap}`, which was not granted")
            }
            Self::Ungranted { function, wanted: None } => {
                write!(f, "nothing granted answers `{function}`")
            }
            Self::NotRecorded(why) => write!(f, "the answer could not be written down: {why}"),
            Self::Collapsed(why) => write!(f, "{why}"),
            Self::Unavailable(why) => write!(f, "this machine cannot answer that: {why}"),
        }
    }
}

impl core::error::Error for Unanswered {}

impl World {
    /// A world with nothing granted. Every question goes unanswered.
    #[must_use]
    pub fn sealed() -> Self {
        Self::default()
    }

    /// Grant one capability, by handing over the thing that answers for it.
    #[must_use]
    pub fn granting(mut self, provider: Box<dyn Provider>) -> Self {
        self.granted.push(provider);
        self
    }

    /// Whether that capability was granted: `d ∈ δ`.
    ///
    /// A set and not the depth order. Authority is which doors are open, and
    /// comparing it by maximum would make *may draw random bytes* mean *may
    /// fetch a URL*. §2.1, and `nether_core::Held`.
    #[must_use]
    pub fn holds(&self, capability: Capability) -> bool {
        self.granted.iter().any(|p| p.capability() == capability)
    }

    /// The deepest stratum anything granted here can reach.
    #[must_use]
    pub fn depth(&self) -> Depth {
        self.granted.iter().fold(Depth::PURE, |d, p| d.join(p.capability().stratum()))
    }

    /// Put a question to whatever was granted for it.
    ///
    /// # Errors
    ///
    /// [`Unanswered`] if nothing granted answers it, or if the ledger would not
    /// take what came back. Neither is the world saying no: a refusal comes
    /// back as an ordinary [`Recorded`]. §9.9.
    pub fn ask(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Unanswered> {
        let Some(provider) = self.granted.iter().find(|p| p.answers(&call.function)) else {
            return Err(Unanswered::Ungranted {
                function: call.function.clone(),
                wanted: wants(&call.function),
            });
        };
        provider.answer(call, span, into).map_err(|e| match e {
            Refuse::NotRecorded(e) => Unanswered::NotRecorded(e.to_string()),
            Refuse::Collapse(why) => Unanswered::Collapsed(why),
            Refuse::Unavailable(why) => Unanswered::Unavailable(why),
        })
    }

    /// Answer a slice of holes in discovery order, overlapping contiguous runs
    /// of read-stratum holes (strata 1, 2, 3 and 5) up to [`MOST_IN_FLIGHT`] at
    /// a time.
    ///
    /// Act-stratum holes (strata 4, 6, 7 and 8) are answered alone in sequence,
    /// and a fatal failure stops at the first failing hole in discovery order
    /// while cancelling any later hole in the run that has not started yet.
    #[must_use]
    pub fn ask_all(
        &self,
        asked: &[(Call, Span)],
        into: &Recorder,
    ) -> Vec<Result<Recorded, Unanswered>> {
        let mut out = Vec::with_capacity(asked.len());
        let mut i = 0;
        while i < asked.len() {
            let end = asked[i..]
                .iter()
                .position(|(c, _)| !Self::reads(&c.function))
                .map_or(asked.len(), |n| i + n);
            if end <= i + 1 {
                let res = self.ask(&asked[i].0, asked[i].1, into);
                let stop = Self::fatal(&res);
                out.push(res);
                if stop {
                    return out;
                }
                i += 1;
                continue;
            }
            let (run, next, cutoff) =
                (&asked[i..end], AtomicUsize::new(0), AtomicUsize::new(end - i));
            let slots: Vec<Mutex<Option<Result<Recorded, Unanswered>>>> =
                (0..run.len()).map(|_| Mutex::new(None)).collect();
            std::thread::scope(|s| {
                for _ in 0..run.len().min(MOST_IN_FLIGHT) {
                    s.spawn(|| {
                        loop {
                            let idx = next.fetch_add(1, Ordering::Relaxed);
                            if idx >= cutoff.load(Ordering::Acquire) {
                                break;
                            }
                            let res = self.ask(&run[idx].0, run[idx].1, into);
                            if Self::fatal(&res) {
                                cutoff.fetch_min(idx, Ordering::AcqRel);
                            }
                            *slots[idx].lock().unwrap_or_else(PoisonError::into_inner) = Some(res);
                        }
                    });
                }
            });
            for res in slots
                .into_iter()
                .map_while(|s| s.into_inner().unwrap_or_else(PoisonError::into_inner))
            {
                let stop = Self::fatal(&res);
                out.push(res);
                if stop {
                    return out;
                }
            }
            i = end;
        }
        out
    }

    /// Answer the assigned slot indices of a trace's `holes` list in ascending
    /// slot order (`spec/06-evaluation.md` §6.6), recording each witness in
    /// `into` before returning its `(slot, witness_cairn)` pair.
    ///
    /// # Errors
    ///
    /// Returns [`Unanswered`] if a slot index is out of range, if a referenced
    /// hole is missing or not a `Node::Hole` in the store, or if a provider
    /// fails with a collapse, unavailable source or store write error.
    pub fn ask_slots(
        &self,
        holes: &[Cairn],
        slots: &[usize],
        into: &Recorder,
    ) -> Result<Vec<(usize, Cairn)>, Unanswered> {
        let mut ordered: Vec<usize> = slots.to_vec();
        ordered.sort_unstable();
        ordered.dedup();
        let mut asked = Vec::with_capacity(ordered.len());
        for &slot in &ordered {
            let Some(&hole) = holes.get(slot) else {
                return Err(Unanswered::NotRecorded(format!("hole slot {slot} out of range")));
            };
            match into.store().get(hole) {
                Ok(Stored::Node(Node::Hole { call, span, .. })) => asked.push((call, span)),
                Ok(_) => return Err(Unanswered::NotRecorded(format!("{hole} is not a hole"))),
                Err(e) => return Err(Unanswered::NotRecorded(e.to_string())),
            }
        }
        let mut delivered = Vec::new();
        for (slot, res) in ordered.into_iter().zip(self.ask_all(&asked, into)) {
            match res {
                Ok(rec) => delivered.push((slot, rec.witness())),
                Err(Unanswered::Ungranted { .. }) => {}
                Err(fatal) => return Err(fatal),
            }
        }
        Ok(delivered)
    }

    fn reads(function: &str) -> bool {
        Prim::from_name(function).is_some_and(|p| p.latent().reads())
    }

    const fn fatal(res: &Result<Recorded, Unanswered>) -> bool {
        !matches!(res, Ok(_) | Err(Unanswered::Ungranted { .. }))
    }
}

/// The capability §09 says that prelude function needs.
fn wants(function: &str) -> Option<Capability> {
    let prim = nether_core::Prim::from_name(function)?;
    Capability::at(prim.latent())
}

/// What the world said, shaped the way §09 types the function that asked.
///
/// [`Prim::refusable`] is the rule, stated here so no provider decides for
/// itself. `read` returns `Answer<Bytes>` and is wrapped; `exists` returns a
/// plain `Bool` and is not, because an `Answer` substituted where the program
/// declared a `Bool` is a branch that cannot fold and a deposit inside it that
/// never happens. 0171.
pub(crate) fn given(function: &str, v: Value) -> Value {
    match Prim::from_name(function) {
        Some(p) if !p.refusable() => v,
        _ => Value::Answer(Box::new(AnswerOf::Given(v))),
    }
}

/// Which no it was. §5.1.1's closed set of six, and §9.9 makes it an answer.
pub(crate) fn refused(r: Refusal) -> Value {
    Value::Answer(Box::new(AnswerOf::Refused(r)))
}
