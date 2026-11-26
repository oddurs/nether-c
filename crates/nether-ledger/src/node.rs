//! The node model.
//!
//! A node is what the ledger stores *about* evaluation, as distinct from a
//! value a program can hold. Nodes reference other nodes only by cairn, so the
//! graph is acyclic by construction: a node cannot name one that does not yet
//! exist, and one that exists cannot change.
//!
//! `spec/07-ledger.md` §7.3.

use crate::cairn::Cairn;
use crate::codec::DecodeError;
use crate::value::{NonEmptyName, Stratum, Value};

/// Node kind bytes, fixed by `spec/07-ledger.md` §7.3.1.
pub(crate) mod kind {
    pub const LITERAL: u8 = 0x00;
    pub const APPLY: u8 = 0x01;
    pub const HOLE: u8 = 0x02;
    pub const DEPOSIT: u8 = 0x03;
    pub const WITNESS: u8 = 0x04;
    pub const TRACE: u8 = 0x05;
}

/// Where in a source a thing happened.
///
/// The source is named by cairn rather than by path. A path is a fact about
/// one machine at one moment, and a trace has to mean the same thing on a
/// machine that has never seen that filesystem.
///
/// Private fields enforce `end >= start` (`spec/07-ledger.md` §7.3.1).
///
/// ```compile_fail
/// use nether_ledger::{Cairn, Span};
/// let _ = Span { source: Cairn::of_encoded(b"s"), start: 5, end: 3 };
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Span {
    source: Cairn,
    start: u64,
    end: u64,
}

impl Span {
    /// Constructs a validated span with `end >= start`.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::BackwardsSpan`] if `end < start`.
    pub const fn new(source: Cairn, start: u64, end: u64) -> Result<Self, DecodeError> {
        if end < start {
            Err(DecodeError::BackwardsSpan { start, end })
        } else {
            Ok(Self { source, start, end })
        }
    }

    /// Constructs a zero-width span at byte offset `at`.
    #[must_use]
    pub const fn point(source: Cairn, at: u64) -> Self {
        Self { source, start: at, end: at }
    }

    /// The cairn of the source bytes.
    #[must_use]
    pub const fn source(self) -> Cairn {
        self.source
    }

    /// Byte offset of the first byte.
    #[must_use]
    pub const fn start(self) -> u64 {
        self.start
    }

    /// Byte offset one past the last. Always `>= start()`.
    #[must_use]
    pub const fn end(self) -> u64 {
        self.end
    }
}

/// A question put to the world: a prelude function and its finished arguments.
///
/// The arguments are already buried. `read(concat(dir, name))` is a call to
/// `read` with the finished path, not a call containing a `concat`.
///
/// Private fields enforce a non-empty function name (`spec/07-ledger.md` §7.3.1).
///
/// ```compile_fail
/// use nether_ledger::Call;
/// let _ = Call { function: String::new(), args: Vec::new() };
/// ```
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Call {
    function: NonEmptyName,
    args: Vec<Cairn>,
}

impl Call {
    /// Constructs a call from an already-validated function name and argument cairns.
    #[must_use]
    pub const fn new(function: NonEmptyName, args: Vec<Cairn>) -> Self {
        Self { function, args }
    }

    /// Constructs a call after verifying `function` is non-empty.
    ///
    /// # Errors
    ///
    /// Returns [`DecodeError::EmptyCallName`] if `function` is empty.
    pub fn try_new(function: impl Into<String>, args: Vec<Cairn>) -> Result<Self, DecodeError> {
        let function = NonEmptyName::new(function).map_err(|_| DecodeError::EmptyCallName)?;
        Ok(Self { function, args })
    }

    /// The prelude function's name. Never empty.
    #[must_use]
    pub fn function(&self) -> &str {
        self.function.as_str()
    }

    /// Cairns of the fully-evaluated arguments.
    #[must_use]
    pub fn args(&self) -> &[Cairn] {
        &self.args
    }

    pub(crate) fn into_args(self) -> Vec<Cairn> {
        self.args
    }
}

/// A node: the unit the ledger stores.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Node {
    /// A value that was written down rather than computed.
    Literal(Value),
    /// A function applied to arguments, and what came out.
    Apply {
        /// The function.
        function: Cairn,
        /// Its arguments, in order.
        args: Vec<Cairn>,
        /// What it produced.
        result: Cairn,
    },
    /// A suspended world-question.
    ///
    /// A hole does not record its dependents. It cannot: it would get a new
    /// name every time something came to wait on it. What is suspended on a
    /// hole is derived by reading the graph backwards.
    /// `spec/06-evaluation.md` §6.3.
    Hole {
        /// What is being asked.
        call: Call,
        /// The stratum the call would reach.
        stratum: Stratum,
        /// Where it was asked.
        span: Span,
    },
    /// A value a program left behind. Never printed; read afterwards with a lamp.
    Deposit {
        /// The value deposited.
        value: Cairn,
        /// Where it was deposited.
        span: Span,
    },
    /// What the world said, written down before the program was told.
    ///
    /// A refusal is an ordinary witness. `spec/09-prelude.md` §9.9.
    Witness {
        /// The stratum that was reached.
        stratum: Stratum,
        /// What was asked.
        call: Call,
        /// What came back.
        answer: Cairn,
        /// Where it was asked.
        span: Span,
    },
    /// A whole burial.
    Trace {
        /// The residue, as source, named like any other `Bytes`.
        ///
        /// A residue is a program and not a set of nodes: it holds branches
        /// and loops and bindings, none of which is a thing that happened.
        /// `spec/06-evaluation.md` §6.5.
        residue: Cairn,
        /// The holes, in the order they were discovered.
        holes: Vec<Cairn>,
        /// The witnesses, in the order the world answered.
        ///
        /// What a burial was told. A trace names them because nothing else
        /// can: a witness records no forward edge to the hole it answered, so
        /// without this a recorded answer is unreachable — and replay is
        /// re-burying with only the recorded answers. §6.6, §6.7.
        witnesses: Vec<Cairn>,
        /// What the program deposited, in source order.
        deposits: Vec<Cairn>,
        /// The source this burial started from.
        ///
        /// What the burial *spent* is not here. §6.7 asks a sealed trace to
        /// replay byte-identically, and replay buries a residue that is
        /// already folded — a cheaper burial — so a trace holding that number
        /// could never come back as itself. §8.2.
        source: Cairn,
        /// The deepest stratum reached.
        depth: Stratum,
        /// Whether this trace reached stratum 8.
        ///
        /// Permanent and transitive. A marked trace may never claim to be
        /// replayable. `spec/01-strata.md` §1.7.
        unrecorded: bool,
    },
}

impl Node {
    /// The kind byte this node encodes under.
    pub(crate) const fn kind(&self) -> u8 {
        match self {
            Self::Literal(_) => kind::LITERAL,
            Self::Apply { .. } => kind::APPLY,
            Self::Hole { .. } => kind::HOLE,
            Self::Deposit { .. } => kind::DEPOSIT,
            Self::Witness { .. } => kind::WITNESS,
            Self::Trace { .. } => kind::TRACE,
        }
    }

    /// The name of this node.
    #[must_use]
    pub fn cairn(&self) -> Cairn {
        Cairn::of_encoded(&self.encode())
    }

    /// The cairns this node names that are themselves nodes.
    ///
    /// A node names two kinds of thing, and §7.3 says which is which: a
    /// `Trace`'s `holes`, `witnesses`, and `deposits` are nodes, and every other
    /// cairn a node holds — an argument, an answer, a deposited value, a
    /// span's source — is a value. A walk that wants the shape of an
    /// evaluation rather than its contents wants these.
    #[must_use]
    pub fn nodes(&self) -> Vec<Cairn> {
        match self {
            // The residue and the source are `Bytes` values, not nodes.
            Self::Trace { holes, witnesses, deposits, .. } => {
                let mut out = holes.clone();
                out.extend_from_slice(witnesses);
                out.extend_from_slice(deposits);
                out
            }
            Self::Literal(_)
            | Self::Apply { .. }
            | Self::Hole { .. }
            | Self::Deposit { .. }
            | Self::Witness { .. } => Vec::new(),
        }
    }

    /// Every cairn this node names, in encoding order.
    ///
    /// This is the forward edge. Provenance and "what is waiting on this hole"
    /// are both the same edge read backwards. `spec/07-ledger.md` §7.4. For
    /// only the ones that are nodes, see [`Node::nodes`].
    #[must_use]
    pub fn references(&self) -> Vec<Cairn> {
        match self {
            Self::Literal(_) => Vec::new(),
            Self::Apply { function, args, result } => {
                let mut out = vec![*function];
                out.extend_from_slice(args);
                out.push(*result);
                out
            }
            Self::Hole { call, span, .. } => {
                let mut out = call.args.clone();
                out.push(span.source);
                out
            }
            Self::Deposit { value, span } => vec![*value, span.source],
            Self::Witness { call, answer, span, .. } => {
                let mut out = call.args.clone();
                out.push(*answer);
                out.push(span.source);
                out
            }
            Self::Trace { residue, holes, witnesses, deposits, source, .. } => {
                let mut out = vec![*residue];
                out.extend_from_slice(holes);
                out.extend_from_slice(witnesses);
                out.extend_from_slice(deposits);
                out.push(*source);
                out
            }
        }
    }
}
