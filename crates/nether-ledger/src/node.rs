//! The node model.
//!
//! A node is what the ledger stores *about* evaluation, as distinct from a
//! value a program can hold. Nodes reference other nodes only by cairn, so the
//! graph is acyclic by construction: a node cannot name one that does not yet
//! exist, and one that exists cannot change.
//!
//! `spec/07-ledger.md` §7.3.

use crate::cairn::Cairn;
use crate::value::Value;

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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Span {
    /// The cairn of the source bytes.
    pub source: Cairn,
    /// Byte offset of the first byte.
    pub start: u64,
    /// Byte offset one past the last. Never before `start`.
    pub end: u64,
}

/// A question put to the world: a prelude function and its finished arguments.
///
/// The arguments are already buried. `read(concat(dir, name))` is a call to
/// `read` with the finished path, not a call containing a `concat`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Call {
    /// The prelude function's name. Never empty.
    pub function: String,
    /// Cairns of the fully-evaluated arguments.
    pub args: Vec<Cairn>,
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
        stratum: u8,
        /// Where it was asked.
        span: Span,
        /// What must exist for the call to be made at all.
        depends: Vec<Cairn>,
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
        stratum: u8,
        /// What was asked.
        call: Call,
        /// What came back.
        answer: Cairn,
        /// Where it was asked.
        span: Span,
    },
    /// A whole burial.
    Trace {
        /// What was demanded.
        roots: Vec<Cairn>,
        /// How many evaluation steps it took.
        fuel_spent: u64,
        /// The deepest stratum reached.
        depth: u8,
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
    /// `Trace`'s roots and a `Hole`'s `depends` are nodes, and every other
    /// cairn a node holds — an argument, an answer, a deposited value, a
    /// span's source — is a value. A walk that wants the shape of an
    /// evaluation rather than its contents wants these.
    #[must_use]
    pub fn nodes(&self) -> Vec<Cairn> {
        match self {
            Self::Trace { roots, .. } => roots.clone(),
            Self::Hole { depends, .. } => depends.clone(),
            Self::Literal(_) | Self::Apply { .. } | Self::Deposit { .. } | Self::Witness { .. } => {
                Vec::new()
            }
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
            Self::Hole { call, span, depends, .. } => {
                let mut out = call.args.clone();
                out.push(span.source);
                out.extend_from_slice(depends);
                out
            }
            Self::Deposit { value, span } => vec![*value, span.source],
            Self::Witness { call, answer, span, .. } => {
                let mut out = call.args.clone();
                out.push(*answer);
                out.push(span.source);
                out
            }
            Self::Trace { roots, .. } => roots.clone(),
        }
    }
}
