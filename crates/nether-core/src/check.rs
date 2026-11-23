//! The depth checker: `spec/02-calculus.md`, and nothing else.
//!
//! Eleven rules. The checker derives a depth for every expression and compares
//! it against the depth the IR states, so a node that claims a depth the
//! calculus does not produce is a fault rather than a fact.
//!
//! Two premises are enforced, because they are the two the specification
//! writes down: [APP]'s `dƒ ⊆ δ` and [LOOK]'s `d ∈ δ`. Inside a function body
//! the first is not a fault but a question for the caller — it is where a
//! latent depth comes from — while the second stays local, because §1.6 says
//! the Orpheus check does not propagate. The
//! ambient bound is otherwise a theorem rather than a check (§2.4) — a descent
//! concludes at the depth it reached, which is the whole reason it exists.
//!
//! Forms the specification gives no rule for — indexing, projection, loops,
//! branches, blocks — compose by [PRIM]: the maximum of their parts. That is
//! the only rule available and it is the conservative one.
//!
//! The walk is recursive and has no bound of its own. It inherits
//! `nether-syntax`'s `MAX_NESTING`, because the IR it is given is the IR
//! lowering produced from a parse — and a caller that hand-builds something
//! deeper than the surface can express is a caller with a bug rather than a
//! limit to report. §6.4.
//!
//! This checks depth. It is not a type checker: it reads a type exactly where
//! a depth rule depends on one, which is a shade's origin, an arrow's latent
//! depth, and what the four rites produce.

use core::fmt;

use crate::depth::{Capability, Depth, Held};
use crate::ir::{Block, Expr, ExprKind, GlobalId, LocalId, Proj, Rite, Span, Stmt};
use crate::prim::Prim;
use crate::report::{Cause, Diagnostic};
use crate::ty::Type;
use crate::unit::{Asserted, FuncDef, Unit};

/// Upper bound on total loop fixpoint re-walks per unit to prevent adversarial
/// exponential blowup on deeply nested loops.
const MAX_FIXPOINT_STEPS: usize = 4096;

/// Something the calculus does not derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Where it was written.
    pub span: Span,
    /// What is wrong.
    pub kind: FaultKind,
    /// What made the value as deep as it is, when that is a different place
    /// from the fault and worth pointing at.
    pub blame: Option<Blame>,
}

/// The call that took a value to the stratum it is at.
///
/// §2.4 promises this is findable by construction: a value at depth 5 means
/// some `descend net` is responsible. This is the call inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blame {
    /// Where the call is.
    pub span: Span,
    /// What it called.
    pub what: String,
}

/// What kind of thing is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// [APP]: an application reaches deeper than what is held here.
    Ungranted {
        /// `dƒ`, the latent depth of the arrow: what applying it reaches.
        needed: Depth,
        /// `δ`.
        held: Held,
    },
    /// [LOOK]: the Orpheus rule. You may carry a shade up out of any stratum;
    /// you may only look at it by going back down.
    Orpheus {
        /// The stratum the shade came from.
        origin: Depth,
        /// `δ`, which does not hold it.
        held: Held,
    },
    /// The IR states a latent set the eleven rules do not produce. [ABS]'s
    /// half of `Stated`, and not a mistake a program can make.
    StatedLatent {
        /// What the rules give.
        derived: Held,
        /// What the node claims.
        stated: Held,
    },
    /// A latent set written after a signature that inference disagrees with.
    /// [ABS]'s half of `Asserted`. §5.6: a written depth is a checked
    /// assertion, not a coercion.
    AssertedLatent {
        /// What the rules give.
        derived: Held,
        /// What the programmer wrote.
        asserted: Held,
    },
    /// The IR states a depth the eleven rules do not produce. Not a mistake a
    /// program can make: a mistake whatever built the IR made.
    Stated {
        /// What the rules give.
        derived: Depth,
        /// What the node claims.
        stated: Depth,
    },
    /// A written `@d` disagrees with inference. An assertion, never a
    /// coercion. `spec/05-types.md` §5.6.
    Asserted {
        /// What the rules give.
        derived: Depth,
        /// What was written.
        asserted: Depth,
    },
    /// The IR is not well-formed enough to have a judgement at all.
    Malformed(&'static str),
    /// A local was assigned after its value had been named.
    ///
    /// Once a value has been used as a value — sealed, shaded, deposited,
    /// returned, or passed as an argument — its cairn exists, and nothing can
    /// change what a cairn names. `spec/05-types.md` §5.4.
    Frozen {
        /// Where it was named. The interesting place is almost never the
        /// assignment; it is the line that gave the value a name.
        named: Span,
    },
}

impl Fault {
    /// This fault, ready to print.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            span: self.span,
            headline: self.to_string(),
            label: self.label(),
            cause: self.cause(),
            note: self.note(),
        }
    }

    /// What goes at the end of the caret row, about the thing underlined.
    fn label(&self) -> Option<String> {
        let blame = self.blame.as_ref()?;
        match self.kind {
            FaultKind::Orpheus { origin, .. } => {
                Some(format!("this shade came from `{}` at stratum {origin}", blame.what))
            }
            FaultKind::Ungranted { needed, .. } => {
                Some(format!("`{}` reaches stratum {needed}", blame.what))
            }
            _ => None,
        }
    }

    /// The other end of the error: the line the depth came from.
    ///
    /// Only an assertion gets one. The other two that carry blame say it on
    /// the caret row, because for them the cause and the symptom are the same
    /// expression; an annotation is about a number that came from somewhere
    /// else, which is the whole reason it is hard to phrase.
    fn cause(&self) -> Option<Cause> {
        let blame = self.blame.as_ref()?;
        let label = match self.kind {
            FaultKind::Asserted { derived, .. } => {
                format!("`{}` reaches stratum {derived}", blame.what)
            }
            FaultKind::AssertedLatent { derived, asserted } => {
                let extra = derived.without(asserted);
                let stratum = if extra.is_none() { derived.deepest() } else { extra.deepest() };
                format!("`{}` reaches stratum {stratum}", blame.what)
            }
            _ => return None,
        };
        Some(Cause { span: blame.span, label })
    }

    /// The line after the gap, which says what to do rather than what is wrong.
    fn note(&self) -> Option<String> {
        match self.kind {
            FaultKind::Orpheus { origin, .. } => Capability::at(origin).map(|c| {
                format!(
                    "the value is here, but you are not. Wrap the look in `descend {c} {{ … }}`."
                )
            }),
            FaultKind::Ungranted { needed, .. } => {
                Capability::at(needed).map(|c| format!("wrap it in `descend {c} {{ … }}`."))
            }
            FaultKind::Asserted { derived, .. } => {
                Some(format!("inference is not a coercion. Write `@{derived}`, or write nothing."))
            }
            FaultKind::AssertedLatent { derived, .. } => {
                Some(format!("inference is not a coercion. Write `@{derived}`, or write nothing."))
            }
            FaultKind::Frozen { .. } => Some(
                "a value that has been named cannot change; make another one instead.".to_string(),
            ),
            FaultKind::Stated { .. } | FaultKind::StatedLatent { .. } | FaultKind::Malformed(_) => {
                None
            }
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FaultKind::Ungranted { needed, held } if held.is_none() => {
                write!(f, "this reaches stratum {needed}, and depth 0 is held here")
            }
            FaultKind::Ungranted { needed, held } => match Capability::at(needed) {
                Some(c) => write!(f, "this reaches stratum {needed}; `{c}` is not in {held}"),
                None => write!(f, "this reaches stratum {needed}, which is not in {held}"),
            },
            // §1.6: where something is held and it is the wrong one, say so
            // rather than report a number. A shade from stratum 3 looked at
            // under `descend net` is not a shallower scope, and "at depth 5"
            // would tell the reader their scope is deep enough.
            FaultKind::Orpheus { origin, held } if held.is_none() => {
                write!(f, "cannot look at a shade from stratum {origin} at depth 0")
            }
            FaultKind::Orpheus { origin, held } => {
                write!(f, "cannot look at a shade from stratum {origin}; {held} is held here")
            }
            FaultKind::Stated { derived, stated } => {
                write!(f, "the rules give depth {derived}; this node says {stated}")
            }
            FaultKind::StatedLatent { derived, stated } => {
                write!(f, "the rules give @{derived}; this node says @{stated}")
            }
            FaultKind::Asserted { derived, asserted } => {
                write!(f, "annotated @{asserted}; inference gives {derived}")
            }
            FaultKind::AssertedLatent { derived, asserted } => {
                write!(f, "annotated @{asserted}; inference gives @{derived}")
            }
            FaultKind::Frozen { .. } => {
                f.write_str("this was named, and a name cannot change what it names")
            }
            FaultKind::Malformed(why) => f.write_str(why),
        }
    }
}

/// Check a unit against the calculus. An empty result is a clean bill.
#[must_use]
pub fn check(unit: &Unit) -> Vec<Fault> {
    let mut c = Checker {
        unit,
        faults: Vec::new(),
        globals: Vec::new(),
        locals: Vec::new(),
        named: Vec::new(),
        deferred_asserts: Vec::new(),
        func: None,
        returns: Depth::PURE,
        needs: None,
        fixpoint_steps: 0,
    };
    // A unit-level binding may name one declared before it and not one
    // declared after, so declaration order is checking order. Unit-level code
    // binds into the unit's frame, as a body binds into its function's.
    c.unit_frame();
    for g in &unit.globals {
        let (derived, _) = c.expr(&g.value, Held::NONE, Depth::PURE);
        if !c.depth_types_agree(&g.value.ty, &g.ty) {
            c.malformed(g.span, "shade origin does not match binding type");
        }
        c.globals.push(derived);
        c.asserted_lazy(g.span, derived, g.asserted, |this| this.blame(&g.value, derived));
        c.check_deferred_asserts();
    }
    for f in &unit.funcs {
        c.func(f);
    }
    // [DEMAND] keeps the depth and throws the value away: `demand e : U0@d`.
    c.unit_frame();
    for d in &unit.demands {
        c.expr(&d.value, Held::NONE, Depth::PURE);
        c.check_deferred_asserts();
    }
    c.faults
}

/// Control-flow exit depths propagated out of a statement or expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExitPc {
    /// Deepest control condition guarding a `break` or `continue` in this form.
    loop_exit: Option<Depth>,
    /// Deepest control condition guarding a `return` in this form.
    fn_exit: Option<Depth>,
}

impl ExitPc {
    const NONE: Self = Self { loop_exit: None, fn_exit: None };

    fn join(self, other: Self) -> Self {
        Self {
            loop_exit: join_opt(self.loop_exit, other.loop_exit),
            fn_exit: join_opt(self.fn_exit, other.fn_exit),
        }
    }

    fn deepest(self) -> Option<Depth> {
        join_opt(self.loop_exit, self.fn_exit)
    }
}

fn join_opt(a: Option<Depth>, b: Option<Depth>) -> Option<Depth> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.join(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

struct Checker<'a> {
    unit: &'a Unit,
    faults: Vec<Fault>,
    globals: Vec<Depth>,
    /// The current function's bindings. `None` until bound.
    locals: Vec<Option<Depth>>,
    /// Where each local's value was named, once it has been.
    ///
    /// A local may be assigned until its value is used *as a value* — sealed,
    /// shaded, deposited, returned, or passed as an argument. From then on its
    /// cairn exists and nothing can change what a cairn names. §5.4.
    ///
    /// Arithmetic on a local does not name it: `t + 1` makes a different value
    /// with a different cairn, and `t`'s own name never existed. That is why
    /// `for (I64 i = 0; i < n; i += 1)` is legal.
    named: Vec<Option<Span>>,
    /// Locals whose `@d` assertion is checked against their final depth once
    /// writes have settled (`Stmt::Declare` and locals deepened by `Assign`).
    deferred_asserts: Vec<LocalId>,
    /// The function being checked, for the spans of its bindings.
    func: Option<&'a FuncDef>,
    /// The deepest value any `return` has carried out of it so far.
    returns: Depth,
    /// While inside a function body: the deepest stratum an application in it
    /// has asked for and not been given. That is the function's latent depth,
    /// and it is a question for its callers rather than a fault here.
    ///
    /// `None` at the top level of a file, where there is nobody to ask.
    needs: Option<Held>,
    /// Total loop fixpoint re-iterations used in this unit.
    fixpoint_steps: usize,
}

impl<'a> Checker<'a> {
    /// What one turn of a loop hands the next. A local bound inside the body
    /// is bound again before the next turn reads it, so its state is left out.
    /// Comparing it would cost every loop a second walk, and a loop nested in
    /// it twice that.
    fn carried(&self, inner: &[LocalId]) -> (Vec<Option<Depth>>, Vec<Option<Span>>) {
        let (mut locals, mut named) = (self.locals.clone(), self.named.clone());
        for id in inner {
            let i = id.0 as usize;
            if let (Some(l), Some(n)) = (locals.get_mut(i), named.get_mut(i)) {
                (*l, *n) = (None, None);
            }
        }
        (locals, named)
    }

    fn fault(&mut self, span: Span, kind: FaultKind) {
        self.faults.push(Fault { span, kind, blame: None });
    }

    fn fault_blaming(&mut self, span: Span, kind: FaultKind, blame: Option<Blame>) {
        self.faults.push(Fault { span, kind, blame });
    }

    /// The call that took `x` to `depth`, named.
    ///
    /// Only ever walked on the way to an error, and guarded against revisiting
    /// any local or global so DAGs and cyclic hand-built IR stay linear.
    fn blame(&self, x: &Expr, depth: Depth) -> Option<Blame> {
        let mut seen_locals = Vec::new();
        let mut seen_globals = Vec::new();
        self.blame_walk(x, depth, &mut seen_locals, &mut seen_globals)
    }

    fn blame_walk(
        &self,
        x: &Expr,
        depth: Depth,
        seen_locals: &mut Vec<LocalId>,
        seen_globals: &mut Vec<GlobalId>,
    ) -> Option<Blame> {
        if let ExprKind::Call { callee, .. } = &x.kind {
            if let Type::Fn { latent, result_depth, .. } = callee.ty {
                if depth != Depth::PURE && (latent.holds(depth) || result_depth == depth) {
                    if let Some(what) = self.name_of(callee) {
                        return Some(Blame { span: x.span, what });
                    }
                }
            }
        }
        if let ExprKind::Local(id) = x.kind {
            if !seen_locals.contains(&id) {
                seen_locals.push(id);
                if let Some(bound) = self.find_binding(id) {
                    return self.blame_walk(bound, depth, seen_locals, seen_globals);
                }
            }
            return None;
        }
        if let ExprKind::Global(id) = x.kind {
            if !seen_globals.contains(&id) {
                seen_globals.push(id);
                if let Some(g) = self.unit.global(id) {
                    return self.blame_walk(&g.value, depth, seen_locals, seen_globals);
                }
            }
            return None;
        }
        if let Some(found) = children(x)
            .into_iter()
            .find_map(|c| self.blame_walk(c, depth, seen_locals, seen_globals))
        {
            return Some(found);
        }
        // A descent is the answer only when nothing inside it is: `read` is
        // what somebody wrote, and `descend disk` is where they said they were
        // going.
        if let ExprKind::Descend { capability, .. } = &x.kind {
            if capability.stratum() == depth {
                return Some(Blame { span: x.span, what: format!("descend {capability}") });
            }
        }
        None
    }

    fn find_binding(&self, id: LocalId) -> Option<&'a Expr> {
        if let Some(f) = self.func {
            return bound_to(&f.body, id);
        }
        self.unit
            .globals
            .iter()
            .map(|g| &g.value)
            .chain(self.unit.demands.iter().map(|d| &d.value))
            .find_map(|e| bound_to_expr(e, id))
    }

    /// What made a block reach a depth.
    fn blame_block(&self, block: &Block, depth: Depth) -> Option<Blame> {
        let mut seen_locals = Vec::new();
        let mut seen_globals = Vec::new();
        let each = block.stmts.iter().filter_map(|s| match s {
            Stmt::Let { value, .. } | Stmt::Expr(value) => Some(value),
            // Nothing is evaluated, so nothing here reached a depth.
            Stmt::Declare { .. } => None,
        });
        each.chain(block.tail.as_deref())
            .find_map(|x| self.blame_walk(x, depth, &mut seen_locals, &mut seen_globals))
    }

    fn name_of(&self, callee: &Expr) -> Option<String> {
        match &callee.kind {
            ExprKind::Prim(p) => Some(p.name().to_string()),
            ExprKind::Func(id) => self.unit.func(*id).map(|f| f.name.clone()),
            _ => None,
        }
    }

    fn malformed(&mut self, span: Span, why: &'static str) {
        self.fault(span, FaultKind::Malformed(why));
    }

    /// One depth the rules gave, against the one the node states and the one
    /// the programmer wrote.
    fn against_lazy<F>(
        &mut self,
        span: Span,
        derived: Depth,
        stated: Depth,
        asserted: Asserted,
        blame: F,
    ) where
        F: FnOnce(&Self) -> Option<Blame>,
    {
        if derived != stated {
            self.fault(span, FaultKind::Stated { derived, stated });
        }
        self.asserted_lazy(span, derived, asserted, blame);
    }

    /// The same, for the set [ABS] gives. §2.1: `dƒ` is a set and `d_r` is a
    /// number, so they are checked by two functions rather than one.
    fn against_latent_lazy<F>(
        &mut self,
        span: Span,
        derived: Held,
        stated: Held,
        asserted: Option<Held>,
        blame: F,
    ) where
        F: FnOnce(&Self, Depth) -> Option<Blame>,
    {
        if derived != stated {
            self.fault(span, FaultKind::StatedLatent { derived, stated });
        }
        if let Some(asserted) = asserted {
            if asserted != derived {
                let extra = derived.without(asserted);
                let target = if extra.is_none() { derived.deepest() } else { extra.deepest() };
                let blame = blame(self, target);
                self.fault_blaming(span, FaultKind::AssertedLatent { derived, asserted }, blame);
            }
        }
    }

    fn asserted_lazy<F>(&mut self, span: Span, derived: Depth, asserted: Option<Depth>, blame: F)
    where
        F: FnOnce(&Self) -> Option<Blame>,
    {
        if let Some(asserted) = asserted {
            if asserted != derived {
                let blame = blame(self);
                self.fault_blaming(span, FaultKind::Asserted { derived, asserted }, blame);
            }
        }
    }

    fn expect_ty(&mut self, x: &Expr, want: &Type, why: &'static str) {
        if x.ty != *want {
            self.malformed(x.span, why);
        }
    }

    fn bind(&mut self, id: LocalId, depth: Depth, span: Span) {
        match self.locals.get_mut(id.0 as usize) {
            Some(slot) => *slot = Some(depth),
            None => self.malformed(span, "this binding names no local"),
        }
        if let Some(slot) = self.named.get_mut(id.0 as usize) {
            *slot = None;
        }
        if let Some(l) = self.func.map_or_else(|| self.unit.local(id), |f| f.local(id))
            && l.asserted.is_some()
            && !self.deferred_asserts.contains(&id)
        {
            self.deferred_asserts.push(id);
        }
    }

    fn declare(&mut self, id: LocalId) {
        let span = self
            .func
            .map_or_else(|| self.unit.local(id), |f| f.local(id))
            .map_or_else(Span::default, |l| l.span);
        match self.locals.get_mut(id.0 as usize) {
            Some(slot) => *slot = Some(Depth::PURE),
            None => self.malformed(span, "this binding names no local"),
        }
        if let Some(slot) = self.named.get_mut(id.0 as usize) {
            *slot = None;
        }
        if !self.deferred_asserts.contains(&id) {
            self.deferred_asserts.push(id);
        }
    }

    fn check_deferred_asserts(&mut self) {
        let deferred = core::mem::take(&mut self.deferred_asserts);
        for id in deferred {
            let Some(derived) = self.locals.get(id.0 as usize).copied().flatten() else {
                continue;
            };
            let Some(l) = self.func.map_or_else(|| self.unit.local(id), |f| f.local(id)) else {
                continue;
            };
            let (span, asserted) = (l.span, l.asserted);
            self.asserted_lazy(span, derived, asserted, |this| {
                this.find_binding(id)
                    .and_then(|e| this.blame(e, derived))
                    .or_else(|| this.func.and_then(|f| this.blame_block(&f.body, derived)))
            });
        }
    }

    fn local_ty(&self, id: LocalId) -> Option<&'a Type> {
        self.func.map_or_else(|| self.unit.local(id), |f| f.local(id)).map(|l| &l.ty)
    }

    fn field_ty(&self, base_ty: &Type, index: u32) -> Option<&'a Type> {
        let Type::Struct(name) = base_ty else { return None };
        let def = self.unit.structs.iter().find(|s| s.name == *name)?;
        def.fields.get(index as usize).map(|f| &f.ty)
    }

    fn place_ty(&self, place: &crate::Place) -> Option<&'a Type> {
        let mut cur = self.local_ty(place.local)?;
        for proj in &place.path {
            cur = match proj {
                Proj::Field(idx) => self.field_ty(cur, *idx)?,
                Proj::Index(_) => match cur {
                    Type::Array { elem, .. } => elem,
                    _ => return None,
                },
            };
        }
        Some(cur)
    }

    /// [ABS], which produces both of an arrow's depths.
    ///
    /// The body is checked with the parameters at depth 0 — a parameter's real
    /// depth arrives at the call site and [APP] joins it there. What comes
    /// back is the body's value depth, including through every `return`, which
    /// is the other way a value leaves. What the function asks of its caller
    /// is whatever an application in it needed and did not have.
    ///
    /// The ambient depth it is checked at does not matter, because the ambient
    /// depth gates premises and nothing else: no derived depth depends on it.
    /// So the body is walked once, at 0, collecting what it asks for.
    fn unit_frame(&mut self) {
        self.func = None;
        self.locals = vec![None; self.unit.locals.len()];
        self.named = vec![None; self.unit.locals.len()];
        self.deferred_asserts.clear();
    }

    fn func(&mut self, f: &'a FuncDef) {
        self.func = Some(f);
        self.locals = vec![None; f.locals.len()];
        self.named = vec![None; f.locals.len()];
        self.deferred_asserts.clear();
        self.returns = Depth::PURE;
        self.needs = Some(Held::NONE);
        for p in &f.params {
            self.bind(*p, Depth::PURE, f.span);
        }

        let (body_depth, _) = self.block(&f.body, Held::NONE, Depth::PURE);
        match &f.body.tail {
            Some(tail) if !self.depth_types_agree(&tail.ty, &f.ret) => {
                self.malformed(tail.span, "shade origin does not match return type");
            }
            None if !block_always_returns(&f.body)
                && !self.depth_types_agree(&Type::Unit, &f.ret) =>
            {
                self.malformed(
                    f.span,
                    "function body falls off end without returning declared type",
                );
            }
            _ => {}
        }
        let ret_depth = body_depth.join(self.returns);
        let latent = self.needs.take().unwrap_or(Held::NONE);

        self.check_deferred_asserts();
        self.against_lazy(f.span, ret_depth, f.ret_depth, f.asserted_ret, |this| {
            this.blame_block(&f.body, ret_depth)
        });
        self.against_latent_lazy(f.span, latent, f.latent, f.asserted_latent, |this, target| {
            this.blame_block(&f.body, target)
        });
        self.func = None;
    }

    fn block(&mut self, block: &Block, ambient: Held, pc: Depth) -> (Depth, ExitPc) {
        let mut block_pc = pc;
        let mut exit = ExitPc::NONE;
        for s in &block.stmts {
            match s {
                Stmt::Let { local, value } => {
                    let (d, ex) = self.expr(value, ambient, block_pc);
                    if let Some(lty) = self.local_ty(*local)
                        && !self.depth_types_agree(&value.ty, lty)
                    {
                        self.malformed(value.span, "shade origin does not match binding type");
                    }
                    self.bind(*local, d, value.span);
                    exit = exit.join(ex);
                    if let Some(ex_d) = ex.deepest() {
                        block_pc = block_pc.join(ex_d);
                    }
                }
                Stmt::Expr(x) => {
                    let (_, ex) = self.expr(x, ambient, block_pc);
                    // §4.7: a bare expression statement deposits its value.
                    self.name_local(x);
                    exit = exit.join(ex);
                    if let Some(ex_d) = ex.deepest() {
                        block_pc = block_pc.join(ex_d);
                    }
                }
                // A local with no value is pure until something is written to
                // it, and `Assign` joins the depth of what it writes. §5.4.
                Stmt::Declare { local } => self.declare(*local),
            }
        }
        let tail = block
            .tail
            .as_ref()
            .map_or((Depth::PURE, ExitPc::NONE), |t| self.expr(t, ambient, block_pc));
        (tail.0, exit.join(tail.1))
    }

    #[expect(clippy::too_many_lines, reason = "eleven rules, and an arm for each")]
    fn expr(&mut self, x: &Expr, ambient: Held, pc: Depth) -> (Depth, ExitPc) {
        let (derived, exit) = match &x.kind {
            // [LIT]. This is why pure code disappears at burial.
            ExprKind::Literal(_) => {
                if self.carries_depth_type(&x.ty) {
                    self.malformed(x.span, "literal cannot fabricate a shaded or function type");
                }
                (Depth::PURE, ExitPc::NONE)
            }
            ExprKind::Break | ExprKind::Continue => {
                (Depth::PURE, ExitPc { loop_exit: Some(pc), fn_exit: None })
            }

            // [VAR].
            ExprKind::Local(id) => {
                if let Some(lty) = self.local_ty(*id)
                    && !self.depth_types_agree(&x.ty, lty)
                {
                    self.malformed(x.span, "local type does not match its binding");
                }
                let d = if let Some(d) = self.locals.get(id.0 as usize).copied().flatten() {
                    d
                } else {
                    self.malformed(x.span, "this local is not bound here");
                    x.depth
                };
                (d, ExitPc::NONE)
            }
            ExprKind::Global(id) => {
                if let Some(g) = self.unit.global(*id)
                    && !self.depth_types_agree(&x.ty, &g.ty)
                {
                    self.malformed(x.span, "global type does not match its binding");
                }
                let d = if let Some(d) = self.globals.get(id.0 as usize).copied() {
                    d
                } else {
                    self.malformed(x.span, "this unit-level binding is not declared yet");
                    x.depth
                };
                (d, ExitPc::NONE)
            }

            // [ABS]. Building a function that will touch the disk does not
            // touch the disk, so the closure itself is pure.
            ExprKind::Func(id) => {
                match self.unit.func(*id) {
                    Some(f) => {
                        let params = f
                            .params
                            .iter()
                            .filter_map(|p| f.local(*p))
                            .map(|l| l.ty.clone())
                            .collect();
                        let result = Box::new(f.ret.clone());
                        let want = Type::Fn {
                            params,
                            latent: f.latent,
                            result,
                            result_depth: f.ret_depth,
                        };
                        self.expect_ty(x, &want, "this is not the signature of that function");
                    }
                    None => self.malformed(x.span, "this names no function in the unit"),
                }
                (Depth::PURE, ExitPc::NONE)
            }
            ExprKind::Prim(p) => {
                self.prim(x, *p);
                (Depth::PURE, ExitPc::NONE)
            }

            // [APP]. Three depths in the result, not two: the latent depth of
            // the arrow, the depth of the function value itself, and the
            // argument's. A function fetched over the network is deep before
            // it is ever called.
            //
            // One of the three is the premise. A capability is what it takes
            // to reach a stratum, and the latent depth is the only term that
            // reaches one: the other two are facts about where those values
            // have already been.
            ExprKind::Call { callee, args } => {
                let mut cur_pc = pc;
                let (d_f, mut ex) = self.expr(callee, ambient, cur_pc);
                if let Some(ed) = ex.deepest() {
                    cur_pc = cur_pc.join(ed);
                }
                let mut d_a = Depth::PURE;
                for a in args {
                    let (d, a_ex) = self.expr(a, ambient, cur_pc);
                    // Passing a local hands its value somewhere this function
                    // cannot see, which is naming it immediately before later
                    // arguments evaluate. §5.4.
                    self.name_local(a);
                    d_a = d_a.join(d);
                    ex = ex.join(a_ex);
                    if let Some(ed) = a_ex.deepest() {
                        cur_pc = cur_pc.join(ed);
                    }
                }
                let (latent, result_depth) =
                    if let Type::Fn { params, latent, result, result_depth } = &callee.ty {
                        if args.len() != params.len() {
                            self.malformed(x.span, "wrong number of arguments");
                        }
                        for (arg, param) in args.iter().zip(params) {
                            if !self.depth_types_agree(&arg.ty, param) {
                                self.malformed(
                                    arg.span,
                                    "shade origin does not match parameter type",
                                );
                            }
                        }
                        if !self.depth_types_agree(&x.ty, result) {
                            self.malformed(x.span, "shade origin does not match call result type");
                        }
                        (*latent, *result_depth)
                    } else {
                        self.malformed(callee.span, "this is applied and is not a function");
                        (Held::NONE, Depth::PURE)
                    };
                if !latent.subset_of(ambient) {
                    // Inside a function body this is not a fault: it is what
                    // the function asks of whoever calls it. At the top level
                    // of a file there is nobody to ask.
                    let missing = latent.without(ambient);
                    if let Some(needs) = self.needs {
                        self.needs = Some(needs.union(missing));
                    } else {
                        let blame = self.name_of(callee).map(|what| Blame { span: x.span, what });
                        let needed = missing.deepest();
                        let kind = FaultKind::Ungranted { needed, held: ambient };
                        self.fault_blaming(x.span, kind, blame);
                    }
                }
                (result_depth.join(d_f).join(d_a), ex)
            }

            // [DESCEND]. The only rule that raises δ, and only inside its own
            // premise. Nothing lowers δ; nothing lowers d.
            ExprKind::Descend { capability, body } => {
                if !self.depth_types_agree(&x.ty, &body.ty) {
                    self.malformed(x.span, "shade origin does not match descend body");
                }
                self.expr(body, ambient.with(capability.stratum()), pc)
            }

            ExprKind::Rite { rite, operand } => self.rite(x, *rite, operand, ambient, pc),

            // [PRIM], and everything the specification gives no other rule for.
            ExprKind::Unary { operand, .. } => {
                if self.carries_depth_type(&x.ty) {
                    self.malformed(
                        x.span,
                        "unary operation cannot fabricate a shaded or function type",
                    );
                }
                self.expr(operand, ambient, pc)
            }
            ExprKind::Field { base, index } => {
                let res = self.expr(base, ambient, pc);
                if let Some(fty) = self.field_ty(&base.ty, *index) {
                    if !self.depth_types_agree(&x.ty, fty) {
                        self.malformed(x.span, "shade origin does not match field type");
                    }
                } else if self.carries_depth_type(&x.ty) {
                    self.malformed(x.span, "unknown struct or field");
                }
                res
            }
            ExprKind::Binary { lhs, rhs, .. } => {
                if self.carries_depth_type(&x.ty) {
                    self.malformed(
                        x.span,
                        "binary operation cannot fabricate a shaded or function type",
                    );
                }
                let (dl, el) = self.expr(lhs, ambient, pc);
                let rhs_pc = el.deepest().map_or(pc, |e| pc.join(e));
                let (dr, er) = self.expr(rhs, ambient, rhs_pc);
                (dl.join(dr), el.join(er))
            }
            ExprKind::Index { base, index } => {
                let (dl, el) = self.expr(base, ambient, pc);
                let idx_pc = el.deepest().map_or(pc, |e| pc.join(e));
                let (dr, er) = self.expr(index, ambient, idx_pc);
                if let Type::Array { elem, .. } = &base.ty {
                    if !self.depth_types_agree(&x.ty, elem) {
                        self.malformed(x.span, "shade origin does not match element type");
                    }
                } else if self.carries_depth_type(&x.ty) {
                    self.malformed(x.span, "indexed base is not an array");
                }
                (dl.join(dr), el.join(er))
            }
            // §5.4: the arms are exclusive, so neither precedes the other and
            // a naming on one does not freeze the other. Each is walked from
            // where the block started; what they name is joined afterwards,
            // because it is named for everything *after* the block.
            ExprKind::Select { cond, then, otherwise } => {
                let (d_cond, e_cond) = self.expr(cond, ambient, pc);
                let branch_pc = e_cond.deepest().map_or(pc, |e| pc.join(e)).join(d_cond);
                let (named, locals) = (self.named.clone(), self.locals.clone());
                let (d_then, e_then) = self.expr(then, ambient, branch_pc);
                let from_then = core::mem::replace(&mut self.named, named);
                let locals_then = core::mem::replace(&mut self.locals, locals);
                let (d_else, e_else) = self.expr(otherwise, ambient, branch_pc);
                for (slot, was) in self.named.iter_mut().zip(&from_then) {
                    *slot = slot.or(*was);
                }
                for (s, w) in self.locals.iter_mut().zip(&locals_then) {
                    *s = s.zip(*w).map(|(a, b)| a.join(b)).or(*s).or(*w);
                }
                if !self.depth_types_agree(&then.ty, &x.ty)
                    || !self.depth_types_agree(&otherwise.ty, &x.ty)
                {
                    self.malformed(x.span, "shade origin does not match branch arms");
                }
                (d_cond.join(d_then).join(d_else), e_cond.join(e_then).join(e_else))
            }
            // §5.4: a loop body runs again, so a naming anywhere in it precedes
            // an assignment anywhere in it — including one written above it.
            // A turn starts where the last one ended, so the body is walked
            // until a turn ends where it started, and only that walk's faults
            // are kept. A write only joins and naming only adds, so the walks
            // stop. Two walks were not enough: `z = y; y = x; x = deep;` takes
            // a turn per link to carry `x`'s depth to `z`.
            ExprKind::Loop { body, step } => {
                let mut inner = bound_inside(body);
                if let Some(s) = step.as_deref() {
                    inner.extend(bound_inside(s));
                }
                let mut loop_pc = pc;
                loop {
                    let mark = self.faults.len();
                    let was = (self.carried(&inner), loop_pc);
                    for id in &inner {
                        if let Some(slot) = self.locals.get_mut(id.0 as usize) {
                            *slot = None;
                        }
                        if let Some(slot) = self.named.get_mut(id.0 as usize) {
                            *slot = None;
                        }
                    }
                    let (d_body, e_body) = self.expr(body, ambient, loop_pc);
                    let step_pc = e_body.deepest().map_or(loop_pc, |e| loop_pc.join(e));
                    let (d, e_step) = match step.as_ref() {
                        Some(s) => {
                            let (ds, es) = self.expr(s, ambient, step_pc);
                            (d_body.join(ds), es)
                        }
                        None => (d_body, ExitPc::NONE),
                    };
                    let combined = e_body.join(e_step);
                    if let Some(ex) = combined.deepest() {
                        loop_pc = loop_pc.join(ex);
                    }
                    let out_exit = ExitPc { loop_exit: None, fn_exit: combined.fn_exit };
                    if (self.carried(&inner), loop_pc) == was {
                        break (d, out_exit);
                    }
                    self.fixpoint_steps += 1;
                    if self.fixpoint_steps > MAX_FIXPOINT_STEPS {
                        self.faults.truncate(mark);
                        self.malformed(x.span, "loop depth inference exceeded iteration limit");
                        break (d, out_exit);
                    }
                    self.faults.truncate(mark);
                }
            }
            ExprKind::Return(v) => {
                let (d, ex) =
                    v.as_ref().map_or((Depth::PURE, ExitPc::NONE), |v| self.expr(v, ambient, pc));
                if let Some(v) = v.as_ref() {
                    self.name_local(v);
                    if let Some(f) = self.func
                        && !self.depth_types_agree(&v.ty, &f.ret)
                    {
                        self.malformed(v.span, "shade origin does not match return type");
                    }
                } else if let Some(f) = self.func
                    && !self.depth_types_agree(&Type::Unit, &f.ret)
                {
                    self.malformed(x.span, "empty return does not match function return type");
                }
                let ret_pc = ex.deepest().map_or(pc, |e| pc.join(e));
                self.returns = self.returns.join(d).join(ret_pc);
                (d, ex.join(ExitPc { loop_exit: None, fn_exit: Some(ret_pc) }))
            }
            ExprKind::Block(b) => {
                let res = self.block(b, ambient, pc);
                match &b.tail {
                    Some(tail) if !self.depth_types_agree(&x.ty, &tail.ty) => {
                        self.malformed(x.span, "shade origin does not match block tail");
                    }
                    None if !self.depth_types_agree(&x.ty, &Type::Unit) => {
                        self.malformed(x.span, "block without tail produces U0");
                    }
                    _ => {}
                }
                res
            }

            // A write into a binding deepens it. Nothing lowers a depth, so a
            // binding is as deep as the deepest thing ever written into it,
            // and no program can observe the difference: a value that has been
            // read has already been named. `spec/05-types.md` §5.4.
            ExprKind::Assign { place, value } => {
                let mut cur_pc = pc;
                let mut ex = ExitPc::NONE;
                let mut d_path = Depth::PURE;
                for p in &place.path {
                    if let Proj::Index(i) = p {
                        let (di, ei) = self.expr(i, ambient, cur_pc);
                        d_path = d_path.join(di);
                        ex = ex.join(ei);
                        if let Some(ed) = ei.deepest() {
                            cur_pc = cur_pc.join(ed);
                        }
                    }
                }
                let (d_value, e_val) = self.expr(value, ambient, cur_pc);
                ex = ex.join(e_val);
                if let Some(ed) = e_val.deepest() {
                    cur_pc = cur_pc.join(ed);
                }
                if let Some(pty) = self.place_ty(place) {
                    if !self.depth_types_agree(&value.ty, pty) {
                        self.malformed(value.span, "shade origin does not match assigned place");
                    }
                } else if self.carries_depth_type(&value.ty) {
                    self.malformed(x.span, "assigned place has no valid type");
                }
                if !self.depth_types_agree(&x.ty, &Type::Unit) {
                    self.malformed(x.span, "assignment produces U0");
                }
                if let Some(Some(named)) = self.named.get(place.local.0 as usize).copied() {
                    self.fault(x.span, FaultKind::Frozen { named });
                }
                let written = d_value.join(d_path).join(cur_pc);
                match self.locals.get_mut(place.local.0 as usize) {
                    Some(Some(was)) => {
                        let next = was.join(written);
                        let deepened = next != *was;
                        *was = next;
                        if deepened && !self.deferred_asserts.contains(&place.local) {
                            self.deferred_asserts.push(place.local);
                        }
                    }
                    Some(None) => self.malformed(x.span, "this local is not bound here"),
                    None => self.malformed(x.span, "this assigns to no local"),
                }
                (d_value.join(d_path), ex)
            }
        };

        if derived != x.depth {
            self.fault(x.span, FaultKind::Stated { derived, stated: x.depth });
        }
        (derived, exit)
    }

    /// Mark whatever local `x` reads as named, if it reads one directly (or
    /// through `opaque`, `descend`, or a block tail).
    ///
    /// Only a direct read counts. `seal h` names `h`; `seal (h.len + 1)` names
    /// nothing, because the value sealed is a fresh one that `h` never had a
    /// name for. §5.4's list — sealed, shaded, deposited, returned, passed —
    /// is exactly the set of places a local's own value escapes.
    fn name_local(&mut self, x: &Expr) {
        let mut todo = vec![x];
        while let Some(cur) = todo.pop() {
            match &cur.kind {
                ExprKind::Local(id) => {
                    if let Some(slot) = self.named.get_mut(id.0 as usize) {
                        // The first naming is the one worth reporting: it is where the
                        // value stopped being the program's to change.
                        if slot.is_none() {
                            *slot = Some(cur.span);
                        }
                    }
                }
                ExprKind::Rite { rite: Rite::Opaque, operand } => todo.push(operand),
                ExprKind::Descend { body, .. } => todo.push(body),
                ExprKind::Block(b) => {
                    if let Some(tail) = b.tail.as_deref() {
                        todo.push(tail);
                    }
                }
                ExprKind::Select { then, otherwise, .. } => {
                    todo.push(then);
                    todo.push(otherwise);
                }
                _ => {}
            }
        }
    }

    /// [SEAL], [SHADE], [LOOK] and [OPAQUE].
    fn rite(
        &mut self,
        x: &Expr,
        rite: Rite,
        operand: &Expr,
        ambient: Held,
        pc: Depth,
    ) -> (Depth, ExitPc) {
        let (d, ex) = self.expr(operand, ambient, pc);
        // `seal` and `shade` take the value itself; `look` and `opaque` pass
        // one through without giving it a name of its own.
        if matches!(rite, Rite::Seal | Rite::Shade) {
            self.name_local(operand);
        }
        let derived = match rite {
            // A cairn is a name, and a name is pure whatever it names.
            Rite::Seal => {
                if self.carries_depth_type(&x.ty) {
                    self.malformed(x.span, "`seal` produces a cairn");
                }
                Depth::PURE
            }
            // The origin is carried in the type, which is what makes [LOOK]
            // checkable at all.
            Rite::Shade => {
                let want = Type::Shade { origin: d, inner: Box::new(operand.ty.clone()) };
                self.expect_ty(x, &want, "this shade claims an origin its value never had");
                Depth::PURE
            }
            Rite::Look => {
                if let Type::Shade { origin, inner } = &operand.ty {
                    self.expect_ty(x, inner, "`look` gives back the type inside the shade");
                    // Unlike [APP]'s premise this does not become a question
                    // for the caller. §1.6: the check is local, it does not
                    // propagate, and it does not stain the enclosing scope —
                    // and a latent depth inferred from a `look` would be that
                    // staining, one scope out.
                    if !ambient.holds(*origin) {
                        let blame = self.blame(operand, *origin);
                        let kind = FaultKind::Orpheus { origin: *origin, held: ambient };
                        self.fault_blaming(x.span, kind, blame);
                    }
                    // Deep for two independent reasons: where the value came
                    // from, and where the shade itself came from.
                    origin.join(d)
                } else {
                    self.malformed(operand.span, "`look` needs a shade");
                    d
                }
            }
            // Opaque is a fact about evaluation and none about the calculus.
            Rite::Opaque => {
                if !self.depth_types_agree(&x.ty, &operand.ty) {
                    self.malformed(x.span, "shade origin does not match opaque operand");
                }
                d
            }
        };
        (derived, ex)
    }

    /// A prelude reference states an instantiated signature. Only the latent
    /// depth and shade-origin preservation on polymorphic forms are checked:
    /// five of the thirty-two are polymorphic in `T`, and the IR's types
    /// deliberately have no variables.
    fn prim(&mut self, x: &Expr, p: Prim) {
        // Every prelude function hands back what it went and got, so its two
        // depths are the same one. Nothing else in the language has to be.
        match &x.ty {
            Type::Fn { params, latent, result, result_depth }
                if *latent == Held::of(p.latent()) && *result_depth == p.latent() =>
            {
                match p {
                    Prim::Answered => {
                        let ok = params.len() == 1
                            && matches!(result.as_ref(), Type::Answer(inner) if self.depth_types_agree(&params[0], inner));
                        if !ok {
                            self.malformed(x.span, "shade origin does not match `answered` result");
                        }
                    }
                    Prim::Must => {
                        let ok = params.len() == 1
                            && matches!(&params[0], Type::Answer(inner) if self.depth_types_agree(inner, result));
                        if !ok {
                            self.malformed(x.span, "shade origin does not match `must` result");
                        }
                    }
                    _ => {}
                }
            }
            Type::Fn { .. } => self.malformed(x.span, "this prelude function has another stratum"),
            _ => self.malformed(x.span, "a prelude function is a function"),
        }
    }

    fn carries_depth_type(&self, ty: &Type) -> bool {
        let mut seen = Vec::new();
        self.carries_depth_inner(ty, &mut seen)
    }

    fn carries_depth_inner<'b>(&'a self, ty: &'b Type, seen: &mut Vec<&'b str>) -> bool
    where
        'a: 'b,
    {
        match ty {
            Type::Shade { .. } | Type::Fn { .. } => true,
            Type::Answer(inner) | Type::Array { elem: inner, .. } => {
                self.carries_depth_inner(inner, seen)
            }
            Type::Struct(name) => {
                if seen.contains(&name.as_str()) {
                    return false;
                }
                seen.push(name.as_str());
                let found = self.unit.structs.iter().find(|s| s.name == *name).is_some_and(|s| {
                    s.fields.iter().any(|f| self.carries_depth_inner(&f.ty, seen))
                });
                seen.pop();
                found
            }
            _ => false,
        }
    }

    /// Whether `got` agrees with `want` on every depth-bearing type (`Shade`
    /// origin, `Fn` latent/result depth, and any container or nominal `Struct`
    /// carrying them).
    fn depth_types_agree(&self, got: &Type, want: &Type) -> bool {
        match (got, want) {
            (Type::Shade { origin: a, inner: ia }, Type::Shade { origin: b, inner: ib }) => {
                a == b && self.depth_types_agree(ia, ib)
            }
            (Type::Answer(a), Type::Answer(b))
            | (Type::Array { elem: a, .. }, Type::Array { elem: b, .. }) => {
                self.depth_types_agree(a, b)
            }
            (Type::Struct(a), Type::Struct(b)) => a == b,
            (
                Type::Fn { params: pa, latent: la, result: ra, result_depth: da },
                Type::Fn { params: pb, latent: lb, result: rb, result_depth: db },
            ) => {
                la == lb
                    && da == db
                    && pa.len() == pb.len()
                    && pa.iter().zip(pb).all(|(a, b)| self.depth_types_agree(a, b))
                    && self.depth_types_agree(ra, rb)
            }
            _ => !self.carries_depth_type(got) && !self.carries_depth_type(want),
        }
    }
}

fn block_always_returns(b: &Block) -> bool {
    b.stmts.iter().any(|s| matches!(s, Stmt::Expr(e) if always_returns(e)))
        || b.tail.as_deref().is_some_and(always_returns)
}

fn always_returns(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Return(_) => true,
        ExprKind::Descend { body, .. } | ExprKind::Rite { rite: Rite::Opaque, operand: body } => {
            always_returns(body)
        }
        ExprKind::Select { cond, then, otherwise } => {
            always_returns(cond) || (always_returns(then) && always_returns(otherwise))
        }
        ExprKind::Block(b) => block_always_returns(b),
        _ => false,
    }
}

/// Every local a `let` or a declaration binds anywhere inside `x`.
fn bound_inside(x: &Expr) -> Vec<LocalId> {
    let (mut out, mut todo) = (Vec::new(), vec![x]);
    while let Some(x) = todo.pop() {
        if let ExprKind::Block(b) = &x.kind {
            out.extend(b.stmts.iter().filter_map(|s| match s {
                Stmt::Let { local, .. } | Stmt::Declare { local } => Some(*local),
                Stmt::Expr(_) => None,
            }));
        }
        todo.extend(children(x));
    }
    out
}

/// What bound this local, if anything in this block did.
#[must_use]
pub fn bound_to(block: &Block, id: LocalId) -> Option<&Expr> {
    let mut todo: Vec<&Expr> = Vec::new();
    for s in &block.stmts {
        match s {
            Stmt::Let { local, value } if *local == id => return Some(value),
            Stmt::Let { value, .. } | Stmt::Expr(value) => todo.push(value),
            Stmt::Declare { .. } => {}
        }
    }
    if let Some(tail) = block.tail.as_deref() {
        todo.push(tail);
    }
    bound_to_stack(todo, id)
}

fn bound_to_expr(x: &Expr, id: LocalId) -> Option<&Expr> {
    bound_to_stack(vec![x], id)
}

fn bound_to_stack(mut todo: Vec<&Expr>, id: LocalId) -> Option<&Expr> {
    while let Some(x) = todo.pop() {
        if let ExprKind::Block(b) = &x.kind {
            for s in &b.stmts {
                match s {
                    Stmt::Let { local, value } if *local == id => return Some(value),
                    Stmt::Let { value, .. } | Stmt::Expr(value) => todo.push(value),
                    Stmt::Declare { .. } => {}
                }
            }
            if let Some(tail) = b.tail.as_deref() {
                todo.push(tail);
            }
        } else {
            todo.extend(children(x));
        }
    }
    None
}

/// The subexpressions of an expression, in evaluation order.
#[must_use]
pub fn children(x: &Expr) -> Vec<&Expr> {
    match &x.kind {
        ExprKind::Literal(_)
        | ExprKind::Local(_)
        | ExprKind::Global(_)
        | ExprKind::Func(_)
        | ExprKind::Prim(_)
        | ExprKind::Break
        | ExprKind::Continue => Vec::new(),
        ExprKind::Call { callee, args } => {
            let mut out = vec![&**callee];
            out.extend(args);
            out
        }
        ExprKind::Unary { operand, .. }
        | ExprKind::Rite { operand, .. }
        | ExprKind::Descend { body: operand, .. }
        | ExprKind::Field { base: operand, .. } => vec![operand],
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
            vec![lhs, rhs]
        }
        ExprKind::Select { cond, then, otherwise } => vec![cond, then, otherwise],
        ExprKind::Loop { body, step } => {
            let mut out = vec![&**body];
            out.extend(step.as_deref());
            out
        }
        ExprKind::Return(v) => v.as_deref().into_iter().collect(),
        ExprKind::Block(b) => {
            let mut out: Vec<&Expr> = b
                .stmts
                .iter()
                .filter_map(|s| match s {
                    Stmt::Let { value, .. } | Stmt::Expr(value) => Some(value),
                    // No expression, so no child.
                    Stmt::Declare { .. } => None,
                })
                .collect();
            out.extend(b.tail.as_deref());
            out
        }
        ExprKind::Assign { place, value } => {
            let mut out: Vec<&Expr> = place
                .path
                .iter()
                .filter_map(|p| match p {
                    Proj::Index(i) => Some(&**i),
                    Proj::Field(_) => None,
                })
                .collect();
            out.push(value);
            out
        }
    }
}
