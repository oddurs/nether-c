use std::collections::HashMap;

use nether_core::{
    Block, Demand, Depth, Diagnostic, Expr, ExprKind, FuncDef, Literal, LocalDef, LocalId, Prim,
    Rite, Span, Stmt, Type, Unit, grouped,
};
use nether_ledger::{Cairn, Node, Stored, Value};

use crate::{assign_local, tail_block, u32_index, unit_block};

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
    /// Number of world answers consumed from `answers` during this burial.
    pub served: usize,
    /// The deepest stratum anything in the residue still reaches.
    ///
    /// Not `Trace`'s `depth`, which is the deepest stratum a witness *reached*
    /// — `spec/07-ledger.md` §7.3.2. A residue that has been answered nothing
    /// still reaches deep; a trace that has done nothing has not.
    pub depth: Depth,
    /// Reduced initializers for unit-level globals evaluated (or partially
    /// reduced with side effects) during this burial, applied by [`Residue::as_unit`].
    pub(crate) globals: Vec<Option<Expr>>,
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
    /// adds them at the end so that a `FuncId` means the same thing in the
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
        assert!(
            self.demands.len() == buried.demands.len(),
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
        let assign_orig = assign_local(id, orig.clone(), span);
        let guard = Expr {
            kind: ExprKind::Select {
                cond: Box::new(Expr {
                    kind: ExprKind::Literal(Literal::Bool(false)),
                    ty: Type::Bool,
                    depth: Depth::PURE,
                    span,
                }),
                then: Box::new(unit_block(vec![Stmt::Expr(assign_orig)], span)),
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
        tail_block(
            vec![Stmt::Let { local: id, value: reduced.clone() }, Stmt::Expr(guard)],
            tail,
            span,
        )
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
        let id = LocalId(u32_index(locals.len()));
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
            let (span, ty, depth) = (orig.span, orig.ty.clone(), orig.depth);
            let id = Self::fresh_local("__r", &ty, span, locals);
            let tail = Expr { kind: ExprKind::Local(id), ty, depth, span };
            *x = tail_block(vec![Stmt::Let { local: id, value: orig }], tail, span);
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
        fn node(s: &Stored) -> Option<&Node> {
            match s {
                Stored::Node(n) => Some(n),
                Stored::Value(_) => None,
            }
        }
        if self.holes.len() <= 8 {
            return self.holes.iter().filter_map(|c| self.get(*c).and_then(node)).collect();
        }
        let by_cairn: HashMap<Cairn, &Stored> = self.named.iter().map(|(c, s)| (*c, s)).collect();
        self.holes.iter().filter_map(|c| by_cairn.get(c).copied().and_then(node)).collect()
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

/// The ways a burial does not finish.
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

/// Maximum total expression recursion depth across nested evaluation frames.
/// Scaled above `MAX_FRAMES` so recursive function calls always hit `MAX_FRAMES`
/// first, while pathological non-call AST nesting stops before stack overflow.
pub(crate) const MAX_EXPR_DEPTH: u32 = MAX_FRAMES * 8;

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
    pub(crate) const fn at(span: Span, kind: HaltKind) -> Self {
        Self { span, kind, grinding: None }
    }

    pub(crate) const fn collapsed(span: Span, why: &'static str) -> Self {
        Self::at(span, HaltKind::Collapsed(why))
    }

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
