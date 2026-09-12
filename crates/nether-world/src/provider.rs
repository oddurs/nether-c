//! Who answers what, and what a grant is.

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Span, StoreError};

use crate::recorder::{Recorded, Recorder};

/// Something that can answer one of §09's world-touching prelude functions.
///
/// The signature is the discipline. `answer` returns a [`Recorded`], the only
/// way to make one is [`Recorder::record`], and that writes before it returns —
/// so a provider cannot hand back a value it has not first written to the
/// ledger. §1.4 is a fact about this trait rather than a thing to remember.
pub trait Provider {
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
    /// [`StoreError`] if the answer could not be written.
    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, StoreError>;
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

    /// Whether that capability has been granted.
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
        provider.answer(call, span, into).map_err(|e| Unanswered::NotRecorded(e.to_string()))
    }
}

/// The capability §09 says that prelude function needs.
fn wants(function: &str) -> Option<Capability> {
    let prim = nether_core::Prim::from_name(function)?;
    Capability::at(prim.latent())
}
