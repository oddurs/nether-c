//! Writing an answer down, and the proof that it was written.

use nether_core::Depth;
use nether_ledger::{Cairn, Call, Node, Span, Store, StoreError, Stored, Value};

/// What the world said, and the proof that it was written down first.
///
/// There is no way to construct one of these outside this module: the fields
/// are private and there is no constructor. The only thing that makes one is
/// [`Recorder::record`], which writes the answer and its witness to the ledger
/// before it returns. So a value of this type *is* the evidence §1.4 asks for,
/// and a [`crate::Provider`] that returns one has already recorded.
///
/// Forging one does not compile:
///
/// ```compile_fail
/// use nether_world::Recorded;
/// use nether_ledger::Cairn;
///
/// let name = Cairn::of_encoded(b"not written anywhere");
/// let forged = Recorded { witness: name, answer: name };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Recorded {
    witness: Cairn,
    answer: Cairn,
}

impl Recorded {
    /// The `Witness` node. §7.3.
    #[must_use]
    pub const fn witness(&self) -> Cairn {
        self.witness
    }

    /// What the world said, as a value in the ledger.
    ///
    /// A refusal is one of these like any other. §9.9: when the world is asked
    /// a question it is entitled to answer no to, that no **is** the answer.
    #[must_use]
    pub const fn answer(&self) -> Cairn {
        self.answer
    }
}

/// The only way to make a [`Recorded`].
///
/// Holds the store the answer goes to and the cairn of the source the question
/// was asked from, because a span names its source by cairn (§7.3).
pub struct Recorder<'a> {
    store: &'a Store,
    source: Cairn,
}

impl<'a> Recorder<'a> {
    /// A recorder over that store, for questions asked from that source.
    #[must_use]
    pub const fn new(store: &'a Store, source: Cairn) -> Self {
        Self { store, source }
    }

    /// The source questions are being asked from.
    #[must_use]
    pub const fn source(&self) -> Cairn {
        self.source
    }

    /// Write down what the world said, and hand back the proof.
    ///
    /// The answer is written first, then the witness that names it, then the
    /// proof is returned. §1.4's order is this function's order, and it is the
    /// only order there is because nothing else produces a [`Recorded`].
    ///
    /// # Errors
    ///
    /// [`StoreError`] if the ledger would not take it. Nothing is returned in
    /// that case, so nothing was answered — which is the right outcome: an
    /// answer that could not be recorded is an answer the program must not see.
    pub fn record(
        &self,
        call: &Call,
        stratum: Depth,
        span: Span,
        said: &Value,
    ) -> Result<Recorded, StoreError> {
        let answer = self.store.put(&Stored::Value(said.clone()))?;
        let witness = self.store.put(&Stored::Node(Node::Witness {
            stratum: stratum.get(),
            call: call.clone(),
            answer,
            span,
        }))?;
        Ok(Recorded { witness, answer })
    }
}
