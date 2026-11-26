//! Burial: the only form of evaluation Nether C has.
//!
//! `spec/06-evaluation.md`. Burial evaluates every expression it can and stops
//! at every expression it cannot, and what survives is the **residue**: a
//! complete program that can be buried again.
//!
//! Evaluation is demand-driven. A unit-level binding is evaluated the first
//! time something reaches it and never otherwise, and a function body is
//! evaluated when it is called. A world-question becomes a `Hole` node with a
//! name, and `seal` becomes the cairn of what it named — nothing is written
//! anywhere, because writing to a store is stratum 1 and burial holds no
//! capability at all.

mod analysis;
mod block;
mod eval;
mod fold;
mod residue;
mod state;

use std::collections::HashMap;

use nether_core::{Block, Depth, Expr, ExprKind, LocalId, Place, Span, Stmt, Type, Unit};
use nether_ledger::{Cairn, Value};

pub use residue::{Grinding, Halt, HaltKind, MAX_CONCAT, MAX_FRAMES, Residue, STACK};

use state::Burial;
#[cfg(not(target_family = "wasm"))]
use state::MAX_DEFAULT_THREADS;

/// What the world has already said, for a burial that is not the first.
///
/// Burial never asks. An answer is in here because a rite that held a grant
/// got it and wrote it to the ledger before this ran — which is §1.4's order,
/// kept by putting the asking somewhere burial cannot reach. Given one, a
/// world-question folds to what was said instead of becoming a hole. §6.6.
#[derive(Debug, Clone, Default)]
pub struct Answers(HashMap<nether_ledger::Call, Vec<Value>>);

impl Answers {
    /// Nothing has been answered. Every world-question becomes a hole.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// What the world said to that question, as it was written down.
    ///
    /// Appended rather than replaced. §6.3 merges a hole only at a read
    /// stratum, so one call can be asked more than once, and then it is
    /// answered more than once — the *n*th hole takes the *n*th answer, in the
    /// order §6.2 fixes. A read has one hole and one answer and never reaches
    /// past the first.
    #[must_use]
    pub fn and(mut self, asked: nether_ledger::Call, said: Value) -> Self {
        self.0.entry(asked).or_default().push(said);
        self
    }

    pub(crate) fn get(&self, asked: &nether_ledger::Call, nth: usize) -> Option<&Value> {
        self.0.get(asked)?.get(nth)
    }

    /// How many answers it holds.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.values().map(Vec::len).sum()
    }

    /// Whether nothing has.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Bury a unit with a budget of evaluation steps.
///
/// `source` names the bytes the unit was lowered from. Spans in the IR are
/// offsets into it and become ledger spans here, because this is where a span
/// stops being a fact about a file somebody has open and starts being a fact
/// about a source that has a name. `spec/07-ledger.md` §7.3.
///
/// # Errors
///
/// Collapse, exhausted fuel, or an implementation limit
/// ([`HaltKind::TooDeep`], [`HaltKind::TooLarge`], [`HaltKind::NoStack`]).
pub fn bury(unit: &Unit, source: Cairn, fuel: u64) -> Result<Residue, Halt> {
    bury_with(unit, source, fuel, &Answers::none())
}

/// Bury a unit that the world has already answered some of.
///
/// The first burial of a program is [`bury`], which is this with nothing
/// answered. §6.6's exhumation is this with what a grant got, and §6.5's
/// staging law is the claim that the two routes agree.
///
/// # Errors
///
/// As [`bury`].
pub fn bury_with(
    unit: &Unit,
    source: Cairn,
    fuel: u64,
    answers: &Answers,
) -> Result<Residue, Halt> {
    #[cfg(target_family = "wasm")]
    let cores = 1;
    #[cfg(not(target_family = "wasm"))]
    let cores =
        std::thread::available_parallelism().map_or(1, usize::from).min(MAX_DEFAULT_THREADS);
    bury_on(unit, source, fuel, answers, cores)
}

/// Bury on at most this many threads. §6.2.1 makes the count invisible: one
/// thread and sixty-four leave the same trace, halt at the same place and
/// report the same steps. More threads only bury independent demands sooner.
///
/// # Errors
///
/// As [`bury`].
pub fn bury_on(
    unit: &Unit,
    source: Cairn,
    fuel: u64,
    answers: &Answers,
    threads: usize,
) -> Result<Residue, Halt> {
    let run = || Burial::new(unit, source, fuel, answers, false).burrow(threads);
    #[cfg(target_family = "wasm")]
    {
        run()
    }
    #[cfg(not(target_family = "wasm"))]
    std::thread::scope(|s| {
        match std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, run) {
            // A burial that panicked is a bug in this crate, and the caller
            // should see it as one.
            Ok(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
            // Nothing left to spawn with. Not a reason to run on the
            // caller's stack: MAX_FRAMES is thirty-two megabytes of frames
            // against a main thread that has eight, so the fallback would be
            // the overflow this thread exists to prevent.
            Err(_) => Err(Halt::at(Span::default(), HaltKind::NoStack)),
        }
    })
}

pub(crate) fn u32_index(i: usize) -> u32 {
    u32::try_from(i).unwrap_or(u32::MAX)
}

pub(crate) fn unit_block(stmts: Vec<Stmt>, span: Span) -> Expr {
    Expr {
        kind: ExprKind::Block(Block { stmts, tail: None, span }),
        ty: Type::Unit,
        depth: Depth::PURE,
        span,
    }
}

pub(crate) fn tail_block(stmts: Vec<Stmt>, tail: Expr, span: Span) -> Expr {
    let (ty, depth) = (tail.ty.clone(), tail.depth);
    Expr {
        kind: ExprKind::Block(Block { stmts, tail: Some(Box::new(tail)), span }),
        ty,
        depth,
        span,
    }
}

pub(crate) fn assign_local(local: LocalId, value: Expr, span: Span) -> Expr {
    let depth = value.depth;
    let place = Place { local, path: Vec::new() };
    Expr { kind: ExprKind::Assign { place, value: Box::new(value) }, ty: Type::Unit, depth, span }
}
