//! Burial: the only form of evaluation Nether C has.
//!
//! `spec/06-evaluation.md`. Burial evaluates every expression it can and stops
//! at every expression it cannot, and what survives is the **residue**: a
//! complete program that can be buried again.
//!
//! What it can evaluate is decided by depth and nothing else. A depth-0
//! expression is arithmetic on things that are already here, so it is reduced;
//! anything deeper is a question for the world, and no capability has been
//! granted to anything yet, so it stays. `spec/06-evaluation.md` §6.1.
//!
//! Evaluation is demand-driven. A unit-level binding is evaluated the first
//! time something reaches it and never otherwise, and a function body is
//! evaluated when it is called. Nothing a `demand` does not transitively
//! require is touched — not as an optimisation but because evaluating it could
//! reach the world and leave a witness the program never asked for (§6.2).
//!
//! A world-question becomes a `Hole` node with a name, and `seal` becomes the
//! cairn of what it named. Both of those are the ledger, which is why this
//! crate depends on it — but nothing is written anywhere. Writing to a store is
//! stratum 1 and burial holds no capability at all, so what comes back is the
//! nodes a rite will write when one is given the capability to.
//!
//! One thing is deliberately not here yet. **Aggregates**: no prelude function
//! that builds an array or a struct is folded, because there is no way to write
//! one down and a value that cannot be written down cannot be residualised.
//! 0116.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
use std::sync::{Arc, Condvar, Mutex, PoisonError};

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Diagnostic, Expr, ExprKind, FuncDef, FuncId, GlobalId,
    Held, Literal, LocalDef, LocalId, Place, Prim, Refusal as RefusalCode, Rite, Span, Stmt, Type,
    UnOp, Unit, children, grouped,
};
use nether_ledger::{Cairn, Node, Stored, Value};

/// What a burial produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residue {
    /// One residual expression per demand, in source order. A demand that
    /// reduced all the way is a literal.
    pub demands: Vec<Expr>,
    /// Everything burial named, in the order it named it — the values it
    /// finished and the nodes it made about them. Nothing has been written
    /// anywhere: writing to a store is stratum 1, and burial holds no
    /// capability at all.
    pub named: Vec<(Cairn, Stored)>,
    /// What the program deposited, in source order.
    ///
    /// A bare expression statement whose value is not `U0` deposits that value
    /// into the trace (§4.7). It is not printed and it is not discarded — a
    /// program holds no capability that reaches a terminal, so depositing is
    /// the only thing it *can* do with a value it wants kept.
    pub deposits: Vec<Cairn>,
    /// The holes, in the order they were discovered — which §6.2 fixes, since
    /// two burials of the same input produce the same trace including that
    /// order.
    pub holes: Vec<Cairn>,
    /// The functions minted for calls that starved, in the order they were
    /// minted.
    ///
    /// §6.5: a starved call whose arguments are all values residualises as its
    /// reduced body, and the reduced body needs none of those arguments, so it
    /// is written down as a function of no parameters. [`Residue::as_unit`]
    /// puts them after the ones that were buried.
    pub specialised: Vec<FuncDef>,
    /// Evaluation steps spent. Deterministic for a given unit and budget.
    pub fuel_spent: u64,
    /// The deepest stratum anything in the residue still reaches.
    ///
    /// Not `Trace`'s `depth`, which is the deepest stratum a witness *reached*
    /// — `spec/07-ledger.md` §7.3.2. A residue that has been answered nothing
    /// still reaches deep; a trace that has done nothing has not.
    pub depth: Depth,
    /// Reduced initializers for unit-level globals evaluated (or partially
    /// reduced with side effects) during this burial, applied by [`as_unit`].
    globals: Vec<Option<Expr>>,
}

impl Residue {
    /// Whatever this burial filed under that name.
    ///
    /// A scan. `named` is in the order things were named, which is what §6.2
    /// fixes and what a writer iterates; this is for looking one up
    /// afterwards, which nothing does in a loop.
    #[must_use]
    pub fn get(&self, cairn: Cairn) -> Option<&Stored> {
        self.named.iter().find(|(c, _)| *c == cairn).map(|(_, s)| s)
    }

    /// The value of that name, if it named a value.
    #[must_use]
    pub fn value(&self, cairn: Cairn) -> Option<&Value> {
        match self.get(cairn) {
            Some(Stored::Value(v)) => Some(v),
            _ => None,
        }
    }

    /// The residue as a program, ready to be printed.
    ///
    /// §6.5 requires that a residue can be written down as source and lowered
    /// again to the same program; without that the staging law is unprovable,
    /// because the second burial would not be burying the first one's residue.
    ///
    /// The declarations come from the unit that was buried. Burial reduces
    /// demands and never touches a `struct`, a function or a global, so
    /// carrying them across loses nothing and an unreduced call still has
    /// something to name. What burial adds is [`Residue::specialised`], and it
    /// adds them at the end so that a [`FuncId`] means the same thing in the
    /// residue as it did in the unit.
    ///
    /// # Panics
    ///
    /// If this residue did not come from that unit. Burial produces exactly one
    /// residual per demand, so a mismatch is a bug here rather than a program
    /// that can be reported on — and zipping the two would drop a demand and
    /// break the staging law with nothing said.
    #[must_use]
    pub fn as_unit(&self, buried: &Unit) -> Unit {
        assert_eq!(
            self.demands.len(),
            buried.demands.len(),
            "this residue has {} demands and the unit it is said to come from has {}",
            self.demands.len(),
            buried.demands.len()
        );
        let mut funcs = buried.funcs.clone();
        let spec_start = funcs.len();
        funcs.extend(self.specialised.iter().cloned());
        let mut locals: Vec<LocalDef> =
            buried.locals.iter().map(|l| LocalDef { asserted: None, ..l.clone() }).collect();
        let mut globals = buried.globals.clone();
        for (i, g) in globals.iter_mut().enumerate() {
            if let Some(Some(reduced)) = self.globals.get(i) {
                g.value = if reduced.depth < g.value.depth && g.ty != Type::Unit {
                    Self::depth_preserving_global(reduced, &g.value, &g.ty, g.span, &mut locals)
                } else {
                    reduced.clone()
                };
            }
            Self::land_refused(&mut g.value, &mut locals, true);
        }
        for f in &mut funcs[spec_start..] {
            Self::land_refused_block(&mut f.body, &mut f.locals, true);
        }
        let mut demands: Vec<Demand> = self
            .demands
            .iter()
            .zip(&buried.demands)
            .map(|(value, original)| Demand { value: value.clone(), span: original.span })
            .collect();
        for d in &mut demands {
            Self::land_refused(&mut d.value, &mut locals, false);
        }
        Unit { structs: buried.structs.clone(), funcs, globals, locals, demands }
    }

    /// Wrap a folded global initializer in `{ T __g = reduced; if (false) { __g = orig; } __g }`
    /// so `g` keeps its original static depth while never re-executing `orig` in Stage 2.
    fn depth_preserving_global(
        reduced: &Expr,
        orig: &Expr,
        ty: &Type,
        span: Span,
        locals: &mut Vec<LocalDef>,
    ) -> Expr {
        let id = Self::fresh_local("__g", ty, span, locals);
        let assign_orig = Expr {
            kind: ExprKind::Assign {
                place: Place { local: id, path: Vec::new() },
                value: Box::new(orig.clone()),
            },
            ty: Type::Unit,
            depth: orig.depth,
            span,
        };
        let guard = Expr {
            kind: ExprKind::Select {
                cond: Box::new(Expr {
                    kind: ExprKind::Literal(Literal::Bool(false)),
                    ty: Type::Bool,
                    depth: Depth::PURE,
                    span,
                }),
                then: Box::new(Expr {
                    kind: ExprKind::Block(Block {
                        stmts: vec![Stmt::Expr(assign_orig)],
                        tail: None,
                        span,
                    }),
                    ty: Type::Unit,
                    depth: Depth::PURE,
                    span,
                }),
                otherwise: Box::new(Expr {
                    kind: ExprKind::Literal(Literal::Unit),
                    ty: Type::Unit,
                    depth: Depth::PURE,
                    span,
                }),
            },
            ty: Type::Unit,
            depth: Depth::PURE,
            span,
        };
        let tail = Expr { kind: ExprKind::Local(id), ty: ty.clone(), depth: orig.depth, span };
        Expr {
            kind: ExprKind::Block(Block {
                stmts: vec![Stmt::Let { local: id, value: reduced.clone() }, Stmt::Expr(guard)],
                tail: Some(Box::new(tail)),
                span,
            }),
            ty: ty.clone(),
            depth: orig.depth,
            span,
        }
    }

    fn fresh_local(prefix: &str, ty: &Type, span: Span, locals: &mut Vec<LocalDef>) -> LocalId {
        let mut n = locals.len();
        let name = loop {
            let cand = format!("{prefix}_{n}");
            if !locals.iter().any(|l| l.name == cand) {
                break cand;
            }
            n += 1;
        };
        let id = LocalId(u32::try_from(locals.len()).unwrap_or(u32::MAX));
        locals.push(LocalDef { name, ty: ty.clone(), asserted: None, span });
        id
    }

    fn land_refused_block(b: &mut Block, locals: &mut Vec<LocalDef>, tail_landed: bool) {
        for s in &mut b.stmts {
            match s {
                Stmt::Let { value, .. } => Self::land_refused(value, locals, true),
                Stmt::Expr(e) => Self::land_refused(e, locals, false),
                Stmt::Declare { .. } => {}
            }
        }
        if let Some(t) = b.tail.as_deref_mut() {
            Self::land_refused(t, locals, tail_landed);
        }
    }

    /// Wrap bare `refused(code)` calls that appear where `nether-syntax::lower`
    /// has no declared `Answer<T>` landing type into `{ Answer<T> __r = refused(code); __r }`.
    fn land_refused(x: &mut Expr, locals: &mut Vec<LocalDef>, landed: bool) {
        let is_bare_refused = matches!(
            &x.kind,
            ExprKind::Call { callee, .. } if matches!(callee.kind, ExprKind::Prim(Prim::Refused))
        );
        if is_bare_refused && !landed {
            let orig = x.clone();
            let span = orig.span;
            let ty = orig.ty.clone();
            let depth = orig.depth;
            let id = Self::fresh_local("__r", &ty, span, locals);
            let tail = Expr { kind: ExprKind::Local(id), ty: ty.clone(), depth, span };
            *x = Expr {
                kind: ExprKind::Block(Block {
                    stmts: vec![Stmt::Let { local: id, value: orig }],
                    tail: Some(Box::new(tail)),
                    span,
                }),
                ty,
                depth,
                span,
            };
            return;
        }
        match &mut x.kind {
            ExprKind::Block(b) => Self::land_refused_block(b, locals, landed),
            ExprKind::Descend { body, .. }
            | ExprKind::Rite { rite: Rite::Opaque, operand: body } => {
                Self::land_refused(body, locals, landed);
            }
            ExprKind::Rite { operand, .. }
            | ExprKind::Unary { operand, .. }
            | ExprKind::Field { base: operand, .. } => {
                Self::land_refused(operand, locals, false);
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
                Self::land_refused(lhs, locals, false);
                Self::land_refused(rhs, locals, false);
            }
            ExprKind::Select { cond, then, otherwise } => {
                Self::land_refused(cond, locals, false);
                Self::land_refused(then, locals, landed);
                Self::land_refused(otherwise, locals, landed);
            }
            ExprKind::Loop { body, step } => {
                Self::land_refused(body, locals, false);
                if let Some(s) = step.as_deref_mut() {
                    Self::land_refused(s, locals, false);
                }
            }
            ExprKind::Return(Some(v)) => Self::land_refused(v, locals, true),
            ExprKind::Assign { place, value } => {
                for p in &mut place.path {
                    if let nether_core::Proj::Index(i) = p {
                        Self::land_refused(i, locals, false);
                    }
                }
                Self::land_refused(value, locals, true);
            }
            ExprKind::Call { callee, args } => {
                Self::land_refused(callee, locals, false);
                let poly_arg = matches!(
                    callee.kind,
                    ExprKind::Prim(Prim::Answered | Prim::Must | Prim::Given | Prim::Refusal)
                );
                let arg_landed = !poly_arg && matches!(callee.ty, Type::Fn { .. });
                for a in args {
                    Self::land_refused(a, locals, arg_landed);
                }
            }
            ExprKind::Literal(_)
            | ExprKind::Local(_)
            | ExprKind::Global(_)
            | ExprKind::Func(_)
            | ExprKind::Prim(_)
            | ExprKind::Break
            | ExprKind::Continue
            | ExprKind::Return(None) => {}
        }
    }

    /// What the world is being asked, in order.
    #[must_use]
    pub fn questions(&self) -> Vec<&Node> {
        if self.holes.len() <= 8 {
            return self
                .holes
                .iter()
                .filter_map(|c| match self.get(*c) {
                    Some(Stored::Node(n)) => Some(n),
                    _ => None,
                })
                .collect();
        }
        let by_cairn: HashMap<Cairn, &Stored> = self.named.iter().map(|(c, s)| (*c, s)).collect();
        self.holes
            .iter()
            .filter_map(|c| match by_cairn.get(c) {
                Some(Stored::Node(n)) => Some(n),
                _ => None,
            })
            .collect()
    }
}

/// Why a burial stopped without finishing.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Halt {
    /// What to point at. For fuel this is the construct evaluation was going
    /// round in, not the expression it happened to be holding when the budget
    /// reached zero: §6.4 asks for where it starved rather than where it
    /// stopped.
    pub span: Span,
    /// What stopped it.
    pub kind: HaltKind,
    /// What it was grinding through, when it was grinding through something.
    pub grinding: Option<Grinding>,
}

/// The two ways a burial does not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltKind {
    /// An expression that cannot produce a value and never will. Not
    /// catchable, and never will be: every way to collapse is a mistake in
    /// the program rather than a fact about the world.
    ///
    /// Not to be confused with *starving*, which is what an expression does
    /// while it waits on a hole — that is ordinary, and is not a halt at all.
    /// `spec/09-prelude.md` §9.9.
    Collapsed(&'static str),
    /// The budget ran out. A diagnostic, not a crash.
    /// `spec/06-evaluation.md` §6.4.
    OutOfFuel {
        /// Steps spent, which is the budget it was given.
        spent: u64,
    },
    /// This implementation's own limit, which §6.4 requires it to state and
    /// to report rather than crash into. Fuel bounds work and this bounds
    /// space, and no single budget is both.
    TooDeep {
        /// [`MAX_FRAMES`].
        limit: u32,
    },
    /// There was no thread to bury on.
    ///
    /// [`MAX_FRAMES`] is a limit against [`STACK`] and means nothing against
    /// anybody else's, so a burial that cannot have its own stack does not run
    /// on the caller's — it says so. §6.4: a limit is reported, not crashed
    /// into.
    NoStack,
    /// A byte value grew beyond this implementation's [`MAX_CONCAT`] limit.
    TooLarge {
        /// [`MAX_CONCAT`].
        limit: usize,
    },
}

/// Maximum byte length produced by `concat`.
pub const MAX_CONCAT: usize =
    if cfg!(target_family = "wasm") { 4 * 1024 * 1024 } else { 64 * 1024 * 1024 };

/// How deep a chain of calls this burial can hold.
///
/// One of the two limits §6.4 requires an implementation to state. The other
/// is [`STACK`], and they are stated together because neither means anything
/// alone: burial evaluates by recursion, so a frame here is a handful of the
/// host's, and the limit is only a limit if the stack underneath it is known.
pub const MAX_FRAMES: u32 = if cfg!(target_family = "wasm") { 128 } else { 2048 };

/// The stack a burial is given.
///
/// Burial does not run on the stack it was called on. An unoptimised build
/// spends something like sixteen kilobytes of host stack per frame, and the
/// smallest stack a caller is likely to have — a test harness thread — holds
/// about a hundred of those, which is not a depth a language can offer. So it
/// runs on a stack of its own, sized for [`MAX_FRAMES`] with room to spare,
/// and the number is written down here rather than inherited from whoever
/// called. The browser instead uses the module's 8 MiB linker stack and a
/// conservative 128-frame limit; it cannot spawn a native thread.
pub const STACK: usize = if cfg!(target_family = "wasm") { 8 << 20 } else { 128 << 20 };

/// What burial was going round in when the fuel ran out.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Grinding {
    /// A loop, and how many times it went round.
    Loop {
        /// Completed turns.
        turns: u64,
    },
    /// A call chain, and how deep it went.
    Calls {
        /// Frames of it on the stack.
        deep: u64,
        /// What was being called.
        name: String,
    },
}

impl Halt {
    /// This halt, ready to print.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        let (headline, label, note) = match self.kind {
            HaltKind::Collapsed(why) => (
                why.to_string(),
                None,
                Some(
                    "a collapse is not catchable, and there will be no recovery form. \
                     This is a mistake in the program rather than a fact about the world."
                        .to_string(),
                ),
            ),
            HaltKind::TooDeep { limit } => (
                format!("burial went more than {limit} calls deep"),
                match &self.grinding {
                    Some(Grinding::Calls { deep, name }) => {
                        Some(format!("`{name}` called {} deep", grouped(*deep)))
                    }
                    _ => None,
                },
                Some(
                    "this is a limit of the implementation and not of the language. \
                     Recursion that does not stop is the usual reason for reaching it."
                        .to_string(),
                ),
            ),
            HaltKind::NoStack => (
                format!("burial could not get a stack of {} bytes", grouped(STACK as u64)),
                None,
                Some(
                    "burial runs on a stack of its own: its frame limit means \
                     nothing against anybody else's."
                        .to_string(),
                ),
            ),
            HaltKind::TooLarge { limit } => (
                format!("value exceeds the {} byte limit", grouped(limit as u64)),
                None,
                Some("this is a limit of the implementation and not of the language.".to_string()),
            ),
            HaltKind::OutOfFuel { spent } => (
                format!("burial ran out of fuel after {} steps", grouped(spent)),
                match &self.grinding {
                    Some(Grinding::Loop { turns }) => {
                        Some(format!("unrolled {} times", grouped(*turns)))
                    }
                    Some(Grinding::Calls { deep, name }) => {
                        Some(format!("`{name}` called {} deep", grouped(*deep)))
                    }
                    None => None,
                },
                Some(
                    "raise the budget with `--fuel`, or put an `opaque` barrier around it."
                        .to_string(),
                ),
            ),
        };
        Diagnostic { span: self.span, headline, label, cause: None, note }
    }
}

/// Maximum total expression recursion depth across nested evaluation frames.
/// Scaled above `MAX_FRAMES` so recursive function calls always hit `MAX_FRAMES`
/// first, while pathological non-call AST nesting stops before stack overflow.
const MAX_EXPR_DEPTH: u32 = MAX_FRAMES * 8;

/// Maximum default worker count when `bury_with` uses host parallelism.
const MAX_DEFAULT_THREADS: usize = 16;

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

    fn get(&self, asked: &nether_ledger::Call, nth: usize) -> Option<&Value> {
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
/// Starvation and exhausted fuel, which are different things, and this
/// implementation's own frame limit, which is a third.
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
    if cfg!(target_family = "wasm") {
        return run();
    }
    std::thread::scope(|s| {
        match std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, run) {
            // A burial that panicked is a bug in this crate, and the caller
            // should see it as one.
            Ok(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
            // Nothing left to spawn with. Not a reason to run on the
            // caller's stack: MAX_FRAMES is thirty-two megabytes of frames
            // against a main thread that has eight, so the fallback would be
            // the overflow this thread exists to prevent.
            Err(_) => Err(Halt { span: Span::default(), kind: HaltKind::NoStack, grinding: None }),
        }
    })
}

/// One demand, between the serial order and the workers: nobody has it yet,
/// somebody is burying it, or a worker finished and this is what it left.
enum Slot {
    Free,
    Taken,
    Done(Option<Residue>),
}

/// A slot per demand behind one lock, and the condition the serial order waits
/// on when a worker has the demand it reached. One allocation for every
/// demand, rather than a channel and a mutex each: most demands never meet a
/// worker. One lock, because a `Condvar` waits with exactly one. 0291.
type Handoff = (Mutex<Vec<Slot>>, Condvar);

/// Guarantees a claimed demand slot is released with `Slot::Done(None)` if a
/// worker panics, preventing `demands()` from waiting forever.
struct SlotGuard<'a> {
    handoff: &'a Handoff,
    index: usize,
    armed: bool,
}

impl<'a> SlotGuard<'a> {
    fn new(handoff: &'a Handoff, index: usize) -> Self {
        Self { handoff, index, armed: true }
    }

    fn finish(mut self, got: Option<Residue>) {
        self.armed = false;
        let mut slots = self.handoff.0.lock().unwrap_or_else(PoisonError::into_inner);
        slots[self.index] = Slot::Done(got);
        drop(slots);
        self.handoff.1.notify_all();
    }
}

impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            let mut slots = self.handoff.0.lock().unwrap_or_else(PoisonError::into_inner);
            slots[self.index] = Slot::Done(None);
            drop(slots);
            self.handoff.1.notify_all();
        }
    }
}

/// Maximum total loop fixpoint iterations across `walk_writes` and
/// `refresh_loop` so nested loops cannot blow up exponentially.
const MAX_FIXPOINT_STEPS: usize = 4096;

/// Snapshot of mutable burial side effects that speculative or abandoned
/// reductions (loops, branches, calls, speculation) must roll back together.
struct Checkpoint {
    deposits: usize,
    named: usize,
    specialised: usize,
    settling: usize,
    globals: Vec<Option<Val>>,
    globals_residual: Vec<Option<Expr>>,
    served: HashMap<nether_ledger::Call, usize>,
    starved: Option<Block>,
    flow: Option<Flow>,
    pc: Depth,
    act_blocked: bool,
}

/// Snapshot taken before an effectful statement that binds/assigns a non-closed
/// value (`Answer`/`Shade`/aggregate) so that if the enclosing block later
/// starves we can roll back its deposits and mutations without false starvation
/// when the block finishes cleanly.
struct DirtyRollback {
    index: usize,
    cp: Checkpoint,
    env: Env,
    kept_len: usize,
    eval_lets: HashSet<u32>,
    let_originals: HashMap<u32, Expr>,
}

impl<'a> Burial<'a> {
    fn new(unit: &'a Unit, source: Cairn, fuel: u64, answers: &'a Answers, ahead: bool) -> Self {
        Burial {
            unit,
            source,
            left: fuel,
            spent: 0,
            globals: vec![None; unit.globals.len()],
            globals_residual: vec![None; unit.globals.len()],
            eval_globals: Vec::new(),
            expr_depth: 0,
            flow: None,
            pc: Depth::PURE,
            grind: Vec::new(),
            answers,
            named: Vec::new(),
            known: HashSet::new(),
            holes: Vec::new(),
            deposits: Vec::new(),
            asked: HashMap::new(),
            served: HashMap::new(),
            reserved_acts: HashMap::new(),
            act_blocked: false,
            specialised: Vec::new(),
            minted: HashMap::new(),
            starved: None,
            speculating: false,
            settling: Vec::new(),
            nonclosed_dirty: Vec::new(),
            ahead,
            stop: None,
            serial_left: None,
        }
    }

    fn checkpoint(&mut self) -> Checkpoint {
        Checkpoint {
            deposits: self.deposits.len(),
            named: self.named.len(),
            specialised: self.specialised.len(),
            settling: self.settling.len(),
            globals: self.globals.clone(),
            globals_residual: self.globals_residual.clone(),
            served: self.served.clone(),
            starved: self.starved.take(),
            flow: self.flow.clone(),
            pc: self.pc,
            act_blocked: self.act_blocked,
        }
    }

    fn rollback(&mut self, cp: Checkpoint) {
        let rolled_back_effects = self.deposits.len() > cp.deposits || self.served != cp.served;
        self.deposits.truncate(cp.deposits);
        if self.named.len() > cp.named {
            // Keep holes (and their argument values) and kept deposits (and
            // their value entries); drop rolled-back deposits, deposit values,
            // and speculative `seal` values from `named[cp.named..]`.
            let mut keep: HashSet<Cairn> = self.holes.iter().copied().collect();
            keep.extend(self.deposits.iter().copied());
            for (c, stored) in self.named[cp.named..].iter().rev() {
                if keep.contains(c) {
                    match stored {
                        Stored::Node(n) => keep.extend(n.references()),
                        Stored::Value(v) => Self::value_cairns(v, &mut keep),
                    }
                }
            }
            let mut write = cp.named;
            for read in cp.named..self.named.len() {
                let c = self.named[read].0;
                if keep.contains(&c) {
                    if write != read {
                        self.named.swap(write, read);
                    }
                    write += 1;
                } else {
                    self.known.remove(&c);
                }
            }
            self.named.truncate(write);
        }
        for (call, &now_count) in &self.served {
            let prev = cp.served.get(call).copied().unwrap_or(0);
            if now_count > prev {
                *self.reserved_acts.entry(call.clone()).or_default() += now_count - prev;
            }
        }
        self.specialised.truncate(cp.specialised);
        let base = self.unit.funcs.len() + cp.specialised;
        self.minted.retain(|_, id| (id.0 as usize) < base);
        self.settling.truncate(cp.settling);
        for (i, prev) in cp.globals.into_iter().enumerate() {
            let Some(slot) = self.globals.get_mut(i) else { continue };
            let newly_eval = prev.is_none() && slot.is_some();
            if newly_eval
                && !self.speculating
                && (rolled_back_effects || slot.as_ref().is_some_and(Val::is_stuck))
                && let Some(g) = self.unit.globals.get(i)
            {
                let id = GlobalId(u32::try_from(i).unwrap_or(u32::MAX));
                let expr = Expr {
                    kind: ExprKind::Global(id),
                    ty: g.ty.clone(),
                    depth: g.value.depth,
                    span: g.span,
                };
                *slot = Some(Self::stuck(&expr));
            } else {
                *slot = prev;
            }
        }
        self.globals_residual = cp.globals_residual;
        self.served = cp.served;
        self.starved = cp.starved;
        self.flow = cp.flow;
        self.pc = cp.pc;
        if self.speculating {
            self.act_blocked = cp.act_blocked;
        }
    }

    fn value_cairns(v: &Value, into: &mut HashSet<Cairn>) {
        let mut todo = vec![v];
        while let Some(cur) = todo.pop() {
            match cur {
                Value::Cairn(c) => {
                    into.insert(*c);
                }
                Value::Shade { value, .. } => {
                    into.insert(*value);
                }
                Value::Answer(a) => {
                    if let nether_ledger::AnswerOf::Given(inner) = a.as_ref() {
                        todo.push(inner);
                    }
                }
                Value::Struct { fields, .. } | Value::Array(fields) => todo.extend(fields),
                Value::Unit
                | Value::Bool(_)
                | Value::Int(_)
                | Value::Bytes(_)
                | Value::Str(_)
                | Value::Refusal(_) => {}
            }
        }
    }

    /// Bury every demand, on this thread and up to `threads - 1` others.
    fn burrow(self, threads: usize) -> Result<Residue, Halt> {
        // This thread is the serial order's and the rest are workers. The
        // browser has no threads, and is not made to download the handoff.
        let workers = threads.min(self.unit.demands.len()).saturating_sub(1);
        if workers == 0 || cfg!(target_family = "wasm") {
            return self.demands(None, None);
        }
        let seed = Burial::new(self.unit, self.source, self.left, self.answers, true);
        let handoff: Handoff =
            (Mutex::new(self.unit.demands.iter().map(|_| Slot::Free).collect()), Condvar::new());
        // The first demand is the serial order's at once, so a worker starts
        // at the second, and stops when this runs past the last.
        let next = AtomicUsize::new(1);
        let stop = Arc::new(AtomicBool::new(false));
        let serial_left = Arc::new(AtomicU64::new(self.left));
        // Demands buried ahead that came to nothing, in a row. Each one costs
        // the serial order a wait for a worker's answer, and the answer is
        // that it has to do the demand itself. Once every worker has come to
        // nothing twice running, the demands left are probably the same kind,
        // and the serial order does them faster alone. Nothing it produces
        // depends on this: it decides every demand either way. 0291.
        let (misses, enough) = (AtomicUsize::new(0), 2 * workers);
        std::thread::scope(|s| {
            for _ in 0..workers {
                let work = || {
                    while misses.load(Relaxed) < enough {
                        let i = next.fetch_add(1, Relaxed);
                        {
                            let mut slots =
                                handoff.0.lock().unwrap_or_else(PoisonError::into_inner);
                            match slots.get_mut(i) {
                                None => break,
                                Some(slot @ Slot::Free) => *slot = Slot::Taken,
                                Some(_) => continue,
                            }
                        }
                        let guard = SlotGuard::new(&handoff, i);
                        let got = seed.early(i, &stop, &serial_left);
                        if got.is_some() {
                            misses.store(0, Relaxed);
                        } else {
                            misses.fetch_add(1, Relaxed);
                        }
                        guard.finish(got);
                    }
                };
                if std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, work).is_err() {
                    break;
                }
            }
            let out = self.demands(Some(&handoff), Some(&serial_left));
            next.store(usize::MAX / 2, Relaxed);
            stop.store(true, Relaxed);
            out
        })
    }

    /// One demand buried from nothing, on its own. If it reached nothing
    /// another demand could have changed, it is what the serial order would
    /// get, so it may be done first and anywhere. §6.2.1.
    fn early(
        &self,
        i: usize,
        stop: &Arc<AtomicBool>,
        serial_left: &Arc<AtomicU64>,
    ) -> Option<Residue> {
        let mut b = Burial::new(self.unit, self.source, self.left, self.answers, true);
        b.stop = Some(Arc::clone(stop));
        b.serial_left = Some(Arc::clone(serial_left));
        let d = self.unit.demands.get(i)?;
        let v = b.expr(&d.value, &mut Env::new(self.unit.locals.len())).ok()?;
        if !v.is_stuck()
            && Self::closed_residual(&v, d.span).is_none()
            && (!b.deposits.is_empty() || !b.served.is_empty())
        {
            return None;
        }
        b.ahead.then(|| b.finish(vec![Self::residual(v, &d.value)]))
    }

    /// Every demand, in source order, which §6.2 fixes: two burials of the
    /// same input produce the same trace, and that includes the order things
    /// were discovered in. A demand some worker took is waited for and kept
    /// if it fits in the fuel left; any other is buried here, so the serial
    /// order is what decides every hole, every halt and every step.
    fn demands(
        mut self,
        handoff: Option<&Handoff>,
        serial_left: Option<&Arc<AtomicU64>>,
    ) -> Result<Residue, Halt> {
        let mut demands = Vec::with_capacity(self.unit.demands.len());
        for (i, d) in self.unit.demands.iter().enumerate() {
            // Whoever reaches a demand first takes it, so nothing waits on a
            // demand no worker started.
            let got = handoff.and_then(|(slots, done)| {
                let mut slots = slots.lock().unwrap_or_else(PoisonError::into_inner);
                if matches!(slots.get(i)?, Slot::Free) {
                    slots[i] = Slot::Taken;
                    return None;
                }
                while matches!(slots[i], Slot::Taken) {
                    slots = done.wait(slots).unwrap_or_else(PoisonError::into_inner);
                }
                match std::mem::replace(&mut slots[i], Slot::Taken) {
                    Slot::Done(got) => got,
                    Slot::Free | Slot::Taken => None,
                }
            });
            match got {
                Some(e) if e.fuel_spent <= self.left => {
                    self.left -= e.fuel_spent;
                    self.spent += e.fuel_spent;
                    self.named.extend(e.named.into_iter().filter(|(c, _)| self.known.insert(*c)));
                    self.deposits.extend(e.deposits);
                    demands.extend(e.demands);
                }
                _ => {
                    self.flow = None;
                    self.pc = Depth::PURE;
                    self.settling.clear();
                    let cp = self.checkpoint();
                    match self.expr(&d.value, &mut Env::new(self.unit.locals.len())) {
                        Ok(v) => {
                            let non_closed_effects = !v.is_stuck()
                                && Self::closed_residual(&v, d.span).is_none()
                                && (self.deposits.len() > cp.deposits || self.served != cp.served);
                            let res = if non_closed_effects {
                                if self.served == cp.served {
                                    self.rollback(cp);
                                }
                                if Self::reaches_act(&d.value, self.unit) {
                                    self.act_blocked = true;
                                }
                                d.value.clone()
                            } else {
                                Self::residual(v, &d.value)
                            };
                            demands.push(res);
                        }
                        Err(halt) => return Err(self.blame(halt)),
                    }
                }
            }
            if let Some(sl) = serial_left {
                sl.store(self.left, Relaxed);
            }
        }
        Ok(self.finish(demands))
    }

    fn finish(self, demands: Vec<Expr>) -> Residue {
        let depth = demands.iter().fold(Depth::PURE, |acc, d| acc.join(d.depth));
        Residue {
            demands,
            specialised: self.specialised,
            named: self.named,
            deposits: self.deposits,
            holes: self.holes,
            fuel_spent: self.spent,
            depth,
            globals: self.globals_residual,
        }
    }

    /// The demand reached something demands share: a unit-level binding, the
    /// world, or a minted function. Sent ahead, it is no use, so it stops,
    /// here and not at its next step: the step it is on is naming a question
    /// or minting a function, which is the dearest thing a burial does, and
    /// nobody will take it. [`Burial::early`] turns the halt into no early
    /// result. The serial order is never ahead, so for it this does nothing.
    /// 0291.
    fn shares(&mut self) -> Result<(), Halt> {
        if !self.ahead {
            return Ok(());
        }
        (self.ahead, self.left) = (false, 0);
        let kind = HaltKind::OutOfFuel { spent: self.spent };
        Err(Halt { span: Span::default(), kind, grinding: None })
    }
}

/// Reference-counted literal stored inside [`Val`] so cloning environments or
/// values with multi-megabyte `Bytes`/`Str` payloads stays O(1).
#[derive(Debug, Clone, PartialEq, Eq)]
enum ValLit {
    Unit,
    Bool(bool),
    Int(i64),
    Bytes(Arc<[u8]>),
    Str(Arc<str>),
    Cairn([u8; 32]),
    Refusal(RefusalCode),
}

impl ValLit {
    fn from_literal(l: Literal) -> Self {
        match l {
            Literal::Unit => Self::Unit,
            Literal::Bool(b) => Self::Bool(b),
            Literal::Int(n) => Self::Int(n),
            Literal::Bytes(b) => Self::Bytes(Arc::from(b)),
            Literal::Str(s) => Self::Str(Arc::from(s)),
            Literal::Cairn(c) => Self::Cairn(c),
            Literal::Refusal(r) => Self::Refusal(r),
        }
    }

    fn to_literal(&self) -> Literal {
        match self {
            Self::Unit => Literal::Unit,
            Self::Bool(b) => Literal::Bool(*b),
            Self::Int(n) => Literal::Int(*n),
            Self::Bytes(b) => Literal::Bytes(b.to_vec()),
            Self::Str(s) => Literal::Str(s.to_string()),
            Self::Cairn(c) => Literal::Cairn(*c),
            Self::Refusal(r) => Literal::Refusal(*r),
        }
    }
}

/// A value, as burial holds one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Val {
    kind: Kind,
    ty: Type,
    depth: Depth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Something that is here.
    Known(ValLit),
    /// A value carried up out of a deeper stratum, still held.
    Shade(Box<Val>),
    /// What the world said: §5.1.1's `Given T`.
    ///
    /// There is no literal for an answer — §04 has no syntax to write one down
    /// — so this never becomes a residual. An answer that `must`, `given` or
    /// `refusal` consumes folds through; one that is bound stays the question
    /// it was and remains a hole for a later burial.
    Answered(Box<Val>),
    /// And §5.1.1's `Refused`, which is an answer like any other. §9.9.
    Refused(RefusalCode),
    /// A function, as a value.
    Func(FuncId),
    /// A prelude function, as a value.
    Prim(Prim),
    /// Something that is not here, and the expression that would produce it.
    Stuck(Box<Expr>),
}

impl Val {
    fn is_stuck(&self) -> bool {
        matches!(self.kind, Kind::Stuck(_))
    }

    fn int(&self) -> Option<i64> {
        match &self.kind {
            Kind::Known(ValLit::Int(n)) => Some(*n),
            _ => None,
        }
    }

    fn bool(&self) -> Option<bool> {
        match &self.kind {
            Kind::Known(ValLit::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    fn bytes(&self) -> Option<&[u8]> {
        match &self.kind {
            Kind::Known(ValLit::Bytes(b)) => Some(b),
            Kind::Known(ValLit::Str(s)) => Some(s.as_bytes()),
            _ => None,
        }
    }
}

/// A jump that is in flight.
///
/// Held on the burial rather than returned, because every form has to be able
/// to stop when one is set and threading it through each return type says the
/// same thing four times.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Flow {
    Broke,
    Continued,
    Returned(Val),
}

struct Burial<'a> {
    unit: &'a Unit,
    /// What the world already said. See [`Answers`].
    answers: &'a Answers,
    /// The cairn of the bytes the unit was lowered from.
    source: Cairn,
    left: u64,
    spent: u64,
    /// Unit-level bindings, evaluated the first time something reaches one.
    globals: Vec<Option<Val>>,
    /// Reduced global initializers written back into `Residue::as_unit`.
    globals_residual: Vec<Option<Expr>>,
    /// Unit-level bindings currently being evaluated, to detect cycles.
    eval_globals: Vec<GlobalId>,
    /// Current expression recursion depth, guarded by [`MAX_EXPR_DEPTH`].
    expr_depth: u32,
    /// A `break`, `continue` or `return` that has happened and not yet landed.
    flow: Option<Flow>,
    /// Current control-dependence `pc` from enclosing residual control flow.
    pc: Depth,
    /// The loops and calls currently open, innermost last. Nothing pops it on
    /// the way out of an error, which is the point: when the budget runs out
    /// this is what evaluation was going round in.
    grind: Vec<Grind>,
    /// Everything named so far, in the order it was named.
    named: Vec<(Cairn, Stored)>,
    /// The same names, for asking whether one is already there.
    ///
    /// `named` keeps the order §6.2 fixes and this keeps the lookup constant:
    /// scanning the vector made naming N values cost N², and §6.6 counts nine
    /// hundred nodes for a program with one hole in it.
    known: HashSet<Cairn>,
    /// The holes, in the order they were found.
    holes: Vec<Cairn>,
    /// What the program deposited, in source order.
    deposits: Vec<Cairn>,
    /// The functions minted for calls that starved. §6.5.
    specialised: Vec<FuncDef>,
    /// The same functions, by what they were specialised to, so that a
    /// function specialised twice to the same constants is minted once. A
    /// recursive interpreter reaches the same state from more than one place
    /// and a residue that held a copy for each would grow with the guest
    /// rather than with what is left of it.
    minted: HashMap<(FuncId, Vec<Literal>), FuncId>,
    /// Whether this is an arm of a branch the world has not decided. §6.5:
    /// such a reduction may produce a value or stop, and nothing else.
    speculating: bool,
    /// Assignments code left as written needs before it, waiting for the
    /// block whose statement that code is in. See [`Burial::settle`].
    settling: Vec<Stmt>,
    /// Locals bound or assigned a non-closed value (`Answer`, deep `Shade`,
    /// etc.) while depositing, mutating `env`, or serving an act.
    nonclosed_dirty: Vec<u32>,
    /// Whether this is a demand sent ahead of the serial order that has not
    /// yet reached anything another demand could change. §6.2.1.
    ahead: bool,
    /// Raised when the serial order is done with a worker's demand, whatever
    /// became of it. A worker stops at its next step rather than spending the
    /// rest of its fuel on a demand nobody will take. Only workers have one.
    stop: Option<Arc<AtomicBool>>,
    /// Remaining serial fuel, shared with workers so they bail out once their
    /// speculative spend can no longer fit in the serial budget.
    serial_left: Option<Arc<AtomicU64>>,
    /// The block the body just starved in, on the way out of [`Burial::block`].
    ///
    /// A side channel because a block's residue is not its value: the value is
    /// what starved and the residue is the statements around it.
    starved: Option<Block>,
    /// The holes already dug, by the question each one asks.
    ///
    /// Read strata only. §6.3 makes the `call` the identity there and not the
    /// node — a node carries the span of the place that asked, so interning on
    /// the node would make the same question asked from two places into two
    /// questions. At 4, 6, 7 and 8 nothing is interned and two asks are two
    /// holes, which is the same section read the other way.
    asked: HashMap<nether_ledger::Call, Cairn>,
    /// How many times each call has already been answered in this burial.
    ///
    /// The *n*th ask takes the *n*th answer (§6.3). Only an act counts: a read
    /// is one hole however many places wait on it, so every one of them takes
    /// the first answer and none of them uses it up.
    served: HashMap<nether_ledger::Call, usize>,
    /// Number of act-stratum answers reserved for earlier rolled-back residuals
    /// so later demands index past them instead of stealing or duplicating acts.
    reserved_acts: HashMap<nether_ledger::Call, usize>,
    /// Raised once an earlier demand or statement starves with an unresolved
    /// residual that can reach an act stratum; blocks later act calls from
    /// serving or digging out of source order (§6.3).
    act_blocked: bool,
}

/// One open loop or call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Grind {
    span: Span,
    what: Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Loop { turns: u64 },
    Call(FuncId),
}

/// A frame: one slot per local of the function being evaluated.
/// A frame's locals, and which of them a write the residue does not hold has
/// changed since the residue last said what they are.
///
/// A statement that only assigns is not in the residue: its value is here and
/// nowhere else. That is fine until code is left as written and reads the
/// local in the residue, and then the residue has to be told. 0285.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Env {
    slots: Vec<Option<Val>>,
    stale: Vec<bool>,
    write_gen: Vec<u64>,
    writes: u64,
}

impl Env {
    fn new(locals: usize) -> Self {
        Env {
            slots: vec![None; locals],
            stale: vec![false; locals],
            write_gen: vec![0; locals],
            writes: 0,
        }
    }

    fn get(&self, id: usize) -> Option<&Option<Val>> {
        self.slots.get(id)
    }

    /// A binding the residue holds as it is here.
    fn bind(&mut self, id: usize, v: Option<Val>) {
        if let (Some(slot), Some(stale)) = (self.slots.get_mut(id), self.stale.get_mut(id)) {
            self.writes = self.writes.wrapping_add(1);
            (*slot, *stale) = (v, false);
        }
    }

    /// A write the residue does not hold.
    fn write(&mut self, id: usize, v: Val) {
        if let (Some(slot), Some(stale), Some(wgen)) =
            (self.slots.get_mut(id), self.stale.get_mut(id), self.write_gen.get_mut(id))
        {
            self.writes = self.writes.wrapping_add(1);
            (*slot, *stale, *wgen) = (Some(v), true, self.writes);
        }
    }
}

impl Burial<'_> {
    /// Write a node down and hand back its name.
    ///
    /// Nothing is written to a store. Writing to a store is stratum 1 and
    /// burial holds no capability at all; these are the nodes a rite will
    /// write when one is given the capability to.
    fn remember(&mut self, what: Stored) -> Cairn {
        let cairn = what.cairn();
        if self.known.insert(cairn) {
            self.named.push((cairn, what));
        }
        cairn
    }

    /// A question for the world, named and written down.
    ///
    /// Two holes with identical calls in one trace MUST be the same hole,
    /// which is what makes exhumation cheap: reading the same file twice is
    /// one question, asked once. §6.3.
    fn dig(&mut self, p: Prim, call: nether_ledger::Call, span: Span) -> Cairn {
        // §6.3: two holes with identical calls at a read stratum MUST be the
        // same hole. The second place to ask is not a second question, so it
        // gets the hole the first one made — span and all. At 4, 6, 7 and 8
        // the second ask is a second act, and §1.8 is the whole reason.
        let shared = p.latent().reads();
        if shared && let Some(dug) = self.asked.get(&call) {
            return *dug;
        }
        let node = Node::Hole {
            call: call.clone(),
            stratum: p.latent().get(),
            span: nether_ledger::Span {
                source: self.source,
                start: u64::from(span.start),
                end: u64::from(span.end),
            },
        };
        let cairn = self.remember(Stored::Node(node));
        if shared {
            self.asked.insert(call, cairn);
        }
        // §7.3.1: one entry per question left open, which is not one entry per
        // node. Four `post`s from one line are one node — same call, same
        // stratum, same span, so one content address — and four entries, the
        // way four deposits from one line are. A read never reaches here twice
        // for one call: the interning above returned the first hole.
        self.holes.push(cairn);
        cairn
    }

    /// A value the program left behind, named and written down.
    ///
    /// §4.7. Unlike a hole, two identical deposits are two deposits: the
    /// program said the same thing twice and the trace records that it did.
    /// They differ by span, so they differ by cairn.
    fn deposit(&mut self, value: Value, span: Span) -> Cairn {
        let at = self.remember(Stored::Value(value));
        let node = Node::Deposit {
            value: at,
            span: nether_ledger::Span {
                source: self.source,
                start: u64::from(span.start),
                end: u64::from(span.end),
            },
        };
        self.remember(Stored::Node(node))
    }

    /// Where a halt should point, and what it was going round in.
    ///
    /// `burn` reports the leaf it was holding, which is almost never the
    /// interesting place. A loop that went round two hundred thousand times is
    /// the answer to *why did this not stop*; the expression it happened to be
    /// inside is not.
    fn blame(&self, halt: Halt) -> Halt {
        if matches!(halt.kind, HaltKind::Collapsed(_) | HaltKind::TooLarge { .. }) {
            return halt;
        }
        let looping = self
            .grind
            .iter()
            .rev()
            .filter_map(|g| match g.what {
                Open::Loop { turns } => Some((g.span, turns)),
                Open::Call(_) => None,
            })
            .max_by_key(|(_, turns)| *turns);
        let calls = self.grind.iter().filter(|g| matches!(g.what, Open::Call(_))).count() as u64;

        match looping {
            Some((span, turns)) if turns >= calls => {
                Halt { span, grinding: Some(Grinding::Loop { turns }), ..halt }
            }
            _ => {
                let first = self.grind.iter().find_map(|g| match g.what {
                    Open::Call(id) => Some((g.span, id)),
                    Open::Loop { .. } => None,
                });
                match first {
                    Some((span, id)) => {
                        let name = self
                            .unit
                            .func(id)
                            .map_or_else(|| "a function".to_string(), |f| f.name.clone());
                        Halt { span, grinding: Some(Grinding::Calls { deep: calls, name }), ..halt }
                    }
                    None => halt,
                }
            }
        }
    }

    fn burn(&mut self, span: Span) -> Result<(), Halt> {
        // A stopped worker's halt never reaches anybody: `early` turns it into
        // no early result, and the serial order has already moved on.
        if self.left == 0
            || self.stop.as_deref().is_some_and(|s| s.load(Relaxed))
            || self.serial_left.as_deref().is_some_and(|sl| self.spent >= sl.load(Relaxed))
        {
            let kind = HaltKind::OutOfFuel { spent: self.spent };
            return Err(Halt { span, kind, grinding: None });
        }
        self.left -= 1;
        self.spent += 1;
        Ok(())
    }

    /// Charge fuel proportional to large byte allocations/copies/hashes
    /// (`bytes / 64`, so small values < 64 B cost 0 extra fuel while multi-MiB
    /// concat/deposit/seal amplification hits `OutOfFuel` cleanly).
    fn burn_bytes(&mut self, bytes: usize, span: Span) -> Result<(), Halt> {
        let extra = u64::try_from(bytes / 64).unwrap_or(u64::MAX);
        if extra == 0 {
            return Ok(());
        }
        if self.left < extra
            || self.stop.as_deref().is_some_and(|s| s.load(Relaxed))
            || self
                .serial_left
                .as_deref()
                .is_some_and(|sl| self.spent.saturating_add(extra) > sl.load(Relaxed))
        {
            let kind = HaltKind::OutOfFuel { spent: self.spent };
            return Err(Halt { span, kind, grinding: None });
        }
        self.left -= extra;
        self.spent = self.spent.saturating_add(extra);
        Ok(())
    }

    /// Recursive byte payload size of a value (including inside `Shade`/`Answered`)
    /// so sealing, depositing, equality, and world-call naming charge proportionally
    /// to hashing/encoding work.
    fn val_byte_work(v: &Val) -> usize {
        match &v.kind {
            Kind::Known(ValLit::Bytes(b)) => b.len(),
            Kind::Known(ValLit::Str(s)) => s.len(),
            Kind::Shade(inner) | Kind::Answered(inner) => Self::val_byte_work(inner),
            _ => 0,
        }
    }

    /// `given`, `refusal` and `must`, which are how §9.9 says a program
    /// handles a no.
    ///
    /// Two of them collapse rather than answer, and §9.9 is explicit that this
    /// is not catchable: "every way to collapse is a mistake in the program
    /// rather than a fact about the world".
    fn inspect(&mut self, p: Prim, values: &[Val], x: &Expr) -> Result<Option<Val>, Halt> {
        let collapse =
            |why| Err(Halt { span: x.span, kind: HaltKind::Collapsed(why), grinding: None });
        let Some(answer) = values.first() else { return Ok(None) };
        Ok(match (p, &answer.kind) {
            (Prim::Utf8, Kind::Known(ValLit::Bytes(bytes))) => {
                self.burn_bytes(bytes.len(), x.span)?;
                Self::answered(
                    &Value::Answer(Box::new(match std::str::from_utf8(bytes) {
                        Ok(s) => nether_ledger::AnswerOf::Given(Value::Str(s.to_owned())),
                        Err(_) => {
                            nether_ledger::AnswerOf::Refused(nether_ledger::Refusal::Malformed)
                        }
                    })),
                    x,
                )
            }
            (Prim::Must, Kind::Answered(v)) => Some(Val { ty: x.ty.clone(), ..(**v).clone() }),
            (Prim::Must, Kind::Refused(_)) => return collapse("this answer was refused"),
            (Prim::Given, Kind::Answered(_)) => Some(Self::known(Literal::Bool(true), x)),
            (Prim::Given, Kind::Refused(_)) => Some(Self::known(Literal::Bool(false), x)),
            (Prim::Refusal, Kind::Refused(r)) => Some(Self::known(Literal::Refusal(*r), x)),
            (Prim::Refusal, Kind::Answered(_)) => return collapse("this answer was given"),
            (
                Prim::Answered,
                Kind::Known(_) | Kind::Shade(_) | Kind::Answered(_) | Kind::Refused(_),
            ) => {
                let kind = Kind::Answered(Box::new(answer.clone()));
                Some(Val { kind, ty: x.ty.clone(), depth: x.depth })
            }
            (Prim::Refused, Kind::Known(ValLit::Refusal(r))) => {
                Some(Val { kind: Kind::Refused(*r), ty: x.ty.clone(), depth: x.depth })
            }
            _ => None,
        })
    }

    /// What the world said, folded into the expression that asked.
    ///
    /// Only a literal can be folded: an aggregate has no way to be written
    /// down (0116), so returning `None` lets the caller rebuild the call with
    /// reduced arguments rather than discarding argument reductions.
    fn answered(said: &Value, x: &Expr) -> Option<Val> {
        let Value::Answer(a) = said else {
            return literal(said).map(|l| Val {
                kind: Kind::Known(ValLit::from_literal(l)),
                ty: x.ty.clone(),
                depth: x.depth,
            });
        };
        let inner_ty = match &x.ty {
            Type::Answer(inner) => (**inner).clone(),
            other => other.clone(),
        };
        let kind = match a.as_ref() {
            nether_ledger::AnswerOf::Refused(r) => Kind::Refused(code(*r)),
            nether_ledger::AnswerOf::Given(v) => {
                let l = literal(v)?;
                Kind::Answered(Box::new(Val {
                    kind: Kind::Known(ValLit::from_literal(l)),
                    ty: inner_ty,
                    depth: x.depth,
                }))
            }
        };
        Some(Val { kind, ty: x.ty.clone(), depth: x.depth })
    }

    fn known(kind: Literal, x: &Expr) -> Val {
        Val { kind: Kind::Known(ValLit::from_literal(kind)), ty: x.ty.clone(), depth: Depth::PURE }
    }

    fn unit() -> Val {
        Val { kind: Kind::Known(ValLit::Unit), ty: Type::Unit, depth: Depth::PURE }
    }

    fn stuck(x: &Expr) -> Val {
        Val { kind: Kind::Stuck(Box::new(x.clone())), ty: x.ty.clone(), depth: x.depth }
    }

    /// A closed, zero-depth expression for a finished value that references no
    /// caller locals, capabilities, or world calls.
    fn closed_residual(v: &Val, span: Span) -> Option<Expr> {
        match &v.kind {
            Kind::Known(l) => Some(Expr {
                kind: ExprKind::Literal(l.to_literal()),
                ty: v.ty.clone(),
                depth: Depth::PURE,
                span,
            }),
            Kind::Func(id) => {
                Some(Expr { kind: ExprKind::Func(*id), ty: v.ty.clone(), depth: Depth::PURE, span })
            }
            Kind::Prim(p) => {
                Some(Expr { kind: ExprKind::Prim(*p), ty: v.ty.clone(), depth: Depth::PURE, span })
            }
            Kind::Shade(inner) => {
                let Type::Shade { origin: Depth::PURE, .. } = v.ty else { return None };
                let op = Self::closed_residual(inner, span)?;
                Some(Expr {
                    kind: ExprKind::Rite { rite: Rite::Shade, operand: Box::new(op) },
                    ty: v.ty.clone(),
                    depth: Depth::PURE,
                    span,
                })
            }
            Kind::Answered(inner) if v.depth == Depth::PURE => {
                let arg = Self::closed_residual(inner, span)?;
                let callee = Expr {
                    kind: ExprKind::Prim(Prim::Answered),
                    ty: Type::Fn {
                        params: vec![arg.ty.clone()],
                        latent: Held::NONE,
                        result: Box::new(v.ty.clone()),
                        result_depth: Depth::PURE,
                    },
                    depth: Depth::PURE,
                    span,
                };
                Some(Expr {
                    kind: ExprKind::Call { callee: Box::new(callee), args: vec![arg] },
                    ty: v.ty.clone(),
                    depth: Depth::PURE,
                    span,
                })
            }
            Kind::Refused(r) if v.depth == Depth::PURE => {
                let arg = Expr {
                    kind: ExprKind::Literal(Literal::Refusal(*r)),
                    ty: Type::Refusal,
                    depth: Depth::PURE,
                    span,
                };
                let callee = Expr {
                    kind: ExprKind::Prim(Prim::Refused),
                    ty: Type::Fn {
                        params: vec![Type::Refusal],
                        latent: Held::NONE,
                        result: Box::new(v.ty.clone()),
                        result_depth: Depth::PURE,
                    },
                    depth: Depth::PURE,
                    span,
                };
                Some(Expr {
                    kind: ExprKind::Call { callee: Box::new(callee), args: vec![arg] },
                    ty: v.ty.clone(),
                    depth: Depth::PURE,
                    span,
                })
            }
            _ => None,
        }
    }

    /// The expression a value residualises as.
    ///
    /// A value that can be written down is written down; anything else is the
    /// expression it came from, unreduced. That is why no fold produces an
    /// array or a struct: there is no way to write one, so a residue holding
    /// one could not be a program.
    fn residual(v: Val, original: &Expr) -> Expr {
        let span = original.span;
        if let Some(closed) = Self::closed_residual(&v, span) {
            return closed;
        }
        match v.kind {
            Kind::Shade(inner) => {
                let inner_orig = match &original.kind {
                    ExprKind::Rite { rite: Rite::Shade, operand } => operand,
                    _ => original,
                };
                let operand = Self::residual(*inner, inner_orig);
                // If `inner` could not residualise on its own and fell back to an
                // already-shaded original (e.g. a local binding), avoid double-wrapping.
                if operand.ty == v.ty {
                    return operand;
                }
                // Preserve original expression when an answered deep inner value
                // folded to a pure literal, so `Type::Shade.origin` matches `operand.depth`.
                if let Type::Shade { origin, .. } = v.ty
                    && operand.depth != origin
                {
                    return original.clone();
                }
                Expr {
                    kind: ExprKind::Rite { rite: Rite::Shade, operand: Box::new(operand) },
                    ty: v.ty,
                    depth: Depth::PURE,
                    span,
                }
            }
            // §04 has no syntax for a deep answer, so one that was not consumed
            // is the question it came from and stays a hole for a later
            // burial. That is sound by §6.5: a residue is a program, and this
            // one still asks.
            Kind::Stuck(e) => *e,
            _ => original.clone(),
        }
    }

    /// A residual expression with a new shape, built from reduced parts.
    fn rebuild(kind: ExprKind, x: &Expr, depth: Depth) -> Val {
        let e = Expr { kind, ty: x.ty.clone(), depth, span: x.span };
        Val { kind: Kind::Stuck(Box::new(e)), ty: x.ty.clone(), depth }
    }

    fn expr(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            let kind = HaltKind::TooDeep { limit: MAX_EXPR_DEPTH };
            return Err(Halt { span: x.span, kind, grinding: None });
        }
        self.expr_depth += 1;
        let res = self.expr_inner(x, env);
        self.expr_depth -= 1;
        res
    }

    fn expr_inner(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.burn(x.span)?;
        Ok(match &x.kind {
            ExprKind::Literal(l) => Self::known(l.clone(), x),

            ExprKind::Local(id) => match env.get(id.0 as usize).and_then(Clone::clone) {
                Some(v) if v.is_stuck() => Self::rebuild(ExprKind::Local(*id), x, v.depth),
                Some(v) => v,
                None => Self::stuck(x),
            },

            ExprKind::Global(id) => self.global(*id, x)?,

            ExprKind::Func(id) => Val { kind: Kind::Func(*id), ty: x.ty.clone(), depth: x.depth },
            ExprKind::Prim(p) => Val { kind: Kind::Prim(*p), ty: x.ty.clone(), depth: x.depth },

            ExprKind::Call { callee, args } => self.call(x, callee, args, env)?,

            ExprKind::Unary { op, operand } => self.unary(x, *op, operand, env)?,

            ExprKind::Binary { op, lhs, rhs } => self.binary(x, *op, lhs, rhs, env)?,

            ExprKind::Select { cond, then, otherwise } => {
                self.select(x, cond, then, otherwise, env)?
            }

            ExprKind::Block(b) => self.block(b, env)?,

            ExprKind::Loop { .. } => self.loop_(x, env)?,

            ExprKind::Descend { capability, body } => self.descend(x, *capability, body, env)?,

            ExprKind::Rite { rite, operand } => self.rite(x, *rite, operand, env)?,

            ExprKind::Assign { place, value } => self.assign(x, place, value, env)?,

            ExprKind::Break => {
                self.flow = Some(Flow::Broke);
                Self::unit()
            }
            ExprKind::Continue => {
                self.flow = Some(Flow::Continued);
                Self::unit()
            }
            ExprKind::Return(v) => {
                let out = match v {
                    Some(v) => {
                        let out = self.expr(v, env)?;
                        if self.flow.is_some() {
                            return Ok(out);
                        }
                        out
                    }
                    None => Self::unit(),
                };
                self.flow = Some(Flow::Returned(out.clone()));
                out
            }

            // Neither has a value form to fold into: indexing and projection
            // need an aggregate, and nothing can build one. Settle stale local
            // writes so the unreduced expression reads the latest values.
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.unreduced(x, env),
        })
    }

    fn unary(&mut self, x: &Expr, op: UnOp, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let v = self.expr(operand, env)?;
        if self.flow.is_some() {
            return Ok(v);
        }
        Ok(if let Some(l) = unary(op, &v) {
            Self::known(l, x)
        } else {
            let operand = Box::new(Self::residual(v, operand));
            let depth = operand.depth;
            Self::rebuild(ExprKind::Unary { op, operand }, x, depth)
        })
    }

    fn binary(
        &mut self,
        x: &Expr,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let a = self.expr(lhs, env)?;
        if self.flow.is_some() {
            return Ok(a);
        }
        let lhs_stuck = a.is_stuck();
        let rhs_mark = self.settling.len();
        let rhs_gen = env.writes;
        let b = self.expr(rhs, env)?;
        if self.flow.is_some() {
            return Ok(b);
        }
        let mut rhs_prefix = Vec::new();
        if lhs_stuck {
            rhs_prefix.extend(self.settling.drain(rhs_mark..));
            Self::flush_stale_since(None, rhs_gen, env, &mut rhs_prefix);
        }
        if matches!(op, BinOp::Eq | BinOp::Ne) {
            let cost = match (&a.kind, &b.kind) {
                (Kind::Known(ValLit::Bytes(x)), Kind::Known(ValLit::Bytes(y))) => {
                    x.len().min(y.len())
                }
                (Kind::Known(ValLit::Str(x)), Kind::Known(ValLit::Str(y))) => x.len().min(y.len()),
                _ => Self::val_byte_work(&a).saturating_add(Self::val_byte_work(&b)),
            };
            self.burn_bytes(cost, x.span)?;
        }
        let folded = binary(op, &a, &b).map_err(|why| Halt {
            span: x.span,
            kind: HaltKind::Collapsed(why),
            grinding: None,
        })?;
        Ok(if let Some(l) = folded {
            Self::known(l, x)
        } else {
            let lhs = Box::new(Self::residual(a, lhs));
            let mut rhs_expr = Self::residual(b, rhs);
            if !rhs_prefix.is_empty() {
                let ty = rhs_expr.ty.clone();
                let depth = rhs_expr.depth;
                let span = rhs_expr.span;
                rhs_expr = Expr {
                    kind: ExprKind::Block(Block {
                        stmts: rhs_prefix,
                        tail: Some(Box::new(rhs_expr)),
                        span,
                    }),
                    ty,
                    depth,
                    span,
                };
            }
            let rhs = Box::new(rhs_expr);
            let depth = lhs.depth.join(rhs.depth);
            Self::rebuild(ExprKind::Binary { op, lhs, rhs }, x, depth)
        })
    }

    fn unreduced_select(
        &mut self,
        x: &Expr,
        c: Val,
        cond: &Expr,
        then: &Expr,
        otherwise: &Expr,
        env: &mut Env,
    ) -> Val {
        let cond_res = Self::residual(c, cond);
        let depth = cond_res.depth.join(then.depth).join(otherwise.depth);
        let whole = Expr {
            kind: ExprKind::Select {
                cond: Box::new(cond_res),
                then: Box::new(then.clone()),
                otherwise: Box::new(otherwise.clone()),
            },
            depth,
            ..x.clone()
        };
        self.unreduced(&whole, env)
    }

    /// A branch on something that is not here residualises whole, and both
    /// arms are reduced where they stand. §6.5 makes that reduction
    /// speculative, which is what keeps it inside §6.2: it cannot ask, deposit
    /// or collapse, so it leaves nothing that §6.2 is protecting.
    fn select(
        &mut self,
        x: &Expr,
        cond: &Expr,
        then: &Expr,
        otherwise: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let c = self.expr(cond, env)?;
        if self.flow.is_some() {
            return Ok(c);
        }
        let Some(taken) = c.bool() else {
            let cond = Box::new(Self::residual(c, cond));
            // An arm that writes a local the branch outlives cannot be
            // folded to its value: the write is the part that matters, and
            // which arm makes it is what the world has not said. So both
            // stay as written, and the local is not known after the branch.
            let branch_pc = self.pc.join(cond.depth);
            let (writes, _, _) = Self::write_depths(&[then, otherwise]);
            let arms = if writes.is_empty() {
                let saved_pc = self.pc;
                self.pc = branch_pc;
                let res = (|| Ok([self.speculate(then, env)?, self.speculate(otherwise, env)?]))();
                self.pc = saved_pc;
                res?
            } else {
                [then.clone(), otherwise.clone()]
            };
            // Any residual arm can still read stale locals, so settle against
            // both residual arms before clearing locals they write.
            if Self::reaches_act(&arms[0], self.unit) || Self::reaches_act(&arms[1], self.unit) {
                self.act_blocked = true;
            }
            let depth = arms.iter().fold(cond.depth, |d, arm| d.join(arm.depth));
            let settled = Self::leave(&[&arms[0], &arms[1]], branch_pc, env);
            self.settling.extend(settled);
            let [then, otherwise] = arms.map(Box::new);
            return Ok(Self::rebuild(ExprKind::Select { cond, then, otherwise }, x, depth));
        };
        let (arm, other) = if taken { (then, otherwise) } else { (otherwise, then) };
        let (val_deeper, effects_deeper) = Self::lowers(arm, other, self.pc);
        if !val_deeper && !effects_deeper {
            return self.expr(arm, env);
        }
        // §6.5, a residue keeps its depths: taking the shallower arm
        // would leave a residue shallower than the branch, and a
        // `look` that held the branch's depth may not hold the arm's.
        // A value is exempt, so the arm runs. If it does not finish,
        // the branch stays as written: what the arm deposited is the
        // branch's to deposit again, and the questions it asked, the
        // residue asks too.
        if effects_deeper && other.ty != Type::Unit {
            return Ok(self.unreduced_select(x, c, cond, then, otherwise, env));
        }
        let cp = self.checkpoint();
        let mut ran = env.clone();
        let v = self.expr(arm, &mut ran)?;
        let stays_residual = v.is_stuck()
            || matches!(v.kind, Kind::Answered(_) | Kind::Refused(_))
            || (self.speculating && (self.flow.is_some() || effects_deeper));
        if !stays_residual {
            if effects_deeper && !matches!(self.flow, Some(Flow::Returned(_))) {
                let already_guard = x.ty == Type::Unit
                    && matches!(cond.kind, ExprKind::Literal(Literal::Bool(_)))
                    && matches!(&arm.kind, ExprKind::Block(b) if b.stmts.is_empty() && b.tail.is_none());
                let guard = if already_guard {
                    x.clone()
                } else {
                    Self::dead_branch_guard(taken, other, x.span)
                };
                let guard_stmt = Stmt::Expr(guard.clone());
                if !self.settling.contains(&guard_stmt) {
                    // Settle against `ran` (clearing stale bits) before emitting the
                    // guard so any local named in the dead arm is not assigned after it.
                    let settled = Self::settle(&[&guard], &mut ran);
                    self.settling.extend(settled);
                    self.settling.push(guard_stmt);
                }
            }
            self.starved = cp.starved;
            *env = ran;
            return Ok(v);
        }
        if self.served != cp.served && v.is_stuck() {
            // An act was already served inside `arm`; keep the reduced `arm` in
            // the branch instead of rolling back and duplicating the act.
            let arm_res = Self::residual(v, arm);
            let (new_then, new_other) =
                if taken { (arm_res, other.clone()) } else { (other.clone(), arm_res) };
            let cond_res = Self::residual(c, cond);
            let depth = cond_res.depth.join(new_then.depth).join(new_other.depth);
            *env = ran;
            self.starved = cp.starved;
            return Ok(Self::rebuild(
                ExprKind::Select {
                    cond: Box::new(cond_res),
                    then: Box::new(new_then),
                    otherwise: Box::new(new_other),
                },
                x,
                depth,
            ));
        }
        self.rollback(cp);
        Ok(self.unreduced_select(x, c, cond, then, otherwise, env))
    }

    /// One arm of an undecided branch, reduced as far as speculation allows.
    ///
    /// On a copy of the environment, because an assignment in an arm that may
    /// not run cannot be seen after it. An arm that collapsed, or that reached
    /// a `break` or `continue`, stays as it was written: the first is §6.5's
    /// third clause, and folding the second to its value would drop the jump.
    /// Fuel is charged as for anything else, and running out of it is still a
    /// halt (§6.4).
    fn speculate(&mut self, arm: &Expr, env: &Env) -> Result<Expr, Halt> {
        let was = std::mem::replace(&mut self.speculating, true);
        let cp = self.checkpoint();
        let out = self.try_expr(arm, &mut env.clone());
        self.speculating = was;
        let out = match out {
            Ok(v) => v,
            Err(halt) => {
                self.rollback(cp);
                return Err(halt);
            }
        };
        // Assignments an arm's own code needs, with no block in the arm to put
        // them in, would land in front of the whole branch.
        if self.settling.len() > cp.settling {
            self.rollback(cp);
            return Ok(arm.clone());
        }
        let flow = self.flow.take();
        if let Some(Flow::Returned(r)) = flow {
            if matches!(&r.kind, Kind::Stuck(e) if Self::ends_returning(e)) {
                self.starved = cp.starved;
                self.flow = cp.flow;
                return Ok(Self::residual(r, arm));
            }
            if arm.ty == Type::Unit
                && r.ty != Type::Unit
                && let Some(val) = Self::closed_residual(&r, arm.span)
            {
                self.starved = cp.starved;
                self.flow = cp.flow;
                if let ExprKind::Block(b) = &arm.kind
                    && b.tail.is_none()
                    && let [Stmt::Expr(Expr { kind: ExprKind::Return(Some(inner)), .. })] =
                        b.stmts.as_slice()
                    && inner.kind == val.kind
                {
                    return Ok(arm.clone());
                }
                let depth = val.depth;
                let ret = Expr {
                    kind: ExprKind::Return(Some(Box::new(val))),
                    ty: Type::Unit,
                    depth,
                    span: arm.span,
                };
                return Ok(match &arm.kind {
                    ExprKind::Block(b) => Expr {
                        kind: ExprKind::Block(Block {
                            stmts: vec![Stmt::Expr(ret)],
                            tail: None,
                            span: b.span,
                        }),
                        ty: Type::Unit,
                        depth: Depth::PURE,
                        span: arm.span,
                    },
                    _ => ret,
                });
            }
            self.rollback(cp);
            return Ok(arm.clone());
        }
        self.starved = cp.starved;
        self.flow = cp.flow;
        Ok(out.map_or_else(|| arm.clone(), |v| Self::residual(v, arm)))
    }

    /// Evaluate `x` speculatively, rolling back what it changed if it collapses
    /// or jumps out of a loop.
    fn try_expr(&mut self, x: &Expr, env: &mut Env) -> Result<Option<Val>, Halt> {
        let (grind, before) = (self.grind.len(), env.clone());
        let cp = self.checkpoint();
        let out = self.expr(x, env);
        match out {
            Ok(v) if !matches!(self.flow, Some(Flow::Broke | Flow::Continued)) => {
                self.starved = cp.starved;
                Ok(Some(v))
            }
            Ok(_) => {
                *env = before;
                self.rollback(cp);
                Ok(None)
            }
            Err(Halt { kind: HaltKind::Collapsed(_), .. }) => {
                self.grind.truncate(grind);
                *env = before;
                self.rollback(cp);
                Ok(None)
            }
            Err(halt) => {
                self.starved = cp.starved;
                Err(halt)
            }
        }
    }

    fn wrap_settling_descend(&mut self, mark: usize, capability: Capability, span: Span) {
        if self.settling.len() <= mark {
            return;
        }
        let inner: Vec<Stmt> = self.settling.drain(mark..).collect();
        let block = Expr {
            kind: ExprKind::Block(Block { stmts: inner, tail: None, span }),
            ty: Type::Unit,
            depth: Depth::PURE,
            span,
        };
        self.settling.push(Stmt::Expr(Expr {
            kind: ExprKind::Descend { capability, body: Box::new(block) },
            ty: Type::Unit,
            depth: Depth::PURE,
            span,
        }));
    }

    /// A descent is a scope marker. Evaluating it evaluates its body; what the
    /// body could not do, the descent still cannot.
    fn descend(
        &mut self,
        x: &Expr,
        capability: Capability,
        body: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let mark = self.settling.len();
        let v = self.expr(body, env)?;
        self.wrap_settling_descend(mark, capability, x.span);
        if let Some(Flow::Returned(r)) = &self.flow {
            if r.is_stuck() || (self.speculating && Self::closed_residual(r, body.span).is_none()) {
                let body = Box::new(Self::residual(r.clone(), body));
                let body_ty = body.ty.clone();
                let depth = body.depth;
                let e = Expr {
                    kind: ExprKind::Descend { capability, body },
                    ty: body_ty.clone(),
                    depth,
                    span: x.span,
                };
                let rebuilt = Val { kind: Kind::Stuck(Box::new(e)), ty: body_ty, depth };
                self.flow = Some(Flow::Returned(rebuilt.clone()));
                return Ok(rebuilt);
            }
            return Ok(v);
        }
        if self.flow.is_some() || !v.is_stuck() {
            return Ok(v);
        }
        let body = Box::new(Self::residual(v, body));
        let depth = body.depth;
        Ok(Self::rebuild(ExprKind::Descend { capability, body }, x, depth))
    }

    /// A unit-level binding, evaluated once and only if something reaches it.
    fn global(&mut self, id: GlobalId, x: &Expr) -> Result<Val, Halt> {
        self.shares()?;
        if let Some(v) = self.globals.get(id.0 as usize).and_then(Clone::clone) {
            return Ok(v);
        }
        let Some(g) = self.unit.global(id) else {
            return Ok(Self::stuck(x));
        };
        if self.eval_globals.contains(&id) {
            return Err(Halt {
                span: x.span,
                kind: HaltKind::Collapsed("unit-level binding refers to itself"),
                grinding: None,
            });
        }
        self.eval_globals.push(id);
        let cp = self.checkpoint();
        self.flow = None;
        self.pc = Depth::PURE;
        // Unit-level code has a frame of its own, as a function does. 0280.
        let res = self.expr(&g.value, &mut Env::new(self.unit.locals.len()));
        self.eval_globals.pop();
        let v = match res {
            Ok(v) => v,
            Err(halt) => {
                self.rollback(cp);
                return Err(halt);
            }
        };
        let had_effects = self.deposits.len() > cp.deposits || self.served != cp.served;
        let keep_ref = !self.unit.locals.is_empty()
            || had_effects
            || Self::reaches_act(&g.value, self.unit)
            || Self::reaches_deposit(&g.value, self.unit);
        let v = if v.is_stuck() {
            if keep_ref {
                if had_effects && !self.speculating {
                    let reduced = Self::residual(v, &g.value);
                    if let Some(slot) = self.globals_residual.get_mut(id.0 as usize) {
                        *slot = Some(reduced);
                    }
                    self.starved = cp.starved;
                    self.settling.truncate(cp.settling);
                    self.flow = cp.flow;
                    self.pc = cp.pc;
                } else {
                    self.rollback(cp);
                }
                Self::stuck(x)
            } else {
                self.starved = cp.starved;
                self.settling.truncate(cp.settling);
                self.flow = cp.flow;
                self.pc = cp.pc;
                v
            }
        } else if let Some(closed) = Self::closed_residual(&v, g.span) {
            if !self.speculating
                && let Some(slot) = self.globals_residual.get_mut(id.0 as usize)
            {
                *slot = Some(closed);
            }
            self.starved = cp.starved;
            self.settling.truncate(cp.settling);
            self.flow = cp.flow;
            self.pc = cp.pc;
            v
        } else if had_effects {
            if self.served != cp.served && !self.speculating {
                let reduced = Self::residual(v, &g.value);
                if let Some(slot) = self.globals_residual.get_mut(id.0 as usize) {
                    *slot = Some(reduced);
                }
                self.starved = cp.starved;
                self.settling.truncate(cp.settling);
                self.flow = cp.flow;
                self.pc = cp.pc;
            } else {
                self.rollback(cp);
            }
            Self::stuck(x)
        } else {
            self.starved = cp.starved;
            self.settling.truncate(cp.settling);
            self.flow = cp.flow;
            self.pc = cp.pc;
            v
        };
        // Not kept while speculating: a binding that starved because an arm
        // may not ask would otherwise stay starved for the code that may.
        if !self.speculating
            && let Some(slot) = self.globals.get_mut(id.0 as usize)
        {
            *slot = Some(v.clone());
        }
        Ok(v)
    }

    fn wrap_prefix(prefix: Vec<Stmt>, res: Expr) -> Expr {
        if prefix.is_empty() {
            return res;
        }
        let (ty, depth, span) = (res.ty.clone(), res.depth, res.span);
        Expr {
            kind: ExprKind::Block(Block { stmts: prefix, tail: Some(Box::new(res)), span }),
            ty,
            depth,
            span,
        }
    }

    fn call_world_prim(&mut self, p: Prim, values: &[Val], x: &Expr) -> Result<Option<Val>, Halt> {
        let shared = p.latent().reads();
        if p.latent() <= Depth::PURE {
            return Ok(None);
        }
        if self.speculating {
            if !shared {
                self.act_blocked = true;
            }
            return Ok(None);
        }
        if !shared && self.act_blocked {
            return Ok(None);
        }
        self.shares()?;
        let Some(args) = values.iter().map(as_value).collect::<Option<Vec<_>>>() else {
            if !shared {
                self.act_blocked = true;
            }
            return Ok(None);
        };
        let arg_bytes: usize =
            values.iter().map(Self::val_byte_work).fold(0usize, usize::saturating_add);
        self.burn_bytes(arg_bytes, x.span)?;
        let asked = nether_ledger::Call {
            function: p.name().to_string(),
            args: args.into_iter().map(|v| self.remember(Stored::Value(v))).collect(),
        };
        let nth = if shared {
            0
        } else {
            self.served.get(&asked).copied().unwrap_or(0)
                + self.reserved_acts.get(&asked).copied().unwrap_or(0)
        };
        if let Some(said) = self.answers.get(&asked, nth).cloned() {
            if let Some(ans) = Self::answered(&said, x) {
                if !shared {
                    *self.served.entry(asked).or_default() += 1;
                }
                self.remember(Stored::Value(said));
                return Ok(Some(ans));
            }
        } else {
            self.dig(p, asked, x.span);
        }
        Ok(None)
    }

    /// [APP]. A call to a function every one of whose arguments is here is
    /// evaluated; anything else residualises as the call it was.
    fn call(&mut self, x: &Expr, callee: &Expr, args: &[Expr], env: &mut Env) -> Result<Val, Halt> {
        let f = self.expr(callee, env)?;
        if self.flow.is_some() {
            return Ok(f);
        }
        let mut any_stuck = f.is_stuck();
        let mut values = Vec::with_capacity(args.len());
        let mut arg_prefixes: Vec<Vec<Stmt>> = Vec::with_capacity(args.len());
        for a in args {
            let mark = self.settling.len();
            let arg_gen = env.writes;
            let v = self.expr(a, env)?;
            if self.flow.is_some() {
                return Ok(v);
            }
            let mut prefix = Vec::new();
            if any_stuck {
                prefix.extend(self.settling.drain(mark..));
                Self::flush_stale_since(None, arg_gen, env, &mut prefix);
            }
            any_stuck |= v.is_stuck();
            values.push(v);
            arg_prefixes.push(prefix);
        }

        if let Kind::Prim(p) = f.kind {
            if let Some(ans) = self.call_world_prim(p, &values, x)? {
                return Ok(ans);
            }
            if let Some(v) = self.inspect(p, &values, x)? {
                return Ok(v);
            }
        }

        let folded = match &f.kind {
            Kind::Prim(p) => {
                let byte_work = prim_byte_work(*p, &values);
                self.burn_bytes(byte_work, x.span)?;
                match prim(*p, &values) {
                    Ok(l) => l.map(|l| Self::known(l, x)),
                    Err(PrimErr::Collapsed(why)) => {
                        let kind = HaltKind::Collapsed(why);
                        return Err(Halt { span: x.span, kind, grinding: None });
                    }
                    Err(PrimErr::TooLarge(limit)) => {
                        let kind = HaltKind::TooLarge { limit };
                        return Err(Halt { span: x.span, kind, grinding: None });
                    }
                }
            }
            Kind::Func(id) => self.apply(*id, &values, args, x)?,
            _ => None,
        };
        if let Some(v) = folded {
            return Ok(v);
        }

        let callee = Box::new(Self::residual(f, callee));
        let result_depth = match callee.ty {
            Type::Fn { result_depth, .. } => result_depth,
            _ => Depth::PURE,
        };
        let args: Vec<Expr> = values
            .into_iter()
            .zip(args)
            .zip(arg_prefixes)
            .map(|((v, a), prefix)| Self::wrap_prefix(prefix, Self::residual(v, a)))
            .collect();
        let depth = args.iter().fold(result_depth.join(callee.depth), |acc, a| acc.join(a.depth));
        Ok(Self::rebuild(ExprKind::Call { callee, args }, x, depth))
    }

    /// Evaluate a function body against known arguments.
    ///
    /// A body that finishes gives the call its value. A body that starves
    /// gives it the reduced body instead, minted as a function of no
    /// parameters — §6.5. The call site's argument expressions come along so
    /// that the arguments can be written down as the bindings the reduced body
    /// still names.
    fn apply(
        &mut self,
        id: FuncId,
        args: &[Val],
        exprs: &[Expr],
        site: &Expr,
    ) -> Result<Option<Val>, Halt> {
        let Some(f) = self.unit.func(id) else { return Ok(None) };
        if args.iter().any(Val::is_stuck) || args.len() != f.params.len() {
            if Self::reaches_act(site, self.unit) {
                self.act_blocked = true;
            }
            return Ok(None);
        }

        let mut frame = Env::new(f.locals.len());
        for (slot, v) in f.params.iter().zip(args) {
            frame.bind(slot.0 as usize, Some(v.clone()));
        }
        let frames = self.grind.iter().filter(|g| matches!(g.what, Open::Call(_))).count();
        if u32::try_from(frames).is_ok_and(|n| n >= MAX_FRAMES) {
            let kind = HaltKind::TooDeep { limit: MAX_FRAMES };
            return Err(Halt { span: f.span, kind, grinding: None });
        }
        self.grind.push(Grind { span: f.span, what: Open::Call(id) });
        // Whatever block the caller was in is not this one. It goes back
        // afterwards: the caller's own statement is still being evaluated.
        let cp = self.checkpoint();
        self.pc = Depth::PURE;
        let fell = self.block(&f.body, &mut frame)?;
        self.grind.pop();
        let out = match self.flow.take() {
            // A `break` outside a loop is not a program this can run.
            Some(Flow::Broke | Flow::Continued) => {
                self.rollback(cp);
                return Ok(None);
            }
            Some(Flow::Returned(v)) => v,
            None => fell,
        };
        let body = self.starved.take();
        if !out.is_stuck() {
            self.starved = cp.starved;
            self.settling.truncate(cp.settling);
            self.flow = cp.flow;
            self.pc = cp.pc;
            return Ok(Some(out));
        }
        // Nothing reduced to show for it: the body starved somewhere no block
        // could be built for, so the residue would be the body as written and
        // the call is the shorter way to say that.
        let Some(body) = body else {
            self.rollback(cp);
            return Ok(None);
        };
        // §6.5, a residue keeps its depths: a body that returns less deeply
        // than its function would leave a residue shallower than the call. The
        // call stays as written, so what the body deposited is the call's to
        // deposit again; the questions it asked, the residue asks too.
        if Self::ret_depth(&body) < site.depth {
            self.rollback(cp);
            return Ok(None);
        }
        let Some(spec) = self.specialise(id, f, args, exprs, site, body) else {
            self.rollback(cp);
            return Ok(None);
        };
        self.starved = cp.starved;
        self.flow = cp.flow;
        self.pc = cp.pc;
        Ok(Some(spec))
    }

    /// §6.5: what a starved call residualises as.
    ///
    /// Every argument is a value — [`Burial::apply`] gives up otherwise — so
    /// the reduced body takes no parameters and the arguments become the first
    /// of its bindings. It keeps the locals table it had, which is why nothing
    /// is renamed: the body is still in the frame it was written for.
    fn specialise(
        &mut self,
        id: FuncId,
        f: &FuncDef,
        args: &[Val],
        exprs: &[Expr],
        site: &Expr,
        body: Block,
    ) -> Option<Val> {
        let mut used_in_body = HashSet::new();
        Self::reads_block(&body, &mut used_in_body);
        let mut stmts: Vec<Stmt> = Vec::with_capacity(f.params.len() + body.stmts.len());
        for ((slot, v), e) in f.params.iter().zip(args).zip(exprs) {
            if !used_in_body.contains(&slot.0) {
                continue;
            }
            // Only splice closed, zero-depth expressions into the callee frame;
            // falling back to caller-scope `e` would leak caller locals, ambient
            // capabilities, or answered calls into the minted function.
            let val_expr = Self::closed_residual(v, e.span)?;
            stmts.push(Stmt::Let { local: *slot, value: val_expr });
        }
        stmts.extend(body.stmts);
        let mut body = Block { stmts, tail: body.tail, span: body.span };
        Self::prune(&mut body);
        // Nothing was reduced, so there is nothing to specialise to, and
        // minting a function here would mint another one for it next time.
        // §6.5's staging law with nothing granted is what that would break:
        // a residue has to be what burying it again leaves.
        if body == f.body {
            return None;
        }
        let depth = Self::ret_depth(&body);
        let latent = Self::latent_of_block(&body, Held::NONE);

        // A worker stops before minting; what it returns is never used.
        self.shares().ok()?;
        let key = Self::constants(args).map(|k| (id, k));
        let base = self.unit.funcs.len();
        let found = key.as_ref().and_then(|k| self.minted.get(k)).copied().filter(|m| {
            self.specialised
                .get((m.0 as usize).saturating_sub(base))
                .is_some_and(|s| s.body == body && s.latent == latent)
        });
        let minted = if let Some(minted) = found {
            minted
        } else {
            let minted = FuncId(u32::try_from(base + self.specialised.len()).unwrap_or(u32::MAX));
            if let Some(k) = key {
                self.minted.entry(k).or_insert(minted);
            }
            let name = self.apart(&f.name, minted);
            let locals =
                f.locals.iter().map(|l| LocalDef { asserted: None, ..l.clone() }).collect();
            self.specialised.push(FuncDef {
                name,
                params: Vec::new(),
                ret: f.ret.clone(),
                ret_depth: depth,
                asserted_ret: None,
                latent,
                asserted_latent: None,
                locals,
                body,
                span: f.span,
            });
            minted
        };
        let ty = Type::Fn {
            params: Vec::new(),
            latent,
            result: Box::new(f.ret.clone()),
            result_depth: depth,
        };
        let callee = Expr { kind: ExprKind::Func(minted), ty, depth: Depth::PURE, span: site.span };
        let kind = ExprKind::Call { callee: Box::new(callee), args: Vec::new() };
        Some(Self::rebuild(kind, site, depth))
    }

    /// Every local read anywhere inside `b`.
    fn reads_block(b: &Block, into: &mut HashSet<u32>) {
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } | Stmt::Expr(value) => Self::reads(value, into),
                Stmt::Declare { .. } => {}
            }
        }
        if let Some(t) = &b.tail {
            Self::reads(t, into);
        }
    }

    /// Capabilities a block reaches outside any enclosing `descend`, matching `check.rs`.
    fn latent_of_block(b: &Block, ambient: Held) -> Held {
        let mut needs = Held::NONE;
        for s in &b.stmts {
            match s {
                Stmt::Let { value, .. } | Stmt::Expr(value) => {
                    needs = needs.union(Self::latent_of_expr(value, ambient));
                }
                Stmt::Declare { .. } => {}
            }
        }
        if let Some(t) = &b.tail {
            needs = needs.union(Self::latent_of_expr(t, ambient));
        }
        needs
    }

    fn latent_of_expr(x: &Expr, ambient: Held) -> Held {
        let mut todo = vec![(x, ambient)];
        let mut needs = Held::NONE;
        while let Some((cur, amb)) = todo.pop() {
            match &cur.kind {
                ExprKind::Call { callee, args } => {
                    if let Type::Fn { latent, .. } = callee.ty {
                        needs = needs.union(latent.without(amb));
                    }
                    todo.push((callee, amb));
                    for a in args {
                        todo.push((a, amb));
                    }
                }
                ExprKind::Descend { capability, body } => {
                    todo.push((body, amb.with(capability.stratum())));
                }
                _ => {
                    for child in children(cur) {
                        todo.push((child, amb));
                    }
                }
            }
        }
        needs
    }

    /// A name for a minted function that collides with no function, global,
    /// local, struct, or prelude binding in the residue.
    fn apart(&self, from: &str, minted: FuncId) -> String {
        let mut name = format!("{from}__{}", minted.0);
        while self
            .unit
            .funcs
            .iter()
            .any(|f| f.name == name || f.locals.iter().any(|l| l.name == name))
            || self
                .specialised
                .iter()
                .any(|f| f.name == name || f.locals.iter().any(|l| l.name == name))
            || self.unit.globals.iter().any(|g| g.name == name)
            || self.unit.locals.iter().any(|l| l.name == name)
            || self.unit.structs.iter().any(|s| s.name == name)
            || Prim::from_name(&name).is_some()
        {
            name.push('_');
        }
        name
    }

    /// The constants a body was specialised to, when every argument is one.
    ///
    /// Only these are memoised. An argument with no literal form — an answer,
    /// a shade — is not one this can compare, and minting a second function
    /// for it makes the residue larger rather than wrong.
    fn constants(args: &[Val]) -> Option<Vec<Literal>> {
        args.iter()
            .map(|v| match &v.kind {
                Kind::Known(l) => Some(l.to_literal()),
                _ => None,
            })
            .collect()
    }

    /// Drop the bindings the reduced body does not read.
    ///
    /// Only the ones bound to something already written down. A binding whose
    /// value still has a question in it is kept whatever names it: dropping it
    /// would drop the question with it, and the residue would stop asking
    /// something the program asks.
    fn prune(body: &mut Block) {
        let mut read: HashSet<u32> = HashSet::new();
        if let Some(t) = &body.tail {
            Self::reads(t, &mut read);
        }
        let mut kept: Vec<Stmt> = Vec::new();
        for s in body.stmts.iter().rev() {
            if let Stmt::Let { local, value } = s
                && !read.contains(&local.0)
                && matches!(value.kind, ExprKind::Literal(_))
            {
                continue;
            }
            match s {
                Stmt::Let { value, .. } | Stmt::Expr(value) => Self::reads(value, &mut read),
                Stmt::Declare { .. } => {}
            }
            kept.push(s.clone());
        }
        kept.reverse();
        body.stmts = kept;
    }

    /// Build a unit-typed dead-branch guard `if (taken) {} else { <other>; }`
    /// (or vice versa) that preserves `other`'s outer local write depths and exit
    /// `pc` in the checker without running `other` or depositing a value.
    fn dead_branch_guard(taken: bool, other: &Expr, span: Span) -> Expr {
        let empty = Expr {
            kind: ExprKind::Block(Block { stmts: Vec::new(), tail: None, span }),
            ty: Type::Unit,
            depth: Depth::PURE,
            span,
        };
        let dead = if other.ty == Type::Unit {
            other.clone()
        } else if let ExprKind::Block(b) = &other.kind {
            let mut stmts = b.stmts.clone();
            if let Some(t) = &b.tail
                && !matches!(t.kind, ExprKind::Local(_) | ExprKind::Literal(_))
            {
                stmts.push(Stmt::Expr((**t).clone()));
            }
            Expr {
                kind: ExprKind::Block(Block { stmts, tail: None, span: b.span }),
                ty: Type::Unit,
                depth: Depth::PURE,
                span: other.span,
            }
        } else {
            Expr {
                kind: ExprKind::Block(Block {
                    stmts: vec![Stmt::Expr(other.clone())],
                    tail: None,
                    span: other.span,
                }),
                ty: Type::Unit,
                depth: Depth::PURE,
                span: other.span,
            }
        };
        let cond = Expr {
            kind: ExprKind::Literal(Literal::Bool(taken)),
            ty: Type::Bool,
            depth: Depth::PURE,
            span,
        };
        let depth = dead.depth;
        let (then, otherwise) = if taken { (empty, dead) } else { (dead, empty) };
        Expr {
            kind: ExprKind::Select {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            ty: Type::Unit,
            depth,
            span,
        }
    }

    /// Whether dropping `other` for `arm` lowers a depth under ambient `pc`:
    /// returns `(value_deeper, effects_deeper)` where `effects_deeper` holds
    /// when `other` writes an outer local more deeply than `arm` or exits at a
    /// deeper control-dependence depth than `arm`. §6.5.
    fn lowers(arm: &Expr, other: &Expr, pc: Depth) -> (bool, bool) {
        let (taken, taken_l, taken_f) = Self::write_depths(&[arm]);
        let (dropped, dropped_l, dropped_f) = Self::write_depths(&[other]);
        let exit_higher = |d: Option<Depth>, t: Option<Depth>| {
            d.is_some_and(|dd| dd.join(pc) > t.map_or(Depth::PURE, |td| td.join(pc)))
        };
        let effects = exit_higher(dropped_l, taken_l)
            || exit_higher(dropped_f, taken_f)
            || dropped.iter().any(|(id, &dd)| {
                let eff = dd.join(pc);
                taken.get(id).map_or(eff > Depth::PURE, |&td| eff > td.join(pc))
            });
        (other.depth > arm.depth, effects)
    }

    /// Deepest write depth (including control-dependence `pc`) for every local
    /// `code` assigns and does not bind itself, plus `(loop_exit_pc, fn_exit_pc)`.
    fn write_depths(code: &[&Expr]) -> (HashMap<u32, Depth>, Option<Depth>, Option<Depth>) {
        let (mut written, mut bound) = (HashMap::new(), BTreeSet::new());
        let (mut la, mut fa) = (None, None);
        let mut steps = 0usize;
        for expr in code {
            let (lp, fp) =
                Self::walk_writes(expr, Depth::PURE, 0, &mut steps, &mut written, &mut bound);
            la = join_opt_depth(la, lp);
            fa = join_opt_depth(fa, fp);
        }
        for id in bound {
            written.remove(&id);
        }
        (written, la, fa)
    }

    fn walk_writes(
        expr: &Expr,
        pc: Depth,
        depth: u32,
        steps: &mut usize,
        written: &mut HashMap<u32, Depth>,
        bound: &mut BTreeSet<u32>,
    ) -> (Option<Depth>, Option<Depth>) {
        const MAX_WALK: u32 = if cfg!(target_family = "wasm") { 1024 } else { 2048 };
        if depth >= MAX_WALK || *steps >= MAX_FIXPOINT_STEPS {
            return (Some(Depth::UNRECORDED), Some(Depth::UNRECORDED));
        }
        let next_d = depth + 1;
        let walk_seq = |items: Vec<&Expr>,
                        mut cur: Depth,
                        steps: &mut usize,
                        written: &mut HashMap<u32, Depth>,
                        bound: &mut BTreeSet<u32>| {
            items.into_iter().fold((None, None), |(la, fa), child| {
                let (lp, fp) = Self::walk_writes(child, cur, next_d, steps, written, bound);
                if let Some(ed) = join_opt_depth(lp, fp) {
                    cur = cur.join(ed);
                }
                (join_opt_depth(la, lp), join_opt_depth(fa, fp))
            })
        };
        match &expr.kind {
            ExprKind::Select { cond, then, otherwise } => {
                let (l0, f0) = Self::walk_writes(cond, pc, next_d, steps, written, bound);
                let bpc = join_opt_depth(l0, f0).map_or(pc, |ed| pc.join(ed)).join(cond.depth);
                let (l1, f1) = Self::walk_writes(then, bpc, next_d, steps, written, bound);
                let (l2, f2) = Self::walk_writes(otherwise, bpc, next_d, steps, written, bound);
                (
                    join_opt_depth(join_opt_depth(l0, l1), l2),
                    join_opt_depth(join_opt_depth(f0, f1), f2),
                )
            }
            ExprKind::Block(blk) => {
                let mut exprs = Vec::with_capacity(blk.stmts.len() + 1);
                for stmt in &blk.stmts {
                    match stmt {
                        Stmt::Declare { local } => {
                            bound.insert(local.0);
                        }
                        Stmt::Let { local, value } => {
                            bound.insert(local.0);
                            exprs.push(value);
                        }
                        Stmt::Expr(inner) => exprs.push(inner),
                    }
                }
                if let Some(tail) = &blk.tail {
                    exprs.push(tail);
                }
                walk_seq(exprs, pc, steps, written, bound)
            }
            ExprKind::Loop { body, step } => {
                let mut lpc = pc;
                loop {
                    *steps += 1;
                    if *steps > MAX_FIXPOINT_STEPS {
                        return (None, Some(Depth::UNRECORDED));
                    }
                    let (l1, f1) = Self::walk_writes(body, lpc, next_d, steps, written, bound);
                    let spc = join_opt_depth(l1, f1).map_or(lpc, |dep| lpc.join(dep));
                    let (l2, f2) = step.as_deref().map_or((None, None), |s| {
                        Self::walk_writes(s, spc, next_d, steps, written, bound)
                    });
                    let fn_ex = join_opt_depth(f1, f2);
                    let next = join_opt_depth(join_opt_depth(l1, l2), fn_ex)
                        .map_or(lpc, |dep| lpc.join(dep));
                    if next == lpc {
                        return (None, fn_ex);
                    }
                    lpc = next;
                }
            }
            ExprKind::Break | ExprKind::Continue => (Some(pc), None),
            ExprKind::Return(val) => {
                let (lp, fp) = val.as_deref().map_or((None, None), |inner| {
                    Self::walk_writes(inner, pc, next_d, steps, written, bound)
                });
                let ret_pc = join_opt_depth(lp, fp).map_or(pc, |dep| pc.join(dep));
                (lp, Some(ret_pc))
            }
            ExprKind::Assign { place, .. } => {
                let exits = walk_seq(children(expr), pc, steps, written, bound);
                let cur = join_opt_depth(exits.0, exits.1).map_or(pc, |ed| pc.join(ed));
                let wdepth = expr.depth.join(cur);
                written
                    .entry(place.local.0)
                    .and_modify(|was| *was = was.join(wdepth))
                    .or_insert(wdepth);
                exits
            }
            _ => walk_seq(children(expr), pc, steps, written, bound),
        }
    }

    /// Every local a `let` or a declaration anywhere in `x` binds.
    fn binds(x: &Expr, into: &mut BTreeSet<u32>) {
        let mut todo = vec![x];
        while let Some(x) = todo.pop() {
            if let ExprKind::Block(b) = &x.kind {
                into.extend(b.stmts.iter().filter_map(|s| match s {
                    Stmt::Let { local, .. } | Stmt::Declare { local } => Some(local.0),
                    Stmt::Expr(_) => None,
                }));
            }
            todo.extend(children(x));
        }
    }

    fn flush_stale_since(
        exclude: Option<&BTreeSet<u32>>,
        since_gen: u64,
        env: &mut Env,
        out: &mut Vec<Stmt>,
    ) {
        if env.writes == since_gen {
            return;
        }
        for idx in 0..env.slots.len() {
            let id = u32::try_from(idx).unwrap_or(u32::MAX);
            if exclude.is_some_and(|ex| ex.contains(&id)) {
                continue;
            }
            if !env.stale[idx] || env.write_gen[idx] <= since_gen {
                continue;
            }
            let Some(v) = env.slots[idx].clone() else { continue };
            if !matches!(v.kind, Kind::Known(_) | Kind::Func(_) | Kind::Prim(_) | Kind::Shade(_)) {
                continue;
            }
            env.stale[idx] = false;
            let at = Expr {
                kind: ExprKind::Local(LocalId(id)),
                ty: v.ty.clone(),
                depth: v.depth,
                span: Span::default(),
            };
            let value = Self::residual(v, &at);
            let depth = value.depth;
            let place = Place { local: LocalId(id), path: Vec::new() };
            let kind = ExprKind::Assign { place, value: Box::new(value) };
            out.push(Stmt::Expr(Expr { kind, ty: Type::Unit, depth, span: Span::default() }));
        }
    }

    fn block_bound(b: &Block) -> BTreeSet<u32> {
        b.stmts
            .iter()
            .filter_map(|s| match s {
                Stmt::Let { local, .. } | Stmt::Declare { local } => Some(local.0),
                Stmt::Expr(_) => None,
            })
            .collect()
    }

    /// What code left as written needs before it in the residue.
    ///
    /// The burial reduced the statements above it, and a statement that only
    /// assigned is not in the residue: its value is in `env` and nowhere else.
    /// Code left as written reads its locals in the residue, so each one it
    /// names that has a value here is assigned that value first. A local the
    /// code reads but never wrote cannot be named yet by anything in the
    /// residue, because a named local is read as itself, and one it writes was
    /// not named before the write by §5.4. Either way the assignment is legal.
    fn settle(code: &[&Expr], env: &mut Env) -> Vec<Stmt> {
        let (mut read, mut bound) = (HashSet::new(), BTreeSet::new());
        for x in code {
            Self::reads(x, &mut read);
            Self::binds(x, &mut bound);
        }
        let names: BTreeSet<u32> = read
            .into_iter()
            .filter(|&id| !bound.contains(&id) && env.stale.get(id as usize) == Some(&true))
            .collect();
        names
            .into_iter()
            .filter_map(|id| {
                let v = env.get(id as usize).cloned().flatten()?;
                if !matches!(
                    v.kind,
                    Kind::Known(_) | Kind::Func(_) | Kind::Prim(_) | Kind::Shade(_)
                ) {
                    return None;
                }
                env.bind(id as usize, Some(v.clone()));
                let at = Expr {
                    kind: ExprKind::Local(LocalId(id)),
                    ty: v.ty.clone(),
                    depth: v.depth,
                    span: Span::default(),
                };
                let value = Self::residual(v, &at);
                let depth = value.depth;
                let place = Place { local: LocalId(id), path: Vec::new() };
                let kind = ExprKind::Assign { place, value: Box::new(value) };
                Some(Stmt::Expr(Expr { kind, ty: Type::Unit, depth, span: Span::default() }))
            })
            .collect()
    }

    /// What code left as written needs before it, as [`Burial::settle`] says.
    /// After it, nothing here knows what the locals it writes hold, so a read
    /// of one is the local itself.
    fn leave(code: &[&Expr], depth: Depth, env: &mut Env) -> Vec<Stmt> {
        let settled = Self::settle(code, env);
        let (writes, _, _) = Self::write_depths(code);
        for (local, written) in writes {
            let id = local as usize;
            if let Some(Some(v)) = env.get(id) {
                let (ty, depth) = (v.ty.clone(), v.depth.join(depth).join(written));
                let at = Expr {
                    kind: ExprKind::Local(LocalId(local)),
                    ty,
                    depth,
                    span: Span::default(),
                };
                env.bind(id, Some(Self::rebuild(at.kind.clone(), &at, depth)));
            }
        }
        settled
    }

    /// `x` left as written: after the assignments it needs, and with what it
    /// writes no longer known here.
    fn unreduced(&mut self, x: &Expr, env: &mut Env) -> Val {
        let settled = Self::leave(&[x], self.pc, env);
        self.settling.extend(settled);
        Self::stuck(x)
    }

    /// Every local an expression names, including inside `Assign` index paths.
    fn reads(x: &Expr, into: &mut HashSet<u32>) {
        let mut todo = vec![x];
        while let Some(cur) = todo.pop() {
            match &cur.kind {
                ExprKind::Local(id) => {
                    into.insert(id.0);
                }
                ExprKind::Assign { place, .. } => {
                    into.insert(place.local.0);
                }
                _ => {}
            }
            todo.extend(children(cur));
        }
    }

    /// [ABS]: the return depth of a reduced body is the join of its tail, every
    /// `return` value, and the control-dependence `pc` at every `return`.
    fn ret_depth(b: &Block) -> Depth {
        let blk = Expr {
            kind: ExprKind::Block(b.clone()),
            ty: Type::Unit,
            depth: Depth::PURE,
            span: b.span,
        };
        let (_, _, fn_exit) = Self::write_depths(&[&blk]);
        let tail = b.tail.as_ref().map_or(Depth::PURE, |t| t.depth);
        tail.join(Self::returns_in(b)).join(fn_exit.unwrap_or(Depth::PURE))
    }

    fn returns_in(b: &Block) -> Depth {
        let tail = b.tail.as_deref().map_or(Depth::PURE, Self::returns);
        b.stmts.iter().fold(tail, |d, s| match s {
            Stmt::Let { value: x, .. } | Stmt::Expr(x) => d.join(Self::returns(x)),
            Stmt::Declare { .. } => d,
        })
    }

    fn returns(x: &Expr) -> Depth {
        let mut todo = vec![x];
        let mut acc = Depth::PURE;
        while let Some(cur) = todo.pop() {
            if let ExprKind::Return(Some(v)) = &cur.kind {
                acc = acc.join(v.depth);
            }
            todo.extend(children(cur));
        }
        acc
    }

    /// §6.5: a block reduces its statements in order. Once a statement starves,
    /// the statements and tail after it are reduced speculatively in place,
    /// with any starved binding kept as an unknown local.
    ///
    /// The residue goes into [`Burial::starved`] rather than coming back as a
    /// value, because a block's value is what starved in it and its residue is
    /// the statements around that.
    /// Residualise `v`, settling stale locals into `kept` when `closed_residual`
    /// falls back to `original`.
    fn residual_settled(v: Val, original: &Expr, kept: &mut Vec<Stmt>, env: &mut Env) -> Expr {
        Self::closed_residual(&v, original.span).unwrap_or_else(|| {
            let r = Self::residual(v, original);
            kept.extend(Self::settle(&[&r], env));
            r
        })
    }

    /// Build a residual block ending in `return r`, settling any stale locals
    /// referenced by the fallback return expression and updating `self.flow`.
    #[allow(clippy::too_many_arguments)]
    fn returned_residue(
        &mut self,
        r: &Val,
        s: &Stmt,
        mut kept: Vec<Stmt>,
        b: &Block,
        saved_pc: Depth,
        env: &mut Env,
        eval_lets: &HashSet<u32>,
        let_originals: &HashMap<u32, Expr>,
    ) -> Val {
        let ret_stmt = Self::returning(r.clone(), s);
        if let Stmt::Expr(ref rx) = ret_stmt
            && Self::closed_residual(r, rx.span).is_none()
        {
            kept.extend(Self::settle(&[rx], env));
        }
        kept.push(ret_stmt);
        let out = self.residue_pruned(kept, None, b, r, saved_pc, eval_lets, let_originals);
        self.flow = Some(Flow::Returned(out.clone()));
        out
    }

    fn drain_settling(&mut self, mark: usize, kept: &mut Vec<Stmt>) {
        for s in self.settling.drain(mark..) {
            if let Stmt::Expr(e) = &s {
                let (_, la, fa) = Self::write_depths(&[e]);
                if let Some(ed) = join_opt_depth(la, fa) {
                    self.pc = self.pc.join(ed);
                }
            }
            kept.push(s);
        }
    }

    fn strip_loop_exit_guards(&mut self, from: usize) {
        let mut write = from;
        for read in from..self.settling.len() {
            let drop_it = matches!(
                &self.settling[read],
                Stmt::Expr(e) if Self::write_depths(&[e]).1.is_some()
            );
            if !drop_it {
                if write != read {
                    self.settling.swap(write, read);
                }
                write += 1;
            }
        }
        self.settling.truncate(write);
    }

    fn may_be_nonclosed(s: &Stmt) -> bool {
        let non_scalar = |ty: &Type| {
            !matches!(
                ty,
                Type::Int | Type::Bool | Type::Bytes | Type::Str | Type::Cairn | Type::Unit
            )
        };
        let root = match s {
            Stmt::Let { value, .. } => {
                if non_scalar(&value.ty) {
                    return true;
                }
                value
            }
            Stmt::Expr(e) => e,
            Stmt::Declare { .. } => return false,
        };
        let mut todo = vec![root];
        while let Some(cur) = todo.pop() {
            match &cur.kind {
                ExprKind::Assign { value, .. } if non_scalar(&value.ty) => return true,
                ExprKind::Block(_) | ExprKind::Select { .. } | ExprKind::Loop { .. } => {
                    return true;
                }
                _ => todo.extend(children(cur)),
            }
        }
        false
    }

    fn rollback_dirty(
        &mut self,
        dirty: DirtyRollback,
        mut kept: Vec<Stmt>,
        b: &Block,
        starved: &Val,
        saved_pc: Depth,
        env: &mut Env,
    ) -> Val {
        self.rollback(dirty.cp);
        *env = dirty.env;
        kept.truncate(dirty.kept_len);
        let rem =
            Block { stmts: b.stmts[dirty.index..].to_vec(), tail: b.tail.clone(), span: b.span };
        let rem_expr =
            Expr { kind: ExprKind::Block(rem), ty: Type::Unit, depth: Depth::PURE, span: b.span };
        kept.extend(Self::leave(&[&rem_expr], self.pc, env));
        kept.extend(b.stmts[dirty.index..].iter().cloned());
        self.residue_pruned(
            kept,
            b.tail.clone(),
            b,
            starved,
            saved_pc,
            &dirty.eval_lets,
            &dirty.let_originals,
        )
    }

    fn eval_block_stmt(
        &mut self,
        s: &Stmt,
        kept: &mut Vec<Stmt>,
        eval_lets: &mut HashSet<u32>,
        let_originals: &mut HashMap<u32, Expr>,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let mark = self.settling.len();
        Ok(match s {
            Stmt::Let { local, value } => {
                let deposits_before = self.deposits.len();
                let served_before = self.served.clone();
                let writes_before = env.writes;
                let v = self.expr(value, env)?;
                self.drain_settling(mark, kept);
                if self.flow.is_some() {
                    return Ok(v);
                }
                let mutates_or_deposits =
                    self.deposits.len() > deposits_before || env.writes != writes_before;
                let effects_or_mut = mutates_or_deposits || self.served != served_before;
                if !v.is_stuck()
                    && Self::closed_residual(&v, value.span).is_none()
                    && effects_or_mut
                    && !self.nonclosed_dirty.contains(&local.0)
                {
                    self.nonclosed_dirty.push(local.0);
                }
                let stuck = v.is_stuck();
                env.bind(local.0 as usize, Some(v.clone()));
                if stuck {
                    if value.depth > v.depth {
                        let_originals.insert(local.0, value.clone());
                    }
                    v
                } else {
                    eval_lets.insert(local.0);
                    let res = Self::residual_settled(v, value, kept, env);
                    if value.depth > res.depth {
                        let_originals.insert(local.0, value.clone());
                    }
                    kept.push(Stmt::Let { local: *local, value: res });
                    Self::unit()
                }
            }
            Stmt::Declare { local } => {
                env.bind(local.0 as usize, None);
                kept.push(Stmt::Declare { local: *local });
                Self::unit()
            }
            Stmt::Expr(x) => {
                let v = self.expr(x, env)?;
                self.drain_settling(mark, kept);
                if self.flow.is_none() && !matches!(v.kind, Kind::Known(ValLit::Unit)) {
                    if let Some(work) = Self::depositable_work(&v) {
                        if self.speculating {
                            let e = Box::new(Self::residual(v.clone(), x));
                            Val { kind: Kind::Stuck(e), ty: v.ty, depth: v.depth }
                        } else {
                            self.burn_bytes(work, x.span)?;
                            if let Some(value) = as_value(&v) {
                                let at = self.deposit(value, x.span);
                                self.deposits.push(at);
                            }
                            v
                        }
                    } else {
                        v
                    }
                } else {
                    v
                }
            }
        })
    }

    fn depositable_work(v: &Val) -> Option<usize> {
        match &v.kind {
            Kind::Func(_) | Kind::Prim(_) | Kind::Stuck(_) => None,
            Kind::Shade(inner) | Kind::Answered(inner) => {
                Self::depositable_work(inner).map(|w| w.saturating_add(64))
            }
            _ => Some(Self::val_byte_work(v).saturating_add(16)),
        }
    }

    fn mark_act_blocked_if_reaches(&mut self, b: &Block, from: usize) {
        let tail_reaches = b.tail.as_deref().is_some_and(|t| Self::reaches_act(t, self.unit));
        let stmts_reach = b.stmts[from..].iter().any(|s| match s {
            Stmt::Let { value, .. } | Stmt::Expr(value) => Self::reaches_act(value, self.unit),
            Stmt::Declare { .. } => false,
        });
        if tail_reaches || stmts_reach {
            self.act_blocked = true;
        }
    }

    fn advance_pc_from_expr(&mut self, e: &Expr) {
        let (_, la, fa) = Self::write_depths(&[e]);
        if let Some(ed) = join_opt_depth(la, fa) {
            self.pc = self.pc.join(ed);
        }
    }

    fn dirty_locals_live(
        dirty_locals: &[u32],
        bound: &BTreeSet<u32>,
        remaining_stmts: &[Stmt],
        tail: Option<&Expr>,
    ) -> bool {
        if dirty_locals.is_empty() {
            return false;
        }
        let mut live = HashSet::new();
        for s in remaining_stmts {
            match s {
                Stmt::Let { value, .. } | Stmt::Expr(value) => Self::reads(value, &mut live),
                Stmt::Declare { .. } => {}
            }
        }
        if let Some(t) = tail {
            Self::reads(t, &mut live);
        }
        dirty_locals.iter().any(|id| !bound.contains(id) || live.contains(id))
    }

    fn merge_dirty(&mut self, mut outer: Vec<u32>) {
        for id in self.nonclosed_dirty.drain(..) {
            if !outer.contains(&id) {
                outer.push(id);
            }
        }
        self.nonclosed_dirty = outer;
    }

    #[allow(clippy::too_many_arguments)]
    fn block_starved_stmt(
        &mut self,
        b: &Block,
        i: usize,
        stmt: &Stmt,
        v: &Val,
        mut kept: Vec<Stmt>,
        dirty_rollback: Option<DirtyRollback>,
        cur_dirty: &[u32],
        saved_pc: Depth,
        entry_gen: u64,
        env: &mut Env,
        eval_lets: &HashSet<u32>,
        let_originals: &HashMap<u32, Expr>,
    ) -> Result<Val, Halt> {
        let bound = Self::block_bound(b);
        if let Some(dirty) = dirty_rollback
            && Self::dirty_locals_live(cur_dirty, &bound, &b.stmts[i..], b.tail.as_deref())
        {
            self.pc = saved_pc;
            return Ok(self.rollback_dirty(dirty, kept, b, v, saved_pc, env));
        }
        Self::flush_stale_since(Some(&bound), entry_gen, env, &mut kept);
        let halted = Self::halted(v.clone(), stmt);
        if let Stmt::Let { value: ex, .. } | Stmt::Expr(ex) = &halted {
            self.advance_pc_from_expr(ex);
        }
        kept.push(halted);
        let res =
            self.after_starved(b, i + 1, kept, v, saved_pc, &bound, env, eval_lets, let_originals);
        self.pc = saved_pc;
        if res.is_ok() {
            self.mark_act_blocked_if_reaches(b, i + 1);
        }
        res
    }

    fn block(&mut self, b: &Block, env: &mut Env) -> Result<Val, Halt> {
        let saved_pc = self.pc;
        let entry_gen = env.writes;
        let outer_dirty = std::mem::take(&mut self.nonclosed_dirty);
        let mut dirty_rollback: Option<DirtyRollback> = None;
        let mut kept: Vec<Stmt> = Vec::new();
        let mut eval_lets: HashSet<u32> = HashSet::new();
        let mut let_originals: HashMap<u32, Expr> = HashMap::new();
        for (i, stmt) in b.stmts.iter().enumerate() {
            let cand =
                (dirty_rollback.is_none() && Self::may_be_nonclosed(stmt)).then(|| DirtyRollback {
                    index: i,
                    cp: self.checkpoint(),
                    env: env.clone(),
                    kept_len: kept.len(),
                    eval_lets: eval_lets.clone(),
                    let_originals: let_originals.clone(),
                });
            let v =
                self.eval_block_stmt(stmt, &mut kept, &mut eval_lets, &mut let_originals, env)?;
            if let Some(c) = cand {
                if !self.nonclosed_dirty.is_empty() {
                    dirty_rollback = Some(c);
                } else if self.starved.is_none() {
                    self.starved = c.cp.starved;
                }
            }
            if v.is_stuck() || self.flow.is_some() {
                if let Some(Flow::Returned(ret)) = self.flow.clone()
                    && (ret.is_stuck()
                        || (self.speculating && Self::closed_residual(&ret, b.span).is_none()))
                {
                    let cur_dirty = std::mem::replace(&mut self.nonclosed_dirty, outer_dirty);
                    self.pc = saved_pc;
                    let bound = Self::block_bound(b);
                    if let Some(dirty) = dirty_rollback
                        && Self::dirty_locals_live(
                            &cur_dirty,
                            &bound,
                            &b.stmts[i..],
                            b.tail.as_deref(),
                        )
                    {
                        return Ok(self.rollback_dirty(dirty, kept, b, &ret, saved_pc, env));
                    }
                    return Ok(self.returned_residue(
                        &ret,
                        stmt,
                        kept,
                        b,
                        saved_pc,
                        env,
                        &eval_lets,
                        &let_originals,
                    ));
                }
                if v.is_stuck() && self.flow.is_none() {
                    let cur_dirty = std::mem::replace(&mut self.nonclosed_dirty, outer_dirty);
                    return self.block_starved_stmt(
                        b,
                        i,
                        stmt,
                        &v,
                        kept,
                        dirty_rollback,
                        &cur_dirty,
                        saved_pc,
                        entry_gen,
                        env,
                        &eval_lets,
                        &let_originals,
                    );
                }
                self.pc = saved_pc;
                self.merge_dirty(outer_dirty);
                self.bubble_outer_guards(b, &kept);
                return Ok(v);
            }
        }
        self.block_tail(
            b,
            kept,
            env,
            dirty_rollback,
            &eval_lets,
            &let_originals,
            saved_pc,
            entry_gen,
            outer_dirty,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn block_tail(
        &mut self,
        b: &Block,
        mut kept: Vec<Stmt>,
        env: &mut Env,
        dirty_rollback: Option<DirtyRollback>,
        eval_lets: &HashSet<u32>,
        let_originals: &HashMap<u32, Expr>,
        saved_pc: Depth,
        entry_gen: u64,
        outer_dirty: Vec<u32>,
    ) -> Result<Val, Halt> {
        let out = match &b.tail {
            Some(tail) => {
                let mark = self.settling.len();
                let v = self.expr(tail, env)?;
                self.drain_settling(mark, &mut kept);
                if let Some(Flow::Returned(ret)) = self.flow.clone()
                    && (ret.is_stuck()
                        || (self.speculating && Self::closed_residual(&ret, tail.span).is_none()))
                {
                    let stmt = Stmt::Expr((**tail).clone());
                    let cur_dirty = std::mem::replace(&mut self.nonclosed_dirty, outer_dirty);
                    let bound = Self::block_bound(b);
                    self.pc = saved_pc;
                    if let Some(dirty) = dirty_rollback
                        && Self::dirty_locals_live(&cur_dirty, &bound, &[], Some(tail))
                    {
                        return Ok(self.rollback_dirty(dirty, kept, b, &ret, saved_pc, env));
                    }
                    return Ok(self.returned_residue(
                        &ret,
                        &stmt,
                        kept,
                        b,
                        saved_pc,
                        env,
                        eval_lets,
                        let_originals,
                    ));
                }
                if v.is_stuck() && self.flow.is_none() {
                    let cur_dirty = std::mem::replace(&mut self.nonclosed_dirty, outer_dirty);
                    let bound = Self::block_bound(b);
                    self.pc = saved_pc;
                    if let Some(dirty) = dirty_rollback
                        && Self::dirty_locals_live(&cur_dirty, &bound, &[], Some(tail))
                    {
                        return Ok(self.rollback_dirty(dirty, kept, b, &v, saved_pc, env));
                    }
                    Self::flush_stale_since(Some(&bound), entry_gen, env, &mut kept);
                    let tail = Some(Box::new(Self::residual(v.clone(), tail)));
                    let out =
                        self.residue_pruned(kept, tail, b, &v, saved_pc, eval_lets, let_originals);
                    return Ok(out);
                }
                v
            }
            None => Self::unit(),
        };
        self.pc = saved_pc;
        self.merge_dirty(outer_dirty);
        self.bubble_outer_guards(b, &kept);
        Ok(out)
    }

    /// Preserve dead-branch depth guards and outer-local non-literal assignments
    /// (`Answered`/`Refused`/deep `Shade`) emitted inside a finished inner block
    /// when they target locals not bound in `b`. Deduplicated so loops stay O(1).
    fn bubble_outer_guards(&mut self, b: &Block, kept: &[Stmt]) {
        let mut bound = BTreeSet::new();
        for s in &b.stmts {
            match s {
                Stmt::Let { local, .. } | Stmt::Declare { local } => {
                    bound.insert(local.0);
                }
                Stmt::Expr(_) => {}
            }
        }
        for s in kept {
            let Stmt::Expr(e) = s else { continue };
            let mut e_reads = HashSet::new();
            Self::reads(e, &mut e_reads);
            if e_reads.iter().any(|id| bound.contains(id)) {
                continue;
            }
            match &e.kind {
                ExprKind::Select { cond, .. }
                    if e.ty == Type::Unit
                        && matches!(cond.kind, ExprKind::Literal(Literal::Bool(_))) =>
                {
                    let (writes, la, fa) = Self::write_depths(&[e]);
                    if (la.is_some() || fa.is_some() || writes.keys().any(|id| !bound.contains(id)))
                        && !self.settling.contains(s)
                    {
                        self.settling.push(s.clone());
                    }
                }
                ExprKind::Assign { place, .. }
                    if place.path.is_empty() && !bound.contains(&place.local.0) =>
                {
                    self.settling.retain(|prev| {
                        !matches!(
                            prev,
                            Stmt::Expr(Expr {
                                kind: ExprKind::Assign { place: p, .. },
                                depth: prev_d,
                                ..
                            }) if p.path.is_empty() && p.local == place.local && e.depth >= *prev_d
                        )
                    });
                    self.settling.push(s.clone());
                }
                _ => {}
            }
        }
    }

    /// Reduce the statements and tail after a starved statement in a block.
    #[allow(clippy::too_many_arguments)]
    fn after_starved(
        &mut self,
        b: &Block,
        from: usize,
        mut kept: Vec<Stmt>,
        starved: &Val,
        saved_pc: Depth,
        bound: &BTreeSet<u32>,
        env: &mut Env,
        eval_lets: &HashSet<u32>,
        let_originals: &HashMap<u32, Expr>,
    ) -> Result<Val, Halt> {
        let was = std::mem::replace(&mut self.speculating, true);
        for (i, s) in b.stmts.iter().enumerate().skip(from) {
            let x = match s {
                Stmt::Declare { local } => {
                    env.bind(local.0 as usize, None);
                    kept.push(Stmt::Declare { local: *local });
                    continue;
                }
                Stmt::Let { value, .. } | Stmt::Expr(value) => value,
            };
            let mark = self.settling.len();
            let stmt_gen = env.writes;
            let res = self.try_expr(x, env);
            self.drain_settling(mark, &mut kept);
            let Some(v) = res.inspect_err(|_| self.speculating = was)? else {
                self.speculating = was;
                let rem =
                    Block { stmts: b.stmts[i..].to_vec(), tail: b.tail.clone(), span: b.span };
                let rem_expr = Expr {
                    kind: ExprKind::Block(rem),
                    ty: Type::Unit,
                    depth: Depth::PURE,
                    span: b.span,
                };
                kept.extend(Self::leave(&[&rem_expr], self.pc, env));
                kept.extend(b.stmts[i..].iter().cloned());
                return Ok(self.residue_pruned(
                    kept,
                    b.tail.clone(),
                    b,
                    starved,
                    saved_pc,
                    eval_lets,
                    let_originals,
                ));
            };
            if let Some(Flow::Returned(r)) = self.flow.take() {
                self.speculating = was;
                return Ok(self.returned_residue(
                    &r,
                    s,
                    kept,
                    b,
                    saved_pc,
                    env,
                    eval_lets,
                    let_originals,
                ));
            }
            self.keep_starved_stmt(s, v, stmt_gen, bound, &mut kept, env);
        }
        let tail = match &b.tail {
            Some(t) => {
                let mark = self.settling.len();
                let tail_gen = env.writes;
                let out = self.try_expr(t, env);
                self.drain_settling(mark, &mut kept);
                self.speculating = was;
                if let Some(v) = out? {
                    if let Some(Flow::Returned(r)) = self.flow.take() {
                        let s = Stmt::Expr((**t).clone());
                        return Ok(self.returned_residue(
                            &r,
                            &s,
                            kept,
                            b,
                            saved_pc,
                            env,
                            eval_lets,
                            let_originals,
                        ));
                    }
                    let tail_expr = Self::residual_settled(v.clone(), t, &mut kept, env);
                    Self::flush_stale_since(Some(bound), tail_gen, env, &mut kept);
                    let tail = Some(Box::new(tail_expr));
                    return Ok(self.residue_pruned(
                        kept,
                        tail,
                        b,
                        &v,
                        saved_pc,
                        eval_lets,
                        let_originals,
                    ));
                }
                kept.extend(Self::leave(&[&**t], self.pc, env));
                b.tail.clone()
            }
            None => None,
        };
        self.speculating = was;
        Ok(self.residue_pruned(kept, tail, b, starved, saved_pc, eval_lets, let_originals))
    }

    fn keep_starved_stmt(
        &mut self,
        s: &Stmt,
        v: Val,
        stmt_gen: u64,
        bound: &BTreeSet<u32>,
        kept: &mut Vec<Stmt>,
        env: &mut Env,
    ) {
        match s {
            Stmt::Let { local, value } => {
                env.bind(local.0 as usize, Some(v.clone()));
                let res = Self::residual_settled(v, value, kept, env);
                Self::flush_stale_since(Some(bound), stmt_gen, env, kept);
                self.advance_pc_from_expr(&res);
                kept.push(Stmt::Let { local: *local, value: res });
            }
            Stmt::Expr(x) if v.is_stuck() || as_value(&v).is_some_and(|val| val != Value::Unit) => {
                let res = Self::residual_settled(v, x, kept, env);
                Self::flush_stale_since(Some(bound), stmt_gen, env, kept);
                self.advance_pc_from_expr(&res);
                kept.push(Stmt::Expr(res));
            }
            Stmt::Expr(_) | Stmt::Declare { .. } => {
                Self::flush_stale_since(Some(bound), stmt_gen, env, kept);
            }
        }
    }

    fn record_prune_info(
        value: &Expr,
        read: &mut HashSet<u32>,
        shaded_reads: &mut HashSet<u32>,
        has_opaque: &mut bool,
        has_assign: &mut bool,
        min_ret: &mut Option<Depth>,
    ) {
        Self::reads(value, read);
        Self::shade_reads(value, shaded_reads);
        if Self::has_opaque(value) {
            *has_opaque = true;
        }
        if Self::has_assign(value) {
            *has_assign = true;
        }
        if let Some(rd) = Self::min_ret_depth(value) {
            *min_ret = Some(min_ret.map_or(rd, |m| m.min(rd)));
        }
    }

    /// Build a residual block, dropping already-evaluated `Stmt::Let` bindings
    /// that no surviving statement reads (so answered calls/deep shades do not
    /// linger or re-execute in Stage 2) and preserving original depth via a
    /// non-executing `if (false)` guard when a surviving `shade` depends on a
    /// folded binding's depth.
    #[allow(clippy::too_many_arguments)]
    fn residue_pruned(
        &mut self,
        stmts: Vec<Stmt>,
        tail: Option<Box<Expr>>,
        b: &Block,
        v: &Val,
        saved_pc: Depth,
        eval_lets: &HashSet<u32>,
        let_originals: &HashMap<u32, Expr>,
    ) -> Val {
        let mut read: HashSet<u32> = HashSet::new();
        let mut shaded_reads: HashSet<u32> = HashSet::new();
        let mut has_opaque = false;
        let mut has_assign = false;
        let mut min_ret: Option<Depth> = None;
        if let Some(t) = &tail {
            Self::record_prune_info(
                t,
                &mut read,
                &mut shaded_reads,
                &mut has_opaque,
                &mut has_assign,
                &mut min_ret,
            );
        }
        let mut kept: Vec<Stmt> = Vec::with_capacity(stmts.len());
        for s in stmts.into_iter().rev() {
            if let Stmt::Let { local, value } = &s
                && eval_lets.contains(&local.0)
                && !read.contains(&local.0)
                && !matches!(value.kind, ExprKind::Literal(_))
            {
                continue;
            }
            if let Stmt::Expr(e) = &s
                && Self::is_dead_branch_guard(e)
            {
                let (writes, la, fa) = Self::write_depths(&[e]);
                let writes_used = writes.keys().any(|id| read.contains(id));
                let exit_used = join_opt_depth(la, fa).is_some_and(|ed| {
                    has_opaque
                        || has_assign
                        || !shaded_reads.is_empty()
                        || min_ret.is_some_and(|rd| ed > rd)
                });
                if !writes_used && !exit_used {
                    continue;
                }
            }
            if let Stmt::Let { local, value } = &s
                && shaded_reads.contains(&local.0)
                && let Some(orig) = let_originals.get(&local.0)
                && orig.depth > value.depth
            {
                let assign_orig = Expr {
                    kind: ExprKind::Assign {
                        place: Place { local: *local, path: Vec::new() },
                        value: Box::new(orig.clone()),
                    },
                    ty: Type::Unit,
                    depth: orig.depth,
                    span: orig.span,
                };
                let guard = Self::dead_branch_guard(false, &assign_orig, orig.span);
                Self::reads(&guard, &mut read);
                Self::reads(&guard, &mut shaded_reads);
                kept.push(Stmt::Expr(guard));
            }
            match &s {
                Stmt::Let { local, value } => {
                    Self::record_prune_info(
                        value,
                        &mut read,
                        &mut shaded_reads,
                        &mut has_opaque,
                        &mut has_assign,
                        &mut min_ret,
                    );
                    if shaded_reads.contains(&local.0) {
                        Self::reads(value, &mut shaded_reads);
                    }
                }
                Stmt::Expr(value) => {
                    Self::record_prune_info(
                        value,
                        &mut read,
                        &mut shaded_reads,
                        &mut has_opaque,
                        &mut has_assign,
                        &mut min_ret,
                    );
                }
                Stmt::Declare { .. } => {}
            }
            kept.push(s);
        }
        kept.reverse();
        let mut tail = tail;
        if !let_originals.is_empty() {
            let mut depths: HashMap<u32, Depth> = HashMap::new();
            let mut steps = 0usize;
            Self::refresh_stmts(&mut kept, &mut tail, saved_pc, &mut steps, &mut depths);
        }
        self.residue(kept, tail, b, v)
    }

    /// Recompute stated `Expr.depth` inside a residual block when an earlier
    /// `Stmt::Let` folded to a shallower depth, so unreduced forms (such as
    /// `opaque`) stay consistent with `nether_core::check`.
    fn refresh_stmts(
        stmts: &mut [Stmt],
        tail: &mut Option<Box<Expr>>,
        pc: Depth,
        steps: &mut usize,
        depths: &mut HashMap<u32, Depth>,
    ) -> (Depth, (Option<Depth>, Option<Depth>)) {
        let mut cur_pc = pc;
        let mut exit: (Option<Depth>, Option<Depth>) = (None, None);
        for s in stmts {
            match s {
                Stmt::Let { local, value } => {
                    let (d, ex) = Self::refresh_expr(value, cur_pc, steps, depths);
                    depths.insert(local.0, d);
                    exit = (join_opt_depth(exit.0, ex.0), join_opt_depth(exit.1, ex.1));
                    if let Some(ed) = join_opt_depth(ex.0, ex.1) {
                        cur_pc = cur_pc.join(ed);
                    }
                }
                Stmt::Declare { local } => {
                    depths.insert(local.0, Depth::PURE);
                }
                Stmt::Expr(x) => {
                    let (_, ex) = Self::refresh_expr(x, cur_pc, steps, depths);
                    exit = (join_opt_depth(exit.0, ex.0), join_opt_depth(exit.1, ex.1));
                    if let Some(ed) = join_opt_depth(ex.0, ex.1) {
                        cur_pc = cur_pc.join(ed);
                    }
                }
            }
        }
        let (td, tex) = tail
            .as_mut()
            .map_or((Depth::PURE, (None, None)), |t| Self::refresh_expr(t, cur_pc, steps, depths));
        (td, (join_opt_depth(exit.0, tex.0), join_opt_depth(exit.1, tex.1)))
    }

    fn refresh_expr(
        x: &mut Expr,
        pc: Depth,
        steps: &mut usize,
        depths: &mut HashMap<u32, Depth>,
    ) -> (Depth, (Option<Depth>, Option<Depth>)) {
        let join_ex = |a: (Option<Depth>, Option<Depth>), b: (Option<Depth>, Option<Depth>)| {
            (join_opt_depth(a.0, b.0), join_opt_depth(a.1, b.1))
        };
        let adv = |p: Depth, e: (Option<Depth>, Option<Depth>)| {
            join_opt_depth(e.0, e.1).map_or(p, |ed| p.join(ed))
        };
        let (d, ex) = match &mut x.kind {
            ExprKind::Literal(_) | ExprKind::Func(_) | ExprKind::Prim(_) => {
                (Depth::PURE, (None, None))
            }
            ExprKind::Break | ExprKind::Continue => (Depth::PURE, (Some(pc), None)),
            ExprKind::Local(id) => (depths.get(&id.0).copied().unwrap_or(x.depth), (None, None)),
            ExprKind::Global(_) => (x.depth, (None, None)),
            ExprKind::Unary { operand, .. } | ExprKind::Field { base: operand, .. } => {
                Self::refresh_expr(operand, pc, steps, depths)
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
                let (dl, el) = Self::refresh_expr(lhs, pc, steps, depths);
                let (dr, er) = Self::refresh_expr(rhs, adv(pc, el), steps, depths);
                (dl.join(dr), join_ex(el, er))
            }
            ExprKind::Call { callee, args } => {
                let (df, mut ex) = Self::refresh_expr(callee, pc, steps, depths);
                let mut cur_pc = adv(pc, ex);
                let mut da = Depth::PURE;
                for a in args {
                    let (ad, aex) = Self::refresh_expr(a, cur_pc, steps, depths);
                    da = da.join(ad);
                    ex = join_ex(ex, aex);
                    cur_pc = adv(cur_pc, aex);
                }
                let rd = match callee.ty {
                    Type::Fn { result_depth, .. } => result_depth,
                    _ => Depth::PURE,
                };
                (rd.join(df).join(da), ex)
            }
            ExprKind::Descend { body, .. } => Self::refresh_expr(body, pc, steps, depths),
            ExprKind::Rite { rite, operand } => {
                let (od, ex) = Self::refresh_expr(operand, pc, steps, depths);
                let rd = match rite {
                    Rite::Seal | Rite::Shade => Depth::PURE,
                    Rite::Opaque => od,
                    Rite::Look => match operand.ty {
                        Type::Shade { origin, .. } => origin.join(od),
                        _ => od,
                    },
                };
                (rd, ex)
            }
            ExprKind::Select { cond, then, otherwise } => {
                let (dc, ec) = Self::refresh_expr(cond, pc, steps, depths);
                let bpc = adv(pc, ec).join(dc);
                let mut else_depths = depths.clone();
                let (dt, et) = Self::refresh_expr(then, bpc, steps, depths);
                let (de, ee) = Self::refresh_expr(otherwise, bpc, steps, &mut else_depths);
                for (id, d) in else_depths {
                    depths.entry(id).and_modify(|v| *v = v.join(d)).or_insert(d);
                }
                (dc.join(dt).join(de), join_ex(ec, join_ex(et, ee)))
            }
            ExprKind::Block(b) => Self::refresh_stmts(&mut b.stmts, &mut b.tail, pc, steps, depths),
            ExprKind::Loop { body, step } => {
                Self::refresh_loop(body, step.as_deref_mut(), pc, steps, depths)
            }
            ExprKind::Return(v) => {
                let (d, ex) = v.as_mut().map_or((Depth::PURE, (None, None)), |e| {
                    Self::refresh_expr(e, pc, steps, depths)
                });
                (d, join_ex(ex, (None, Some(adv(pc, ex)))))
            }
            ExprKind::Assign { place, value } => {
                let (mut cur_pc, mut ex, mut dp) = (pc, (None, None), Depth::PURE);
                for p in &mut place.path {
                    if let nether_core::Proj::Index(i) = p {
                        let (di, ei) = Self::refresh_expr(i, cur_pc, steps, depths);
                        dp = dp.join(di);
                        ex = join_ex(ex, ei);
                        cur_pc = adv(cur_pc, ei);
                    }
                }
                let (dv, ev) = Self::refresh_expr(value, cur_pc, steps, depths);
                ex = join_ex(ex, ev);
                let wd = dp.join(dv).join(adv(cur_pc, ev));
                depths.entry(place.local.0).and_modify(|d| *d = d.join(wd)).or_insert(wd);
                (dp.join(dv), ex)
            }
        };
        x.depth = d;
        (d, ex)
    }

    fn refresh_loop(
        body: &mut Expr,
        mut step: Option<&mut Expr>,
        pc: Depth,
        steps: &mut usize,
        depths: &mut HashMap<u32, Depth>,
    ) -> (Depth, (Option<Depth>, Option<Depth>)) {
        let mut loop_pc = pc;
        let mut res = (Depth::PURE, (None, None));
        for _ in 0..64 {
            if *steps >= MAX_FIXPOINT_STEPS {
                break;
            }
            *steps += 1;
            let before = (depths.clone(), loop_pc);
            let (db, eb) = Self::refresh_expr(body, loop_pc, steps, depths);
            let step_pc = join_opt_depth(eb.0, eb.1).map_or(loop_pc, |e| loop_pc.join(e));
            let (ds, es) = match step.as_deref_mut() {
                Some(s) => Self::refresh_expr(s, step_pc, steps, depths),
                None => (Depth::PURE, (None, None)),
            };
            let combined = (join_opt_depth(eb.0, es.0), join_opt_depth(eb.1, es.1));
            if let Some(ed) = join_opt_depth(combined.0, combined.1) {
                loop_pc = loop_pc.join(ed);
            }
            res = (db.join(ds), (None, combined.1));
            if (&*depths, loop_pc) == (&before.0, before.1) {
                break;
            }
        }
        res
    }

    /// Whether `e` is a unit-typed dead-branch guard `if (true) {} else { .. }`
    /// or `if (false) { .. } else {}`.
    fn is_dead_branch_guard(e: &Expr) -> bool {
        let ExprKind::Select { cond, then, otherwise } = &e.kind else { return false };
        if e.ty != Type::Unit {
            return false;
        }
        let is_empty = |arm: &Expr| {
            matches!(&arm.kind, ExprKind::Literal(Literal::Unit))
                || matches!(&arm.kind, ExprKind::Block(b) if b.stmts.is_empty() && b.tail.is_none())
        };
        match cond.kind {
            ExprKind::Literal(Literal::Bool(true)) => is_empty(then),
            ExprKind::Literal(Literal::Bool(false)) => is_empty(otherwise),
            _ => false,
        }
    }

    /// Whether `x` can deposit a non-`U0` value (directly or through called
    /// functions / referenced globals), so a starved global must stay as
    /// `Global(id)` rather than duplicating its deposits across demand sites.
    fn reaches_deposit(x: &Expr, unit: &Unit) -> bool {
        let mut todo: Vec<&Expr> = vec![x];
        let mut seen_funcs: HashSet<FuncId> = HashSet::new();
        let mut seen_globals: HashSet<GlobalId> = HashSet::new();
        while let Some(cur) = todo.pop() {
            match &cur.kind {
                ExprKind::Block(b) => {
                    for s in &b.stmts {
                        if let Stmt::Expr(e) = s
                            && e.ty != Type::Unit
                        {
                            return true;
                        }
                    }
                }
                ExprKind::Func(id) if seen_funcs.insert(*id) => {
                    if let Some(f) = unit.func(*id) {
                        for s in &f.body.stmts {
                            if let Stmt::Expr(e) = s
                                && e.ty != Type::Unit
                            {
                                return true;
                            }
                            if let Stmt::Let { value, .. } | Stmt::Expr(value) = s {
                                todo.push(value);
                            }
                        }
                        if let Some(t) = &f.body.tail {
                            todo.push(t);
                        }
                    }
                }
                ExprKind::Global(id) if seen_globals.insert(*id) => {
                    if let Some(g) = unit.global(*id) {
                        todo.push(&g.value);
                    }
                }
                _ => {}
            }
            todo.extend(children(cur));
        }
        false
    }

    /// Whether `x` can reach any act-stratum primitive (directly or through
    /// called functions / referenced globals) so a starved global must stay as
    /// `Global(id)` rather than duplicating the act at every demand site.
    fn reaches_act(x: &Expr, unit: &Unit) -> bool {
        let mut todo: Vec<&Expr> = vec![x];
        let mut seen_funcs: HashSet<FuncId> = HashSet::new();
        let mut seen_globals: HashSet<GlobalId> = HashSet::new();
        while let Some(cur) = todo.pop() {
            match &cur.kind {
                ExprKind::Prim(p) if p.latent() > Depth::PURE && !p.latent().reads() => {
                    return true;
                }
                ExprKind::Func(id) if seen_funcs.insert(*id) => {
                    if let Some(f) = unit.func(*id) {
                        for s in &f.body.stmts {
                            if let Stmt::Let { value, .. } | Stmt::Expr(value) = s {
                                todo.push(value);
                            }
                        }
                        if let Some(t) = &f.body.tail {
                            todo.push(t);
                        }
                    }
                }
                ExprKind::Global(id) if seen_globals.insert(*id) => {
                    if let Some(g) = unit.global(*id) {
                        todo.push(&g.value);
                    }
                }
                _ => {}
            }
            if let Type::Fn { latent, .. } = cur.ty
                && [Depth::DISK_WRITE, Depth::NET_WRITE, Depth::ENTROPY, Depth::UNRECORDED]
                    .into_iter()
                    .any(|d| latent.holds(d))
            {
                return true;
            }
            todo.extend(children(cur));
        }
        false
    }

    /// Whether `x` contains any `opaque` rite.
    fn has_opaque(x: &Expr) -> bool {
        let mut todo = vec![x];
        while let Some(cur) = todo.pop() {
            if matches!(cur.kind, ExprKind::Rite { rite: Rite::Opaque, .. }) {
                return true;
            }
            todo.extend(children(cur));
        }
        false
    }

    /// Whether `x` contains any `Assign` whose checked depth depends on ambient `pc`.
    fn has_assign(x: &Expr) -> bool {
        let mut todo = vec![x];
        while let Some(cur) = todo.pop() {
            if matches!(cur.kind, ExprKind::Assign { .. }) {
                return true;
            }
            todo.extend(children(cur));
        }
        false
    }

    /// Shallowest `Return` value depth inside `x`, if any.
    fn min_ret_depth(x: &Expr) -> Option<Depth> {
        let mut todo = vec![x];
        let mut best: Option<Depth> = None;
        while let Some(cur) = todo.pop() {
            if let ExprKind::Return(v) = &cur.kind {
                let d = v.as_deref().map_or(Depth::PURE, |e| e.depth);
                best = Some(best.map_or(d, |b| b.min(d)));
            }
            todo.extend(children(cur));
        }
        best
    }

    /// Locals referenced directly inside any non-pure-origin `Rite::Shade` operand.
    fn shade_reads(x: &Expr, into: &mut HashSet<u32>) {
        let mut todo = vec![x];
        while let Some(cur) = todo.pop() {
            if let ExprKind::Rite { rite: Rite::Shade, operand } = &cur.kind
                && matches!(cur.ty, Type::Shade { origin, .. } if origin > Depth::PURE)
            {
                Self::reads(operand, into);
            }
            todo.extend(children(cur));
        }
    }

    /// What a block that did not finish residualises as.
    ///
    /// It is the block, and not the one statement that starved inside it: a
    /// block holds the bindings that statement reads and the `return` that
    /// stops the ones below it from happening. Reducing it to the bare
    /// expression that starved loses both, and the residue then does things
    /// the program does not.
    ///
    /// The same block goes to [`Burial::apply`], which is the only thing that
    /// wants it as a block rather than as an expression.
    fn residue(&mut self, stmts: Vec<Stmt>, tail: Option<Box<Expr>>, b: &Block, v: &Val) -> Val {
        // A block is worth what its tail is worth, and a block with no tail is
        // worth `U0` — which is what lowering the residue back will say, and
        // the two have to agree or the second burial specialises what the
        // first one already did.
        let (ty, depth) = match &tail {
            Some(t) => (t.ty.clone(), t.depth),
            None => (Type::Unit, Depth::PURE),
        };
        let block = Block { stmts, tail, span: b.span };
        self.starved = Some(block.clone());
        let kind = ExprKind::Block(block);
        let e = Expr { kind, ty, depth, span: b.span };
        // The value is still what starved. That is what the expression around
        // this block is waiting on, and it is not what the block is worth.
        Val { kind: Kind::Stuck(Box::new(e)), ty: v.ty.clone(), depth: v.depth }
    }

    /// Whether a residual block or descent's last word is a `return`.
    fn ends_returning(x: &Expr) -> bool {
        match &x.kind {
            ExprKind::Return(_) => true,
            ExprKind::Descend { body, .. } => Self::ends_returning(body),
            ExprKind::Block(b) => {
                b.tail.is_none()
                    && matches!(b.stmts.last(), Some(Stmt::Expr(last)) if Self::ends_returning(last))
            }
            _ => false,
        }
    }

    /// The statement that starved, reduced as far as it went.
    fn halted(v: Val, s: &Stmt) -> Stmt {
        match s {
            Stmt::Let { local, value } => {
                Stmt::Let { local: *local, value: Self::residual(v, value) }
            }
            Stmt::Declare { local } => Stmt::Declare { local: *local },
            Stmt::Expr(x) => Stmt::Expr(Self::residual(v, x)),
        }
    }

    /// `return e`, where `e` is what the block was carrying out of the
    /// function when it starved.
    fn returning(v: Val, s: &Stmt) -> Stmt {
        if matches!(s, Stmt::Expr(Expr { kind: ExprKind::Return(None), .. })) {
            return s.clone();
        }
        let (Stmt::Expr(x) | Stmt::Let { value: x, .. }) = s else { return s.clone() };
        let inner = Self::residual(v, if let ExprKind::Return(Some(e)) = &x.kind { e } else { x });
        // A `return` whose operand has no value form residualises as the
        // `return` it was, and a block or descent that already ends by
        // returning says it where it happens. Either way, saying it again
        // would say it twice.
        let inner = match inner.kind {
            ExprKind::Return(Some(e)) => *e,
            ExprKind::Block(_) | ExprKind::Descend { .. } if Self::ends_returning(&inner) => {
                return Stmt::Expr(inner);
            }
            _ => inner,
        };
        let depth = inner.depth;
        let kind = ExprKind::Return(Some(Box::new(inner)));
        Stmt::Expr(Expr { kind, ty: Type::Unit, depth, span: x.span })
    }

    /// Unroll a loop as far as it will go, and abandon the unrolling whole if
    /// it stops going.
    ///
    /// Abandoning is safe because nothing evaluated so far can have reached
    /// the world: reaching the world is what made it stop.
    fn loop_(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.grind.push(Grind { span: x.span, what: Open::Loop { turns: 0 } });
        // `?` on the way out of `unroll` leaves the frame where it is, which
        // is how the halt finds out what was going round.
        let out = self.unroll(x, env)?;
        self.grind.pop();
        Ok(out)
    }

    fn strip_expr_loop_guards(x: &mut Expr) {
        match &mut x.kind {
            ExprKind::Block(b) => {
                b.stmts.retain(|s| {
                    !matches!(
                        s,
                        Stmt::Expr(e)
                            if Self::is_dead_branch_guard(e)
                                && Self::write_depths(&[e]).1.is_some()
                    )
                });
                for s in &mut b.stmts {
                    match s {
                        Stmt::Let { value, .. } | Stmt::Expr(value) => {
                            Self::strip_expr_loop_guards(value);
                        }
                        Stmt::Declare { .. } => {}
                    }
                }
                if let Some(t) = b.tail.as_deref_mut() {
                    Self::strip_expr_loop_guards(t);
                }
            }
            ExprKind::Descend { body, .. }
            | ExprKind::Rite { operand: body, .. }
            | ExprKind::Unary { operand: body, .. }
            | ExprKind::Field { base: body, .. } => Self::strip_expr_loop_guards(body),
            ExprKind::Select { cond, then, otherwise } => {
                Self::strip_expr_loop_guards(cond);
                Self::strip_expr_loop_guards(then);
                Self::strip_expr_loop_guards(otherwise);
            }
            ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
                Self::strip_expr_loop_guards(lhs);
                Self::strip_expr_loop_guards(rhs);
            }
            ExprKind::Return(Some(v)) => Self::strip_expr_loop_guards(v),
            _ => {}
        }
    }

    fn clean_returned_from_loop(mut flow: Flow) -> Flow {
        if let Flow::Returned(ref mut ret) = flow
            && let Kind::Stuck(ref mut expr) = ret.kind
        {
            Self::strip_expr_loop_guards(expr);
        }
        flow
    }

    fn unroll(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let ExprKind::Loop { body, step } = &x.kind else {
            return Ok(Self::stuck(x));
        };
        let mut turn_before = env.clone();
        let mut turn_cp = self.checkpoint();
        let entry_settling = turn_cp.settling;
        // Questions asked during an unrolling that is then abandoned are kept
        // (`holes`, `asked`, `named`). Side effects that the residual loop will
        // replay (`deposits`, `served`, `specialised`, `settling`, `flow`) are
        // rolled back via `turn_cp`. Once a turn consumes an act answer, that
        // turn is committed by advancing `turn_cp` so Stage 2 never repeats it.
        loop {
            let v = self.expr(body, env)?;
            match self.flow.take() {
                Some(Flow::Broke) => {
                    self.strip_loop_exit_guards(entry_settling);
                    self.starved = turn_cp.starved;
                    return Ok(Self::unit());
                }
                // A `return` out of a loop keeps going up.
                Some(f @ Flow::Returned(_)) => {
                    self.strip_loop_exit_guards(entry_settling);
                    self.starved = turn_cp.starved;
                    let cleaned = Self::clean_returned_from_loop(f);
                    let out = match &cleaned {
                        Flow::Returned(ret) => ret.clone(),
                        _ => v,
                    };
                    self.flow = Some(cleaned);
                    return Ok(out);
                }
                // A `continue` still runs the step. That is the whole reason
                // the step is a field of the loop and not the last statement
                // of its body.
                Some(Flow::Continued) | None => {}
            }
            if v.is_stuck() {
                if self.served != turn_cp.served {
                    let body_res = Self::residual(v, body);
                    if Self::write_depths(&[&body_res]).1.is_none() {
                        let mut stmts = vec![Stmt::Expr(body_res)];
                        if let Some(s) = step {
                            stmts.push(Stmt::Expr((**s).clone()));
                        }
                        stmts.push(Stmt::Expr(x.clone()));
                        let blk = Expr {
                            kind: ExprKind::Block(Block { stmts, tail: None, span: x.span }),
                            ty: Type::Unit,
                            depth: Depth::PURE,
                            span: x.span,
                        };
                        self.starved = turn_cp.starved;
                        return Ok(self.unreduced(&blk, env));
                    }
                }
                *env = turn_before;
                self.rollback(turn_cp);
                if Self::reaches_act(x, self.unit) {
                    self.act_blocked = true;
                }
                return Ok(self.unreduced(x, env));
            }
            if let Some(s) = step {
                let sv = self.expr(s, env)?;
                match self.flow.take() {
                    Some(Flow::Broke) => {
                        self.strip_loop_exit_guards(entry_settling);
                        self.starved = turn_cp.starved;
                        return Ok(Self::unit());
                    }
                    Some(f @ Flow::Returned(_)) => {
                        self.strip_loop_exit_guards(entry_settling);
                        self.starved = turn_cp.starved;
                        let cleaned = Self::clean_returned_from_loop(f);
                        let out = match &cleaned {
                            Flow::Returned(ret) => ret.clone(),
                            _ => sv,
                        };
                        self.flow = Some(cleaned);
                        return Ok(out);
                    }
                    Some(Flow::Continued) | None => {}
                }
                if sv.is_stuck() {
                    *env = turn_before;
                    self.rollback(turn_cp);
                    return Ok(self.unreduced(x, env));
                }
            }
            if self.served != turn_cp.served {
                // Commit the completed act-consuming turn so a later turn's
                // starvation resumes from this iteration instead of replaying it.
                let saved_starved = turn_cp.starved;
                turn_before = env.clone();
                turn_cp = self.checkpoint();
                turn_cp.starved = saved_starved;
            }
            // Counted at the bottom, so that a turn the fuel cut short is not
            // one this claims to have finished.
            if let Some(Grind { what: Open::Loop { turns }, .. }) = self.grind.last_mut() {
                *turns += 1;
            }
        }
    }

    fn rite(&mut self, x: &Expr, rite: Rite, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        // `opaque` is never burned through, so its operand is not evaluated at
        // all, but stale local writes must be settled before it. §6.4.
        if rite == Rite::Opaque {
            return Ok(self.unreduced(x, env));
        }
        if rite == Rite::Shade {
            let cp = self.checkpoint();
            let before = env.clone();
            let v = match self.expr(operand, env) {
                Ok(v) => v,
                Err(halt) => {
                    *env = before;
                    self.rollback(cp);
                    return Err(halt);
                }
            };
            if self.flow.is_some() {
                self.starved = cp.starved;
                return Ok(v);
            }
            let orig_origin = match x.ty {
                Type::Shade { origin, .. } => origin,
                _ => v.depth,
            };
            if v.is_stuck() {
                let op_res = Self::residual(v, operand);
                if op_res.depth != orig_origin {
                    *env = before;
                    self.rollback(cp);
                    return Ok(self.unreduced(x, env));
                }
                self.starved = cp.starved;
                let ty = Type::Shade { origin: orig_origin, inner: Box::new(op_res.ty.clone()) };
                let kind = ExprKind::Rite { rite, operand: Box::new(op_res) };
                let e = Expr { kind, ty: ty.clone(), depth: Depth::PURE, span: x.span };
                return Ok(Val { kind: Kind::Stuck(Box::new(e)), ty, depth: Depth::PURE });
            }
            self.starved = cp.starved;
            let ty = Type::Shade { origin: orig_origin, inner: Box::new(v.ty.clone()) };
            return Ok(Val { kind: Kind::Shade(Box::new(v)), ty, depth: Depth::PURE });
        }
        let v = self.expr(operand, env)?;
        if self.flow.is_some() {
            return Ok(v);
        }
        Ok(match rite {
            // A name is pure, whatever it names, and a value that is here can
            // be named. One that is not stays a `seal` over the reduced operand.
            Rite::Seal => {
                // §1.5: `seal` on a shade names the shade. §7.1 puts the
                // stratum it came out of in its encoding, so `seal shade e` is
                // not `seal e` — reaching through an opaque thing for a name
                // would be a hole in it.
                if let Some(work) = Self::depositable_work(&v) {
                    self.burn_bytes(work, x.span)?;
                    if let Some(value) = as_value(&v) {
                        let cairn = if self.speculating {
                            Stored::Value(value).cairn()
                        } else {
                            self.remember(Stored::Value(value))
                        };
                        Self::known(Literal::Cairn(*cairn.as_bytes()), x)
                    } else {
                        let operand = Box::new(Self::residual(v, operand));
                        Self::rebuild(ExprKind::Rite { rite, operand }, x, Depth::PURE)
                    }
                } else {
                    let operand = Box::new(Self::residual(v, operand));
                    Self::rebuild(ExprKind::Rite { rite, operand }, x, Depth::PURE)
                }
            }
            Rite::Opaque | Rite::Shade => unreachable!("handled above"),
            Rite::Look => {
                if let Kind::Shade(inner) = v.kind {
                    *inner
                } else {
                    let operand = Box::new(Self::residual(v, operand));
                    let origin = match &operand.ty {
                        Type::Shade { origin, .. } => *origin,
                        _ => Depth::PURE,
                    };
                    let depth = origin.join(operand.depth);
                    Self::rebuild(ExprKind::Rite { rite, operand }, x, depth)
                }
            }
        })
    }

    fn assign(
        &mut self,
        x: &Expr,
        place: &Place,
        value: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let assign_cp = self.checkpoint();
        let assign_writes = env.writes;
        let assign_env = (!place.path.is_empty()).then(|| env.clone());
        let mut path = Vec::with_capacity(place.path.len());
        let mut depth = Depth::PURE;
        for proj in &place.path {
            match proj {
                nether_core::Proj::Field(i) => path.push(nether_core::Proj::Field(*i)),
                nether_core::Proj::Index(idx) => {
                    let iv = self.expr(idx, env)?;
                    if self.flow.is_some() {
                        if self.starved.is_none() {
                            self.starved = assign_cp.starved;
                        }
                        return Ok(iv);
                    }
                    let idx_res = Self::residual(iv, idx);
                    depth = depth.join(idx_res.depth);
                    path.push(nether_core::Proj::Index(Box::new(idx_res)));
                }
            }
        }
        let v = self.expr(value, env)?;
        if self.flow.is_some() {
            if self.starved.is_none() {
                self.starved = assign_cp.starved;
            }
            return Ok(v);
        }
        let effects_or_mut = self.deposits.len() > assign_cp.deposits
            || env.writes != assign_writes
            || self.served != assign_cp.served;
        // A path into an aggregate needs an aggregate, and nothing can build
        // one. Reduce any index subexpressions and settle stale locals before
        // leaving the rebuilt assignment in the residue.
        if !place.path.is_empty() || v.is_stuck() {
            if !v.is_stuck() && Self::closed_residual(&v, value.span).is_none() && effects_or_mut {
                self.rollback(assign_cp);
                if let Some(prev) = assign_env {
                    *env = prev;
                }
                return Ok(self.unreduced(x, env));
            }
            if !v.is_stuck() {
                self.starved = assign_cp.starved;
            }
            let val_res = Self::residual(v, value);
            depth = depth.join(val_res.depth);
            let rebuilt = Expr {
                kind: ExprKind::Assign {
                    place: Place { local: place.local, path },
                    value: Box::new(val_res.clone()),
                },
                ty: x.ty.clone(),
                depth,
                span: x.span,
            };
            if !place.path.is_empty() {
                return Ok(self.unreduced(&rebuilt, env));
            }
            let prev_depth = env
                .get(place.local.0 as usize)
                .and_then(Option::as_ref)
                .map_or(Depth::PURE, |pv| pv.depth);
            let next_depth = prev_depth.join(val_res.depth).join(self.pc);
            let local_expr = Expr {
                kind: ExprKind::Local(place.local),
                ty: val_res.ty,
                depth: next_depth,
                span: x.span,
            };
            env.bind(
                place.local.0 as usize,
                Some(Self::rebuild(local_expr.kind.clone(), &local_expr, next_depth)),
            );
            return Ok(Self::stuck(&rebuilt));
        }
        let non_literal_shade = matches!(
            (&v.kind, &v.ty),
            (Kind::Shade(_), Type::Shade { origin, .. }) if *origin > Depth::PURE
        );
        if matches!(v.kind, Kind::Answered(_) | Kind::Refused(_)) || non_literal_shade {
            if Self::closed_residual(&v, value.span).is_none()
                && effects_or_mut
                && !self.nonclosed_dirty.contains(&place.local.0)
            {
                self.nonclosed_dirty.push(place.local.0);
            }
            self.starved = assign_cp.starved;
            return Ok(self.assign_non_literal(x, place.local, value, v, env));
        }
        self.starved = assign_cp.starved;
        env.write(place.local.0 as usize, v);
        Ok(Val { kind: Kind::Known(ValLit::Unit), ty: Type::Unit, depth: Depth::PURE })
    }

    /// Record a non-literal (`Answered`/`Refused`/deep `Shade`) assignment in
    /// `settling` while keeping `v` bound in `env` for pure consumers.
    fn assign_non_literal(
        &mut self,
        x: &Expr,
        local: LocalId,
        value: &Expr,
        v: Val,
        env: &mut Env,
    ) -> Val {
        let val_res = Self::closed_residual(&v, value.span).unwrap_or_else(|| {
            let r = Self::residual(v.clone(), value);
            self.settling.extend(Self::settle(&[&r], env));
            r
        });
        let rebuilt = Expr {
            kind: ExprKind::Assign {
                place: Place { local, path: Vec::new() },
                value: Box::new(val_res.clone()),
            },
            ty: x.ty.clone(),
            depth: val_res.depth,
            span: x.span,
        };
        self.settling.push(Stmt::Expr(rebuilt));
        env.bind(local.0 as usize, Some(v));
        Val { kind: Kind::Known(ValLit::Unit), ty: Type::Unit, depth: Depth::PURE }
    }
}

const fn join_opt_depth(a: Option<Depth>, b: Option<Depth>) -> Option<Depth> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.join(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

fn unary(op: UnOp, v: &Val) -> Option<Literal> {
    Some(match op {
        // Arithmetic wraps. An implementation MUST NOT make overflow
        // undefined: a canonical encoding cannot be built on top of behaviour
        // that varies by compiler. `spec/05-types.md` §5.1.
        UnOp::Neg => Literal::Int(v.int()?.wrapping_neg()),
        UnOp::BitNot => Literal::Int(!v.int()?),
        UnOp::Not => Literal::Bool(!v.bool()?),
    })
}

fn binary(op: BinOp, a: &Val, b: &Val) -> Result<Option<Literal>, &'static str> {
    if let (Some(x), Some(y)) = (a.int(), b.int()) {
        return Ok(Some(match op {
            BinOp::Add => Literal::Int(x.wrapping_add(y)),
            BinOp::Sub => Literal::Int(x.wrapping_sub(y)),
            BinOp::Mul => Literal::Int(x.wrapping_mul(y)),
            // Dividing by zero cannot produce a value and never will.
            BinOp::Div | BinOp::Rem if y == 0 => return Err("this divides by zero"),
            BinOp::Div => Literal::Int(x.wrapping_div(y)),
            BinOp::Rem => Literal::Int(x.wrapping_rem(y)),
            BinOp::BitAnd => Literal::Int(x & y),
            BinOp::BitOr => Literal::Int(x | y),
            BinOp::BitXor => Literal::Int(x ^ y),
            BinOp::Shl => Literal::Int(x.wrapping_shl(shift(y)?)),
            BinOp::Shr => Literal::Int(x.wrapping_shr(shift(y)?)),
            BinOp::Eq => Literal::Bool(x == y),
            BinOp::Ne => Literal::Bool(x != y),
            BinOp::Lt => Literal::Bool(x < y),
            BinOp::Le => Literal::Bool(x <= y),
            BinOp::Gt => Literal::Bool(x > y),
            BinOp::Ge => Literal::Bool(x >= y),
        }));
    }
    // Compare known literals directly (avoiding O(len_a + len_b) BLAKE3 hashing
    // on mismatched Bytes/Str), and fall back to canonical cairn equality for
    // shades and answers. `spec/05-types.md` §5.3.
    if matches!(op, BinOp::Eq | BinOp::Ne) {
        if let (Kind::Known(la), Kind::Known(lb)) = (&a.kind, &b.kind) {
            return Ok(Some(Literal::Bool(matches!(op, BinOp::Eq) == (la == lb))));
        }
        if let (Some(va), Some(vb)) = (as_value(a), as_value(b)) {
            return Ok(Some(Literal::Bool(matches!(op, BinOp::Eq) == (va.cairn() == vb.cairn()))));
        }
    }
    Ok(None)
}

/// The ledger value a burial value is, when it is one.
fn as_value(v: &Val) -> Option<Value> {
    match &v.kind {
        Kind::Known(ValLit::Unit) => Some(Value::Unit),
        Kind::Known(ValLit::Bool(b)) => Some(Value::Bool(*b)),
        Kind::Known(ValLit::Int(n)) => Some(Value::Int(*n)),
        Kind::Known(ValLit::Bytes(b)) => Some(Value::Bytes(b.to_vec())),
        Kind::Known(ValLit::Str(s)) => Some(Value::Str(s.to_string())),
        Kind::Known(ValLit::Cairn(c)) => Some(Value::Cairn(Cairn::from_bytes(*c))),
        // Only the origin and the name survive, which is what a shade is.
        Kind::Shade(inner) => {
            let Type::Shade { origin, .. } = &v.ty else { return None };
            as_value(inner).map(|held| Value::Shade { origin: origin.get(), value: held.cairn() })
        }
        // §5.1.1 gives a refusal and an answer each a tag, so both are values
        // the ledger can hold.
        Kind::Known(ValLit::Refusal(r)) => Some(Value::Refusal(ledger_code(*r))),
        Kind::Refused(r) => {
            Some(Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(ledger_code(*r)))))
        }
        Kind::Answered(inner) => as_value(inner)
            .map(|held| Value::Answer(Box::new(nether_ledger::AnswerOf::Given(held)))),
        // A function and a prelude function have no written form at all.
        Kind::Func(_) | Kind::Prim(_) | Kind::Stuck(_) => None,
    }
}

/// §5.1.1's six, from the ledger's spelling to the calculus's.
///
/// Two enumerations of one closed set, because `nether-core` does not depend on
/// the ledger and must not. The order is §5.1.1's and both say so.
fn code(r: nether_ledger::Refusal) -> RefusalCode {
    match r {
        nether_ledger::Refusal::Absent => RefusalCode::Absent,
        nether_ledger::Refusal::Denied => RefusalCode::Denied,
        nether_ledger::Refusal::Malformed => RefusalCode::Malformed,
        nether_ledger::Refusal::Unreachable => RefusalCode::Unreachable,
        nether_ledger::Refusal::Exhausted => RefusalCode::Exhausted,
        nether_ledger::Refusal::Conflict => RefusalCode::Conflict,
    }
}

/// And back the other way.
fn ledger_code(r: RefusalCode) -> nether_ledger::Refusal {
    match r {
        RefusalCode::Absent => nether_ledger::Refusal::Absent,
        RefusalCode::Denied => nether_ledger::Refusal::Denied,
        RefusalCode::Malformed => nether_ledger::Refusal::Malformed,
        RefusalCode::Unreachable => nether_ledger::Refusal::Unreachable,
        RefusalCode::Exhausted => nether_ledger::Refusal::Exhausted,
        RefusalCode::Conflict => nether_ledger::Refusal::Conflict,
    }
}

/// A value the world said, as a literal, when it has a form.
fn literal(v: &Value) -> Option<Literal> {
    Some(match v {
        Value::Unit => Literal::Unit,
        Value::Bool(b) => Literal::Bool(*b),
        Value::Int(n) => Literal::Int(*n),
        Value::Bytes(b) => Literal::Bytes(b.clone()),
        Value::Str(s) => Literal::Str(s.clone()),
        Value::Cairn(c) => Literal::Cairn(*c.as_bytes()),
        Value::Refusal(r) => Literal::Refusal(code(*r)),
        // §04 has no syntax for an aggregate, a shade or an answer, so none of
        // them can be a literal. 0116.
        Value::Struct { .. } | Value::Array(_) | Value::Shade { .. } | Value::Answer(_) => {
            return None;
        }
    })
}

/// A shift by something no shift can mean (`< 0` or `>= 64`).
fn shift(n: i64) -> Result<u32, &'static str> {
    u32::try_from(n).ok().filter(|&s| s < 64).ok_or("this shifts by a width that is not one")
}

/// Byte work performed by a pure prelude operation for fuel accounting.
fn prim_byte_work(p: Prim, args: &[Val]) -> usize {
    let bytes = |i: usize| args.get(i).and_then(Val::bytes);
    match p {
        Prim::Concat => bytes(0).zip(bytes(1)).map_or(0, |(a, b)| a.len().saturating_add(b.len())),
        Prim::Slice => {
            match (bytes(0), args.get(1).and_then(Val::int), args.get(2).and_then(Val::int)) {
                (Some(b), Some(from), Some(to)) => usize::try_from(from)
                    .ok()
                    .zip(usize::try_from(to).ok())
                    .and_then(|(f, t)| b.get(f..t))
                    .map_or(0, <[u8]>::len),
                _ => 0,
            }
        }
        Prim::StartsWith => bytes(1).map_or(0, <[u8]>::len),
        Prim::Raw | Prim::CairnOf => bytes(0).map_or(0, <[u8]>::len),
        _ => 0,
    }
}

enum PrimErr {
    Collapsed(&'static str),
    TooLarge(usize),
}

/// The prelude functions burial can finish.
///
/// Every one of them is at depth 0 and produces something with a written form.
/// The rest either need the world, need the ledger, or produce an aggregate
/// that cannot be written down. `spec/09-prelude.md` §9.2.
fn prim(p: Prim, args: &[Val]) -> Result<Option<Literal>, PrimErr> {
    let int = |i: usize| args.get(i).and_then(Val::int);
    let bytes = |i: usize| args.get(i).and_then(Val::bytes);
    let two = |f: fn(i64, i64) -> i64| int(0).zip(int(1)).map(|(a, b)| Literal::Int(f(a, b)));
    Ok(match p {
        Prim::Min => two(i64::min),
        Prim::Max => two(i64::max),
        Prim::Abs => int(0).map(|n| Literal::Int(n.wrapping_abs())),
        Prim::Len => match bytes(0).map(|b| i64::try_from(b.len())) {
            Some(Ok(n)) => Some(Literal::Int(n)),
            Some(Err(_)) => return Err(PrimErr::Collapsed("this is longer than an I64 can count")),
            None => None,
        },
        Prim::Concat => match bytes(0).zip(bytes(1)) {
            Some((a, b)) => {
                let total = a
                    .len()
                    .checked_add(b.len())
                    .filter(|&n| n <= MAX_CONCAT)
                    .ok_or(PrimErr::TooLarge(MAX_CONCAT))?;
                let mut out = Vec::with_capacity(total);
                out.extend_from_slice(a);
                out.extend_from_slice(b);
                Some(Literal::Bytes(out))
            }
            None => None,
        },
        Prim::Slice => match (bytes(0), int(1), int(2)) {
            (Some(b), Some(from), Some(to)) => {
                let range = usize::try_from(from).ok().zip(usize::try_from(to).ok());
                // An out-of-range slice cannot produce a value and never will.
                // `spec/09-prelude.md` §9.9.
                match range.and_then(|(f, t)| b.get(f..t)) {
                    Some(cut) => Some(Literal::Bytes(cut.to_vec())),
                    None => {
                        return Err(PrimErr::Collapsed("this slice is outside what it is slicing"));
                    }
                }
            }
            _ => None,
        },
        Prim::StartsWith => bytes(0).zip(bytes(1)).map(|(b, p)| Literal::Bool(b.starts_with(p))),
        Prim::Raw => bytes(0).map(|b| Literal::Bytes(b.to_vec())),
        Prim::CairnOf => {
            bytes(0).map(|b| Literal::Cairn(*Value::Bytes(b.to_vec()).cairn().as_bytes()))
        }
        Prim::Hex => match args.first().map(|v| &v.kind) {
            Some(Kind::Known(ValLit::Cairn(c))) => {
                Some(Literal::Str(Cairn::from_bytes(*c).to_string()))
            }
            _ => None,
        },
        _ => None,
    })
}
