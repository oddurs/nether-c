//! Writing an answer down, and the proof that it was written.

use nether_core::Depth;
use nether_ledger::{Cairn, Call, Node, Span, Store, StoreError, Stored, Value};

use crate::provider::Refuse;

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
/// Holds the store the answer goes to, and nothing else. A recorder does not
/// need to know where the question came from: the span it is handed names its
/// own source by cairn, because that is what §7.3 makes a span.
pub struct Recorder<'a> {
    store: &'a Store,
}

impl<'a> Recorder<'a> {
    /// A recorder over that store.
    #[must_use]
    pub const fn new(store: &'a Store) -> Self {
        Self { store }
    }

    /// The ledger, for reading a call's arguments back.
    ///
    /// A `Call` holds its arguments by cairn (§7.3), so a provider that wants
    /// the path it was asked about has to look it up. Reading is all this is
    /// for: writing an answer goes through [`Recorder::record`], which is the
    /// only thing that makes the proof.
    #[must_use]
    pub const fn store(&self) -> &Store {
        self.store
    }

    /// Reads argument `index` of `call` from the ledger, propagating any
    /// [`StoreError`] rather than disguising store corruption or missing
    /// argument objects as malformed program inputs.
    ///
    /// # Errors
    ///
    /// Returns [`StoreError`] if the argument cairn cannot be read or verified.
    pub fn arg(&self, call: &Call, index: usize) -> Result<Option<Value>, StoreError> {
        let Some(&cairn) = call.args.get(index) else {
            return Ok(None);
        };
        match self.store.get(cairn)? {
            Stored::Value(v) => Ok(Some(v)),
            Stored::Node(_) => Ok(None),
        }
    }

    /// Reads a `Str` argument from `call` at `index`.
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] from [`Self::arg`].
    pub fn arg_str(&self, call: &Call, index: usize) -> Result<Option<String>, StoreError> {
        Ok(match self.arg(call, index)? {
            Some(Value::Str(s)) => Some(s),
            _ => None,
        })
    }

    /// Reads a `Bytes` argument from `call` at `index`.
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] from [`Self::arg`].
    pub fn arg_bytes(&self, call: &Call, index: usize) -> Result<Option<Vec<u8>>, StoreError> {
        Ok(match self.arg(call, index)? {
            Some(Value::Bytes(b)) => Some(b),
            _ => None,
        })
    }

    /// Reads a `Cairn` argument from `call` at `index`.
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] from [`Self::arg`].
    pub fn arg_cairn(&self, call: &Call, index: usize) -> Result<Option<Cairn>, StoreError> {
        Ok(match self.arg(call, index)? {
            Some(Value::Cairn(c)) => Some(c),
            _ => None,
        })
    }

    /// Reads an `I64` argument from `call` at `index`.
    ///
    /// # Errors
    ///
    /// Propagates [`StoreError`] from [`Self::arg`].
    pub fn arg_int(&self, call: &Call, index: usize) -> Result<Option<i64>, StoreError> {
        Ok(match self.arg(call, index)? {
            Some(Value::Int(n)) => Some(n),
            _ => None,
        })
    }

    /// Write down what the world said, and hand back the proof.
    ///
    /// The answer is written first, then the witness that names it, then the
    /// proof is returned. §1.4's order is this function's order, and it is the
    /// only order there is because nothing else produces a [`Recorded`].
    ///
    /// # Errors
    ///
    /// [`Refuse::NotRecorded`] if the ledger would not take it. Nothing is
    /// returned in that case, so nothing was answered — which is the right
    /// outcome: an answer that could not be recorded is an answer the program
    /// must not see.
    pub fn record(
        &self,
        call: &Call,
        stratum: Depth,
        span: Span,
        said: &Value,
    ) -> Result<Recorded, Refuse> {
        let answer = self.store.put(&Stored::Value(said.clone()))?;
        let witness = self.store.put(&Stored::Node(Node::Witness {
            stratum: stratum.get(),
            call: call.clone(),
            answer,
            span,
        }))?;
        Ok(Recorded { witness, answer })
    }

    /// Record a witness referencing an already-recorded `answer` cairn (used
    /// when deduplicating identical read-stratum holes at distinct spans).
    pub(crate) fn record_existing(
        &self,
        call: &Call,
        stratum: Depth,
        span: Span,
        answer: Cairn,
    ) -> Result<Recorded, Refuse> {
        let witness = self.store.put(&Stored::Node(Node::Witness {
            stratum: stratum.get(),
            call: call.clone(),
            answer,
            span,
        }))?;
        Ok(Recorded { witness, answer })
    }
}
