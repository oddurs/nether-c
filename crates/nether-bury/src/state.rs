use std::collections::{HashMap, HashSet};
use std::sync::Arc;
#[cfg(not(target_family = "wasm"))]
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering::Relaxed};
#[cfg(not(target_family = "wasm"))]
use std::sync::{Condvar, Mutex, MutexGuard, PoisonError};

use nether_core::{
    Block, Demand, Depth, Expr, ExprKind, FuncDef, FuncId, GlobalId, Literal, LocalId, Prim,
    Refusal as RefusalCode, Span, Stmt, Type, Unit,
};
use nether_ledger::{Cairn, Node, Stored, Stratum, Value};

#[cfg(not(target_family = "wasm"))]
use crate::residue::STACK;
use crate::residue::{Grinding, Halt, HaltKind, Residue};
use crate::{Answers, analysis, u32_index};

/// Maximum default worker count when `bury_with` uses host parallelism.
#[cfg(not(target_family = "wasm"))]
pub(crate) const MAX_DEFAULT_THREADS: usize = 16;

/// Dense `u32`-ID set backed by `Vec<bool>` so WASM avoids `BTreeSet`/`HashSet<u32>`.
#[derive(Clone, Default)]
pub(crate) struct LocalSet(Vec<bool>);

impl LocalSet {
    pub(crate) fn insert(&mut self, id: u32) -> bool {
        let i = id as usize;
        if i >= self.0.len() {
            self.0.resize(i + 1, false);
        }
        let was = self.0[i];
        self.0[i] = true;
        !was
    }

    pub(crate) fn contains(&self, id: u32) -> bool {
        self.0.get(id as usize).copied().unwrap_or(false)
    }

    pub(crate) fn is_empty(&self) -> bool {
        !self.0.iter().any(|&b| b)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = u32> + '_ {
        self.0.iter().enumerate().filter(|&(_, &b)| b).map(|(i, _)| u32_index(i))
    }

    pub(crate) fn any(&self, f: impl Fn(u32) -> bool) -> bool {
        self.iter().any(f)
    }
}

/// Dense `u32`-ID map backed by `Vec<Option<T>>` so WASM avoids `HashMap<u32, _>`.
#[derive(Clone, PartialEq, Eq)]
pub(crate) struct LocalMap<T>(Vec<Option<T>>);

impl<T> Default for LocalMap<T> {
    fn default() -> Self {
        Self(Vec::new())
    }
}

impl<T> LocalMap<T> {
    pub(crate) fn get(&self, id: u32) -> Option<&T> {
        self.0.get(id as usize).and_then(Option::as_ref)
    }

    pub(crate) fn insert(&mut self, id: u32, val: T) {
        let i = id as usize;
        if i >= self.0.len() {
            self.0.resize_with(i + 1, || None);
        }
        self.0[i] = Some(val);
    }

    pub(crate) fn remove(&mut self, id: u32) {
        if let Some(slot) = self.0.get_mut(id as usize) {
            *slot = None;
        }
    }

    pub(crate) fn is_empty(&self) -> bool {
        self.0.iter().all(Option::is_none)
    }

    pub(crate) fn iter(&self) -> impl Iterator<Item = (u32, &T)> {
        self.0.iter().enumerate().filter_map(|(i, v)| v.as_ref().map(|v| (u32_index(i), v)))
    }
}

impl LocalMap<Depth> {
    pub(crate) fn join(&mut self, id: u32, d: Depth) {
        let next = self.get(id).map_or(d, |was| was.join(d));
        self.insert(id, next);
    }
}

/// One demand, between the serial order and the workers: nobody has it yet,
/// somebody is burying it, or a worker finished and this is what it left.
#[cfg(not(target_family = "wasm"))]
enum Slot {
    Free,
    Taken,
    Done(Option<Residue>),
}

/// A slot per demand behind one lock, and the condition the serial order waits
/// on when a worker has the demand it reached. One allocation for every
/// demand, rather than a channel and a mutex each: most demands never meet a
/// worker. One lock, because a `Condvar` waits with exactly one. 0291.
#[cfg(not(target_family = "wasm"))]
struct Handoff(Mutex<Vec<Slot>>, Condvar);

#[cfg(not(target_family = "wasm"))]
impl Handoff {
    fn new(len: usize) -> Self {
        Self(Mutex::new((0..len).map(|_| Slot::Free).collect()), Condvar::new())
    }

    fn lock(&self) -> MutexGuard<'_, Vec<Slot>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    fn wait<'a>(&self, guard: MutexGuard<'a, Vec<Slot>>) -> MutexGuard<'a, Vec<Slot>> {
        self.1.wait(guard).unwrap_or_else(PoisonError::into_inner)
    }

    fn complete(&self, index: usize, got: Option<Residue>) {
        self.lock()[index] = Slot::Done(got);
        self.1.notify_all();
    }
}

/// Guarantees a claimed demand slot is released with `Slot::Done(None)` if a
/// worker panics, preventing `demands()` from waiting forever.
#[cfg(not(target_family = "wasm"))]
struct SlotGuard<'a> {
    handoff: &'a Handoff,
    index: usize,
    armed: bool,
}

#[cfg(not(target_family = "wasm"))]
impl<'a> SlotGuard<'a> {
    fn new(handoff: &'a Handoff, index: usize) -> Self {
        Self { handoff, index, armed: true }
    }

    fn finish(mut self, got: Option<Residue>) {
        self.armed = false;
        self.handoff.complete(self.index, got);
    }
}

#[cfg(not(target_family = "wasm"))]
impl Drop for SlotGuard<'_> {
    fn drop(&mut self) {
        if self.armed {
            self.handoff.complete(self.index, None);
        }
    }
}

/// Reference-counted literal stored inside [`Val`] so cloning environments or
/// values with multi-megabyte `Bytes`/`Str` payloads stays O(1).
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum ValLit {
    Unit,
    Bool(bool),
    Int(i64),
    Bytes(Arc<[u8]>),
    Str(Arc<str>),
    Cairn([u8; 32]),
    Refusal(RefusalCode),
}

impl ValLit {
    pub(crate) fn from_literal(l: Literal) -> Self {
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

    pub(crate) fn to_literal(&self) -> Literal {
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
pub(crate) struct Val {
    pub(crate) kind: Kind,
    pub(crate) ty: Type,
    pub(crate) depth: Depth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Kind {
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
    pub(crate) fn is_stuck(&self) -> bool {
        matches!(self.kind, Kind::Stuck(_))
    }

    pub(crate) fn is_settleable(&self) -> bool {
        matches!(self.kind, Kind::Known(_) | Kind::Func(_) | Kind::Prim(_) | Kind::Shade(_))
    }

    pub(crate) fn int(&self) -> Option<i64> {
        match &self.kind {
            Kind::Known(ValLit::Int(n)) => Some(*n),
            _ => None,
        }
    }

    pub(crate) fn bool(&self) -> Option<bool> {
        match &self.kind {
            Kind::Known(ValLit::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    pub(crate) fn bytes(&self) -> Option<&[u8]> {
        match &self.kind {
            Kind::Known(ValLit::Bytes(b)) => Some(b),
            Kind::Known(ValLit::Str(s)) => Some(s.as_bytes()),
            _ => None,
        }
    }

    pub(crate) fn known(kind: Literal, x: &Expr) -> Self {
        Self { kind: Kind::Known(ValLit::from_literal(kind)), ty: x.ty.clone(), depth: Depth::PURE }
    }

    pub(crate) const fn unit() -> Self {
        Self { kind: Kind::Known(ValLit::Unit), ty: Type::Unit, depth: Depth::PURE }
    }

    pub(crate) fn stuck(x: &Expr) -> Self {
        Self { kind: Kind::Stuck(Box::new(x.clone())), ty: x.ty.clone(), depth: x.depth }
    }

    /// A residual expression with a new shape, built from reduced parts.
    pub(crate) fn rebuild(kind: ExprKind, x: &Expr, depth: Depth) -> Self {
        let e = Expr { kind, ty: x.ty.clone(), depth, span: x.span };
        Self { kind: Kind::Stuck(Box::new(e)), ty: x.ty.clone(), depth }
    }

    pub(crate) fn stuck_local(id: LocalId, ty: Type, depth: Depth, span: Span) -> Self {
        let e = Expr { kind: ExprKind::Local(id), ty: ty.clone(), depth, span };
        Self { kind: Kind::Stuck(Box::new(e)), ty, depth }
    }
}

/// A jump that is in flight.
///
/// Held on the burial rather than returned, because every form has to be able
/// to stop when one is set and threading it through each return type says the
/// same thing four times.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum Flow {
    Broke,
    Continued,
    Returned(Val),
}

/// One open loop or call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct Grind {
    pub(crate) span: Span,
    pub(crate) what: Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Open {
    Loop { turns: u64 },
    Call(FuncId),
}

/// A frame's locals, and which of them a write the residue does not hold has
/// changed since the residue last said what they are.
///
/// A statement that only assigns is not in the residue: its value is here and
/// nowhere else. That is fine until code is left as written and reads the
/// local in the residue, and then the residue has to be told. 0285.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Env {
    pub(crate) slots: Vec<Option<Val>>,
    pub(crate) stale: Vec<bool>,
    pub(crate) write_gen: Vec<u64>,
    pub(crate) writes: u64,
}

impl Env {
    pub(crate) fn new(locals: usize) -> Self {
        Self {
            slots: vec![None; locals],
            stale: vec![false; locals],
            write_gen: vec![0; locals],
            writes: 0,
        }
    }

    pub(crate) fn get(&self, id: usize) -> Option<&Option<Val>> {
        self.slots.get(id)
    }

    /// A binding the residue holds as it is here.
    pub(crate) fn bind(&mut self, id: usize, v: Option<Val>) {
        if let (Some(slot), Some(stale)) = (self.slots.get_mut(id), self.stale.get_mut(id)) {
            self.writes = self.writes.wrapping_add(1);
            (*slot, *stale) = (v, false);
        }
    }

    /// A write the residue does not hold.
    pub(crate) fn write(&mut self, id: usize, v: Val) {
        if let (Some(slot), Some(stale), Some(wgen)) =
            (self.slots.get_mut(id), self.stale.get_mut(id), self.write_gen.get_mut(id))
        {
            self.writes = self.writes.wrapping_add(1);
            (*slot, *stale, *wgen) = (Some(v), true, self.writes);
        }
    }
}

/// Snapshot of mutable burial side effects that speculative or abandoned
/// reductions (loops, branches, calls, speculation) must roll back together.
pub(crate) struct Checkpoint {
    pub(crate) deposits: usize,
    named: usize,
    specialised: usize,
    pub(crate) settling: usize,
    globals: Vec<Option<Val>>,
    globals_residual: Vec<Option<Expr>>,
    served: HashMap<nether_ledger::Call, usize>,
    acts_served: usize,
    answers_used: usize,
    pub(crate) starved: Option<Block>,
    pub(crate) flow: Option<Flow>,
    pub(crate) pc: Depth,
    act_blocked: bool,
}

pub(crate) struct Burial<'a> {
    pub(crate) unit: &'a Unit,
    /// What the world already said. See [`Answers`].
    pub(crate) answers: &'a Answers,
    /// The cairn of the bytes the unit was lowered from.
    pub(crate) source: Cairn,
    pub(crate) left: u64,
    pub(crate) spent: u64,
    /// Unit-level bindings, evaluated the first time something reaches one.
    pub(crate) globals: Vec<Option<Val>>,
    /// Reduced global initializers written back into `Residue::as_unit`.
    pub(crate) globals_residual: Vec<Option<Expr>>,
    /// Unit-level bindings currently being evaluated, to detect cycles.
    pub(crate) eval_globals: Vec<GlobalId>,
    /// Current expression recursion depth, guarded by `MAX_EXPR_DEPTH`.
    pub(crate) expr_depth: u32,
    /// A `break`, `continue` or `return` that has happened and not yet landed.
    pub(crate) flow: Option<Flow>,
    /// Current control-dependence `pc` from enclosing residual control flow.
    pub(crate) pc: Depth,
    /// The loops and calls currently open, innermost last. Nothing pops it on
    /// the way out of an error, which is the point: when the budget runs out
    /// this is what evaluation was going round in.
    pub(crate) grind: Vec<Grind>,
    /// Everything named so far, in the order it was named.
    pub(crate) named: Vec<(Cairn, Stored)>,
    /// The same names, for asking whether one is already there.
    pub(crate) known: HashSet<Cairn>,
    /// The holes, in the order they were found.
    pub(crate) holes: Vec<Cairn>,
    /// What the program deposited, in source order.
    pub(crate) deposits: Vec<Cairn>,
    /// The functions minted for calls that starved. §6.5.
    pub(crate) specialised: Vec<FuncDef>,
    /// The same functions, by what they were specialised to, so that a
    /// function specialised twice to the same constants is minted once.
    pub(crate) minted: HashMap<(FuncId, Vec<Literal>), FuncId>,
    /// Whether this is an arm of a branch the world has not decided. §6.5:
    /// such a reduction may produce a value or stop, and nothing else.
    pub(crate) speculating: bool,
    /// Assignments code left as written needs before it, waiting for the
    /// block whose statement that code is in.
    pub(crate) settling: Vec<Stmt>,
    /// Locals bound or assigned a non-closed value (`Answer`, deep `Shade`,
    /// etc.) while depositing, mutating `env`, or serving an act.
    pub(crate) nonclosed_dirty: Vec<u32>,
    /// Whether this is a demand sent ahead of the serial order that has not
    /// yet reached anything another demand could change. §6.2.1.
    pub(crate) ahead: bool,
    /// Raised when the serial order is done with a worker's demand, whatever
    /// became of it. A worker stops at its next step rather than spending the
    /// rest of its fuel on a demand nobody will take. Only workers have one.
    #[cfg(not(target_family = "wasm"))]
    stop: Option<Arc<AtomicBool>>,
    /// Remaining serial fuel, shared with workers so they bail out once their
    /// speculative spend can no longer fit in the serial budget.
    #[cfg(not(target_family = "wasm"))]
    serial_left: Option<Arc<AtomicU64>>,
    /// The block the body just starved in, on the way out of `Burial::block`.
    pub(crate) starved: Option<Block>,
    /// The holes already dug, by the question each one asks (read strata only).
    pub(crate) asked: HashMap<nether_ledger::Call, Cairn>,
    /// How many times each act call has already been answered in this burial.
    pub(crate) served: HashMap<nether_ledger::Call, usize>,
    /// Monotonic total of act-stratum answers served (mirrors sum of `served`).
    pub(crate) acts_served: usize,
    /// Total answers (read and act) consumed from `self.answers` in non-rolled-back paths.
    pub(crate) answers_used: usize,
    /// Number of act-stratum answers reserved for earlier rolled-back residuals
    /// so later demands index past them instead of stealing or duplicating acts.
    pub(crate) reserved_acts: HashMap<nether_ledger::Call, usize>,
    /// Raised once an earlier demand or statement starves with an unresolved
    /// residual that can reach an act stratum; blocks later act calls from
    /// serving or digging out of source order (§6.3).
    pub(crate) act_blocked: bool,
}

impl<'a> Burial<'a> {
    pub(crate) fn new(
        unit: &'a Unit,
        source: Cairn,
        fuel: u64,
        answers: &'a Answers,
        ahead: bool,
    ) -> Self {
        Self {
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
            acts_served: 0,
            answers_used: 0,
            reserved_acts: HashMap::new(),
            act_blocked: false,
            specialised: Vec::new(),
            minted: HashMap::new(),
            starved: None,
            speculating: false,
            settling: Vec::new(),
            nonclosed_dirty: Vec::new(),
            ahead,
            #[cfg(not(target_family = "wasm"))]
            stop: None,
            #[cfg(not(target_family = "wasm"))]
            serial_left: None,
        }
    }

    pub(crate) fn checkpoint(&mut self) -> Checkpoint {
        Checkpoint {
            deposits: self.deposits.len(),
            named: self.named.len(),
            specialised: self.specialised.len(),
            settling: self.settling.len(),
            globals: self.globals.clone(),
            globals_residual: self.globals_residual.clone(),
            served: self.served.clone(),
            acts_served: self.acts_served,
            answers_used: self.answers_used,
            starved: self.starved.take(),
            flow: self.flow.clone(),
            pc: self.pc,
            act_blocked: self.act_blocked,
        }
    }

    pub(crate) const fn acts_served_since(&self, cp: &Checkpoint) -> bool {
        self.acts_served != cp.acts_served
    }

    pub(crate) fn had_effects(&self, cp: &Checkpoint) -> bool {
        self.deposits.len() > cp.deposits || self.acts_served_since(cp)
    }

    pub(crate) fn restore_frame(&mut self, cp: Checkpoint) {
        self.starved = cp.starved;
        self.settling.truncate(cp.settling);
        self.flow = cp.flow;
        self.pc = cp.pc;
    }

    pub(crate) fn rollback(&mut self, cp: Checkpoint) {
        let rolled_back_effects = self.had_effects(&cp);
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
        if self.acts_served_since(&cp) {
            for (call, &now_count) in &self.served {
                let prev = cp.served.get(call).copied().unwrap_or(0);
                if now_count > prev {
                    *self.reserved_acts.entry(call.clone()).or_default() += now_count - prev;
                }
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
                let expr = Expr {
                    kind: ExprKind::Global(GlobalId(u32_index(i))),
                    ty: g.ty.clone(),
                    depth: g.value.depth,
                    span: g.span,
                };
                *slot = Some(Val::stuck(&expr));
            } else {
                *slot = prev;
            }
        }
        self.globals_residual = cp.globals_residual;
        self.served = cp.served;
        self.acts_served = cp.acts_served;
        self.answers_used = cp.answers_used;
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

    fn demand_serial(&mut self, d: &Demand) -> Result<Expr, Halt> {
        self.flow = None;
        self.pc = Depth::PURE;
        self.settling.clear();
        let cp = self.checkpoint();
        match self.expr(&d.value, &mut Env::new(self.unit.locals.len())) {
            Ok(v) => {
                let non_closed_effects = !v.is_stuck()
                    && Self::closed_residual(&v, d.span).is_none()
                    && self.had_effects(&cp);
                if non_closed_effects {
                    if !self.acts_served_since(&cp) {
                        self.rollback(cp);
                    }
                    if analysis::reaches_act(&d.value, self.unit) {
                        self.act_blocked = true;
                    }
                    Ok(d.value.clone())
                } else {
                    Ok(Self::residual(v, &d.value))
                }
            }
            Err(halt) => Err(self.blame(halt)),
        }
    }

    #[cfg(target_family = "wasm")]
    pub(crate) fn burrow(mut self, _threads: usize) -> Result<Residue, Halt> {
        let mut demands = Vec::with_capacity(self.unit.demands.len());
        for d in &self.unit.demands {
            demands.push(self.demand_serial(d)?);
        }
        Ok(self.finish(demands))
    }

    /// Bury every demand, on this thread and up to `threads - 1` others.
    #[cfg(not(target_family = "wasm"))]
    pub(crate) fn burrow(self, threads: usize) -> Result<Residue, Halt> {
        let workers = threads.min(self.unit.demands.len()).saturating_sub(1);
        if workers == 0 {
            return self.demands(None, None);
        }
        let seed = Burial::new(self.unit, self.source, self.left, self.answers, true);
        let handoff = Handoff::new(self.unit.demands.len());
        // The first demand is the serial order's at once, so a worker starts
        // at the second, and stops when this runs past the last.
        let next = AtomicUsize::new(1);
        let stop = Arc::new(AtomicBool::new(false));
        let serial_left = Arc::new(AtomicU64::new(self.left));
        // Demands buried ahead that came to nothing, in a row. Once every
        // worker has come to nothing twice running, the serial order does the
        // rest faster alone. 0291.
        let (misses, enough) = (AtomicUsize::new(0), 2 * workers);
        std::thread::scope(|s| {
            for _ in 0..workers {
                let work = || {
                    while misses.load(Relaxed) < enough {
                        let i = next.fetch_add(1, Relaxed);
                        {
                            let mut slots = handoff.lock();
                            match slots.get_mut(i) {
                                None => break,
                                Some(slot @ Slot::Free) => *slot = Slot::Taken,
                                Some(_) => continue,
                            }
                        }
                        let guard = SlotGuard::new(&handoff, i);
                        let got = seed.bury_ahead(i, &stop, &serial_left);
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
    #[cfg(not(target_family = "wasm"))]
    fn bury_ahead(
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
            && (!b.deposits.is_empty() || b.acts_served > 0)
        {
            return None;
        }
        b.ahead.then(|| b.finish(vec![Self::residual(v, &d.value)]))
    }

    /// Every demand, in source order, which §6.2 fixes.
    #[cfg(not(target_family = "wasm"))]
    fn demands(
        mut self,
        handoff: Option<&Handoff>,
        serial_left: Option<&Arc<AtomicU64>>,
    ) -> Result<Residue, Halt> {
        let mut demands = Vec::with_capacity(self.unit.demands.len());
        for (i, d) in self.unit.demands.iter().enumerate() {
            // Whoever reaches a demand first takes it, so nothing waits on a
            // demand no worker started.
            let got = handoff.and_then(|h| {
                let mut slots = h.lock();
                if matches!(slots.get(i)?, Slot::Free) {
                    slots[i] = Slot::Taken;
                    return None;
                }
                while matches!(slots[i], Slot::Taken) {
                    slots = h.wait(slots);
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
                    self.answers_used += e.served;
                    self.named.extend(e.named.into_iter().filter(|(c, _)| self.known.insert(*c)));
                    self.deposits.extend(e.deposits);
                    demands.extend(e.demands);
                }
                _ => demands.push(self.demand_serial(d)?),
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
            served: self.answers_used,
            depth,
            globals: self.globals_residual,
        }
    }

    /// The demand reached something demands share: a unit-level binding, the
    /// world, or a minted function. Sent ahead, it is no use, so it stops.
    /// `Burial::bury_ahead` turns the halt into no early result. 0291.
    pub(crate) fn shares(&mut self) -> Result<(), Halt> {
        if !self.ahead {
            return Ok(());
        }
        (self.ahead, self.left) = (false, 0);
        Err(Halt::at(Span::default(), HaltKind::OutOfFuel { spent: self.spent }))
    }

    /// Write a node down and hand back its name.
    pub(crate) fn remember(&mut self, what: Stored) -> Cairn {
        let cairn = what.cairn();
        if self.known.insert(cairn) {
            self.named.push((cairn, what));
        }
        cairn
    }

    fn ledger_span(&self, span: Span) -> nether_ledger::Span {
        let start = u64::from(span.start);
        let end = u64::from(span.end);
        nether_ledger::Span::new(self.source, start, end)
            .unwrap_or_else(|_| nether_ledger::Span::point(self.source, start))
    }

    /// A question for the world, named and written down. §6.3.
    pub(crate) fn dig(&mut self, p: Prim, call: nether_ledger::Call, span: Span) -> Cairn {
        // §6.3: two holes with identical calls at a read stratum MUST be the
        // same hole. At 4, 6, 7 and 8 the second ask is a second act.
        let shared = p.latent().reads();
        if shared && let Some(dug) = self.asked.get(&call) {
            return *dug;
        }
        let node = Node::Hole {
            call: call.clone(),
            stratum: Stratum::new(p.latent().get()).unwrap_or(Stratum::MAX),
            span: self.ledger_span(span),
        };
        let cairn = self.remember(Stored::Node(node));
        if shared {
            self.asked.insert(call, cairn);
        }
        self.holes.push(cairn);
        cairn
    }

    /// A value the program left behind, named and written down. §4.7.
    pub(crate) fn deposit(&mut self, value: Value, span: Span) -> Cairn {
        let at = self.remember(Stored::Value(value));
        let node = Node::Deposit { value: at, span: self.ledger_span(span) };
        self.remember(Stored::Node(node))
    }

    /// Where a halt should point, and what it was going round in.
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

    #[cfg(not(target_family = "wasm"))]
    fn stopped(&self, steps: u64) -> bool {
        self.stop.as_deref().is_some_and(|s| s.load(Relaxed))
            || self
                .serial_left
                .as_deref()
                .is_some_and(|sl| sl.load(Relaxed).saturating_sub(self.spent) < steps)
    }

    #[cfg(target_family = "wasm")]
    const fn stopped(&self, _steps: u64) -> bool {
        false
    }

    pub(crate) fn charge(&mut self, steps: u64, span: Span) -> Result<(), Halt> {
        if self.left < steps || self.stopped(steps) {
            return Err(Halt::at(span, HaltKind::OutOfFuel { spent: self.spent }));
        }
        self.left -= steps;
        self.spent = self.spent.saturating_add(steps);
        Ok(())
    }

    /// Charge fuel proportional to large byte allocations/copies/hashes
    /// (`bytes / 64`, so small values < 64 B cost 0 extra fuel while multi-MiB
    /// concat/deposit/seal amplification hits `OutOfFuel` cleanly).
    pub(crate) fn burn_bytes(&mut self, bytes: usize, span: Span) -> Result<(), Halt> {
        let extra = u64::try_from(bytes / 64).unwrap_or(u64::MAX);
        if extra == 0 {
            return Ok(());
        }
        self.charge(extra, span)
    }

    /// Recursive byte payload size of a value (including inside `Shade`/`Answered`)
    /// so sealing, depositing, equality, and world-call naming charge proportionally
    /// to hashing/encoding work.
    pub(crate) fn val_byte_work(v: &Val) -> usize {
        match &v.kind {
            Kind::Known(ValLit::Bytes(b)) => b.len(),
            Kind::Known(ValLit::Str(s)) => s.len(),
            Kind::Shade(inner) | Kind::Answered(inner) => Self::val_byte_work(inner),
            _ => 0,
        }
    }

    pub(crate) fn depositable_work(v: &Val) -> Option<usize> {
        match &v.kind {
            Kind::Func(_) | Kind::Prim(_) | Kind::Stuck(_) => None,
            Kind::Shade(inner) | Kind::Answered(inner) => {
                Self::depositable_work(inner).map(|w| w.saturating_add(64))
            }
            _ => Some(Self::val_byte_work(v).saturating_add(16)),
        }
    }

    pub(crate) fn is_stuck_return(&self, ret: &Val, span: Span) -> bool {
        ret.is_stuck() || (self.speculating && Self::closed_residual(ret, span).is_none())
    }
}
