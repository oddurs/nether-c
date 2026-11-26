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

use crate::depth::{Depth, Held};
use crate::fault::{Blame, Fault, FaultKind, extra_stratum};
use crate::ir::{
    BinOp, Block, Expr, ExprKind, FuncId, GlobalId, LocalId, Place, Proj, Rite, Span, Stmt,
};
use crate::prim::Prim;
use crate::query::{
    block_always_returns, block_ends_with_value_select, bound_inside, bound_to, bound_to_expr,
    children,
};
use crate::ty::Type;
use crate::unit::{Asserted, Field, FuncDef, LocalDef, Unit};

/// Upper bound on total loop fixpoint re-walks per unit to prevent adversarial
/// exponential blowup on deeply nested loops.
const MAX_FIXPOINT_STEPS: usize = 4096;

/// Check a unit against the calculus. An empty result is a clean bill.
#[must_use]
pub fn check(unit: &Unit) -> Vec<Fault> {
    let mut c = Checker {
        unit,
        faults: Vec::new(),
        globals: Vec::new(),
        func_sigs: vec![(Depth::PURE, Held::NONE); unit.funcs.len()],
        param_shade_bounds: unit.funcs.iter().map(|f| vec![None; f.params.len()]).collect(),
        has_shade_bounds: false,
        locals: Vec::new(),
        local_param_deps: Vec::new(),
        named: Vec::new(),
        deferred_asserts: Vec::new(),
        deferred_field_asserts: Vec::new(),
        func: None,
        func_idx: None,
        returns: Depth::PURE,
        needs: None,
        fixpoint_steps: 0,
        in_loop: 0,
    };
    // Derive least `(ret_depth, latent)` signatures and parameter shade bounds
    // from lattice bottom so recursive functions cannot self-justify inflated
    // depths or smuggle deeper arguments into a callee's `shade`. [ABS], [SHADE].
    if !unit.funcs.is_empty() {
        for _ in 0..=(unit.funcs.len() + unit.globals.len()) * 2 + 2 {
            c.faults.clear();
            c.globals.clear();
            c.fixpoint_steps = 0;
            let mut moved = false;
            for g in &unit.globals {
                c.unit_frame();
                let (derived, _, _) = c.expr(&g.value, Held::NONE, Pc::PURE);
                c.globals.push(derived);
            }
            for (i, f) in unit.funcs.iter().enumerate() {
                let prev_bounds = c.param_shade_bounds[i].clone();
                let sig = c.func_signature(i, f);
                if c.func_sigs[i] != sig || c.param_shade_bounds[i] != prev_bounds {
                    c.func_sigs[i] = sig;
                    moved = true;
                }
            }
            if !moved {
                break;
            }
        }
        c.faults.clear();
        c.globals.clear();
        c.fixpoint_steps = 0;
    }
    // A unit-level binding may name one declared before it and not one
    // declared after, so declaration order is checking order. Each global and
    // demand gets an isolated local frame.
    for g in &unit.globals {
        c.unit_frame();
        let (derived, _, _) = c.expr(&g.value, Held::NONE, Pc::PURE);
        if !c.depth_types_agree(&g.value.ty, &g.ty) {
            c.malformed(g.span, "shade origin does not match binding type");
        }
        c.globals.push(derived);
        c.asserted_lazy(g.span, derived, g.asserted, |this| this.blame(&g.value, derived));
        c.check_deferred_asserts();
    }
    for (i, f) in unit.funcs.iter().enumerate() {
        c.func(i, f);
    }
    // [DEMAND] keeps the depth and throws the value away: `demand e : U0@d`.
    for d in &unit.demands {
        c.unit_frame();
        c.expr(&d.value, Held::NONE, Pc::PURE);
        c.check_deferred_asserts();
    }
    c.faults
}

/// Control-dependence depth and parameter bitmask at the current point.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Pc {
    depth: Depth,
    deps: u64,
}

impl Pc {
    const PURE: Self = Self { depth: Depth::PURE, deps: 0 };

    fn advance(&mut self, ex: ExitPc) {
        if let Some(ed) = ex.deepest() {
            self.depth = self.depth.join(ed);
        }
        self.deps |= ex.deps();
    }

    fn with_exit(mut self, ex: ExitPc) -> Self {
        self.advance(ex);
        self
    }
}

/// Control-flow exit depths propagated out of a statement or expression.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct ExitPc {
    /// Deepest control condition guarding a `break` or `continue` in this form.
    loop_exit: Option<Depth>,
    /// Deepest control condition guarding a `return` in this form.
    fn_exit: Option<Depth>,
    /// Parameter dependencies of control conditions guarding loop exits.
    loop_deps: u64,
    /// Parameter dependencies of control conditions guarding function returns.
    fn_deps: u64,
}

impl ExitPc {
    const NONE: Self = Self { loop_exit: None, fn_exit: None, loop_deps: 0, fn_deps: 0 };

    fn join(self, other: Self) -> Self {
        Self {
            loop_exit: Depth::join_opt(self.loop_exit, other.loop_exit),
            fn_exit: Depth::join_opt(self.fn_exit, other.fn_exit),
            loop_deps: self.loop_deps | other.loop_deps,
            fn_deps: self.fn_deps | other.fn_deps,
        }
    }

    fn deepest(self) -> Option<Depth> {
        Depth::join_opt(self.loop_exit, self.fn_exit)
    }

    fn deps(self) -> u64 {
        self.loop_deps | self.fn_deps
    }
}

/// Non-leaf struct field `@d` assertion accumulated across sub-field writes.
struct DeferredFieldAssert {
    local: LocalId,
    path: Vec<u32>,
    asserted: Depth,
    depth: Depth,
    span: Span,
}

type FieldAssert = (Vec<u32>, Depth, bool);

struct Checker<'a> {
    unit: &'a Unit,
    faults: Vec<Fault>,
    globals: Vec<Depth>,
    /// Least `(ret_depth, latent)` derived per function by fixpoint iteration.
    func_sigs: Vec<(Depth, Held)>,
    /// Tightest `shade` origin reached by each parameter of each function.
    /// Any caller passing an argument deeper than this bound would invalidate
    /// the callee's static `Shade<T@origin>` and bypass Orpheus (`[SHADE]`).
    param_shade_bounds: Vec<Vec<Option<Depth>>>,
    /// Fast-path flag: true once any function records a parameter shade bound.
    has_shade_bounds: bool,
    /// The current function's bindings. `None` until bound.
    locals: Vec<Option<Depth>>,
    /// Parameter-dependency bitmask for each local in the current function.
    local_param_deps: Vec<u64>,
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
    /// Non-leaf struct field `@d` assertions accumulated across sub-field writes.
    deferred_field_asserts: Vec<DeferredFieldAssert>,
    /// The function being checked, for the spans of its bindings.
    func: Option<&'a FuncDef>,
    /// Index of `self.func` inside `self.unit.funcs`.
    func_idx: Option<usize>,
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
    /// Number of enclosing `Loop` expressions around the current expression.
    in_loop: u32,
}

fn param_bit(index: usize) -> u64 {
    1u64 << index.min(63)
}

fn tighten_bound(slot: &mut Option<Depth>, bound: Depth) {
    *slot = Some(slot.map_or(bound, |prev| bound.min(prev)));
}

impl<'a> Checker<'a> {
    fn clear_local(&mut self, id: LocalId) {
        let i = id.0 as usize;
        if let Some(slot) = self.locals.get_mut(i) {
            *slot = None;
        }
        if let Some(slot) = self.local_param_deps.get_mut(i) {
            *slot = 0;
        }
        if let Some(slot) = self.named.get_mut(i) {
            *slot = None;
        }
    }

    /// What one turn of a loop hands the next. A local bound inside the body
    /// is bound again before the next turn reads it, so its state is left out.
    /// Comparing it would cost every loop a second walk, and a loop nested in
    /// it twice that.
    fn carried(&self, inner: &[LocalId]) -> (Vec<Option<Depth>>, Vec<Option<Span>>, Vec<u64>) {
        let (mut locals, mut named, mut deps) =
            (self.locals.clone(), self.named.clone(), self.local_param_deps.clone());
        for id in inner {
            let i = id.0 as usize;
            if let (Some(l), Some(n)) = (locals.get_mut(i), named.get_mut(i)) {
                (*l, *n) = (None, None);
            }
            if let Some(d) = deps.get_mut(i) {
                *d = 0;
            }
        }
        (locals, named, deps)
    }

    fn record_param_shade_bound(&mut self, deps: u64, bound: Depth) {
        let Some(fi) = self.func_idx else { return };
        let Some(bounds) = self.param_shade_bounds.get_mut(fi) else { return };
        for (pi, slot) in bounds.iter_mut().enumerate() {
            if (deps & param_bit(pi)) != 0 {
                tighten_bound(slot, bound);
                self.has_shade_bounds = true;
            }
        }
    }

    fn check_callee_shade_bounds(
        &mut self,
        fid: FuncId,
        args: &[Expr],
        arg_info: &[(Depth, u64)],
        pc_deps: u64,
    ) {
        if !self.has_shade_bounds {
            return;
        }
        let Some(bounds) = self.param_shade_bounds.get(fid.0 as usize) else {
            return;
        };
        if !bounds.iter().any(Option::is_some) {
            return;
        }
        let bounds = bounds.clone();
        for (arg_idx, (arg, &(ad, arg_deps))) in args.iter().zip(arg_info).enumerate() {
            if let Some(Some(bound)) = bounds.get(arg_idx).copied() {
                if ad > bound {
                    self.malformed(arg.span, "argument is deeper than a shade inside the callee");
                }
                let deps = arg_deps | pc_deps;
                if deps != 0 {
                    self.record_param_shade_bound(deps, bound);
                }
            }
        }
    }

    fn callee_funcs(callee: &Expr, out: &mut Vec<FuncId>) {
        let mut stack = vec![callee];
        while let Some(cur) = stack.pop() {
            match &cur.kind {
                ExprKind::Func(id) if !out.contains(id) => out.push(*id),
                ExprKind::Block(b) => stack.extend(b.tail.as_deref()),
                ExprKind::Descend { body, .. } => stack.push(body),
                ExprKind::Rite { rite: Rite::Opaque, operand } => stack.push(operand),
                ExprKind::Select { then, otherwise, .. } => {
                    stack.push(then);
                    stack.push(otherwise);
                }
                _ => {}
            }
        }
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
        self.blame_walk(x, depth, false, &mut Vec::new(), &mut Vec::new())
    }

    fn blame_shade(&self, x: &Expr, depth: Depth) -> Option<Blame> {
        self.blame_walk(x, depth, true, &mut Vec::new(), &mut Vec::new())
    }

    fn blame_walk(
        &self,
        x: &Expr,
        depth: Depth,
        through_shade: bool,
        seen_locals: &mut Vec<LocalId>,
        seen_globals: &mut Vec<GlobalId>,
    ) -> Option<Blame> {
        match &x.kind {
            ExprKind::Rite { rite: Rite::Seal, .. } => return None,
            ExprKind::Rite { rite: Rite::Shade, .. } if !through_shade => return None,
            ExprKind::Call { callee, .. } => {
                if let Type::Fn { latent, result_depth, .. } = callee.ty
                    && depth != Depth::PURE
                    && (latent.holds(depth) || result_depth == depth)
                    && let Some(what) = self.name_of(callee)
                {
                    return Some(Blame { span: x.span, what });
                }
            }
            ExprKind::Local(id) => {
                if seen_locals.contains(id) {
                    return None;
                }
                seen_locals.push(*id);
                return self.find_binding(*id).and_then(|bound| {
                    self.blame_walk(bound, depth, through_shade, seen_locals, seen_globals)
                });
            }
            ExprKind::Global(id) => {
                if seen_globals.contains(id) {
                    return None;
                }
                seen_globals.push(*id);
                return self.unit.global(*id).and_then(|g| {
                    self.blame_walk(&g.value, depth, through_shade, seen_locals, seen_globals)
                });
            }
            _ => {}
        }
        if let Some(found) = children(x)
            .into_iter()
            .find_map(|c| self.blame_walk(c, depth, through_shade, seen_locals, seen_globals))
        {
            return Some(found);
        }
        // A descent is the answer only when nothing inside it is: `read` is
        // what somebody wrote, and `descend disk` is where they said they were
        // going.
        if let ExprKind::Descend { capability, .. } = &x.kind
            && capability.stratum() == depth
        {
            return Some(Blame { span: x.span, what: format!("descend {capability}") });
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
        block
            .children()
            .find_map(|x| self.blame_walk(x, depth, false, &mut seen_locals, &mut seen_globals))
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
        if let Some(asserted) = asserted
            && asserted != derived
        {
            let blame = blame(self, extra_stratum(derived, asserted));
            self.fault_blaming(span, FaultKind::AssertedLatent { derived, asserted }, blame);
        }
    }

    fn asserted_lazy<F>(&mut self, span: Span, derived: Depth, asserted: Option<Depth>, blame: F)
    where
        F: FnOnce(&Self) -> Option<Blame>,
    {
        if let Some(asserted) = asserted
            && asserted != derived
        {
            let blame = blame(self);
            self.fault_blaming(span, FaultKind::Asserted { derived, asserted }, blame);
        }
    }

    fn expect_ty(&mut self, x: &Expr, want: &Type, why: &'static str) {
        if x.ty != *want {
            self.malformed(x.span, why);
        }
    }

    fn local_def(&self, id: LocalId) -> Option<&'a LocalDef> {
        self.func.map_or_else(|| self.unit.local(id), |f| f.local(id))
    }

    fn bind_slot(&mut self, id: LocalId, depth: Depth, span: Span) -> bool {
        match self.locals.get_mut(id.0 as usize) {
            Some(slot) if slot.is_some() => {
                self.malformed(span, "this local is already bound here");
                return false;
            }
            Some(slot) => *slot = Some(depth),
            None => {
                self.malformed(span, "this binding names no local");
                return false;
            }
        }
        if let Some(slot) = self.named.get_mut(id.0 as usize) {
            *slot = None;
        }
        true
    }

    fn bind(&mut self, id: LocalId, depth: Depth, span: Span) {
        if self.bind_slot(id, depth, span)
            && self.local_def(id).is_some_and(|l| l.asserted.is_some())
            && !self.deferred_asserts.contains(&id)
        {
            self.deferred_asserts.push(id);
        }
    }

    fn declare(&mut self, id: LocalId) {
        let span = self.local_def(id).map_or_else(Span::default, |l| l.span);
        if self.bind_slot(id, Depth::PURE, span) && !self.deferred_asserts.contains(&id) {
            self.deferred_asserts.push(id);
        }
    }

    fn flush_field_asserts(&mut self, id: LocalId) {
        let mut i = 0;
        while i < self.deferred_field_asserts.len() {
            if self.deferred_field_asserts[i].local == id {
                let f = self.deferred_field_asserts.swap_remove(i);
                if f.depth != f.asserted {
                    self.fault(
                        f.span,
                        FaultKind::Asserted { derived: f.depth, asserted: f.asserted },
                    );
                }
            } else {
                i += 1;
            }
        }
    }

    fn check_deferred_asserts(&mut self) {
        for id in core::mem::take(&mut self.deferred_asserts) {
            self.check_deferred_local(id);
        }
        for f in core::mem::take(&mut self.deferred_field_asserts) {
            if f.depth != f.asserted {
                self.fault(f.span, FaultKind::Asserted { derived: f.depth, asserted: f.asserted });
            }
        }
    }

    fn check_deferred_local(&mut self, id: LocalId) {
        let Some(derived) = self.locals.get(id.0 as usize).copied().flatten() else {
            return;
        };
        let Some(l) = self.local_def(id) else {
            return;
        };
        self.asserted_lazy(l.span, derived, l.asserted, |this| {
            this.find_binding(id)
                .and_then(|e| this.blame(e, derived))
                .or_else(|| this.func.and_then(|f| this.blame_block(&f.body, derived)))
        });
        self.flush_field_asserts(id);
    }

    fn local_ty(&self, id: LocalId) -> Option<&'a Type> {
        self.local_def(id).map(|l| &l.ty)
    }

    fn field_def(&self, base_ty: &Type, index: u32) -> Option<&'a Field> {
        let Type::Struct(name) = base_ty else { return None };
        self.unit.struct_def(name)?.fields.get(index as usize)
    }

    fn field_ty(&self, base_ty: &Type, index: u32) -> Option<&'a Type> {
        self.field_def(base_ty, index).map(|f| &f.ty)
    }

    fn place_ty_and_asserts(&self, place: &Place) -> Option<(&'a Type, Vec<FieldAssert>)> {
        let mut cur = self.local_ty(place.local)?;
        let mut asserts = Vec::new();
        let mut path_prefix = Vec::new();
        let last = place.path.len().saturating_sub(1);
        for (i, proj) in place.path.iter().enumerate() {
            cur = match proj {
                Proj::Field(idx) => {
                    let field = self.field_def(cur, *idx)?;
                    path_prefix.push(*idx);
                    if let Some(d) = field.asserted {
                        asserts.push((path_prefix.clone(), d, i == last));
                    }
                    &field.ty
                }
                Proj::Index(_) => match cur {
                    Type::Array { elem, .. } => {
                        path_prefix.push(u32::MAX);
                        elem
                    }
                    _ => return None,
                },
            };
        }
        Some((cur, asserts))
    }

    fn unit_frame(&mut self) {
        self.func = None;
        self.func_idx = None;
        self.locals = vec![None; self.unit.locals.len()];
        self.local_param_deps.clear();
        self.named = vec![None; self.unit.locals.len()];
        self.deferred_asserts.clear();
        self.deferred_field_asserts.clear();
        self.in_loop = 0;
    }

    fn func_signature(&mut self, idx: usize, f: &'a FuncDef) -> (Depth, Held) {
        self.func = Some(f);
        self.func_idx = Some(idx);
        self.locals = vec![None; f.locals.len()];
        self.local_param_deps = vec![0; f.locals.len()];
        self.named = vec![None; f.locals.len()];
        self.deferred_asserts.clear();
        self.deferred_field_asserts.clear();
        self.returns = Depth::PURE;
        self.needs = Some(Held::NONE);
        self.in_loop = 0;
        for (pi, p) in f.params.iter().enumerate() {
            self.bind(*p, Depth::PURE, f.span);
            if let Some(slot) = self.local_param_deps.get_mut(p.0 as usize) {
                *slot = param_bit(pi);
            }
        }

        let (body_depth, _, _) = self.block(&f.body, Held::NONE, Pc::PURE);
        match &f.body.tail {
            Some(tail) if !self.depth_types_agree(&tail.ty, &f.ret) => {
                self.malformed(tail.span, "shade origin does not match return type");
            }
            None if !block_always_returns(&f.body)
                && !block_ends_with_value_select(&f.body)
                && f.ret != Type::Unit =>
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
        self.func = None;
        self.func_idx = None;
        (ret_depth, latent)
    }

    fn func(&mut self, idx: usize, f: &'a FuncDef) {
        let (ret_depth, latent) = self.func_signature(idx, f);
        self.func = Some(f);
        self.func_idx = Some(idx);
        self.against_lazy(f.span, ret_depth, f.ret_depth, f.asserted_ret, |this| {
            this.blame_block(&f.body, ret_depth)
        });
        self.against_latent_lazy(f.span, latent, f.latent, f.asserted_latent, |this, target| {
            this.blame_block(&f.body, target)
        });
        self.func = None;
        self.func_idx = None;
    }

    fn block(&mut self, block: &Block, ambient: Held, mut pc: Pc) -> (Depth, ExitPc, u64) {
        let mut exit = ExitPc::NONE;
        let mut block_locals: Vec<LocalId> = Vec::new();
        for s in &block.stmts {
            match s {
                Stmt::Let { local, value } => {
                    let (d, ex, val_deps) = self.expr(value, ambient, pc);
                    if let Some(lty) = self.local_ty(*local)
                        && !self.depth_types_agree(&value.ty, lty)
                    {
                        self.malformed(value.span, "shade origin does not match binding type");
                    }
                    let was_unbound =
                        self.locals.get(local.0 as usize).is_some_and(Option::is_none);
                    self.bind(*local, d, value.span);
                    if let Some(slot) = self.local_param_deps.get_mut(local.0 as usize) {
                        *slot = val_deps | pc.deps;
                    }
                    if was_unbound {
                        block_locals.push(*local);
                    }
                    exit = exit.join(ex);
                    pc.advance(ex);
                }
                Stmt::Expr(x) => {
                    let (_, ex, _) = self.expr(x, ambient, pc);
                    // §4.7: a bare expression statement deposits its value.
                    self.name_local(x);
                    exit = exit.join(ex);
                    pc.advance(ex);
                }
                // A local with no value is pure until something is written to
                // it, and `Assign` joins the depth of what it writes. §5.4.
                Stmt::Declare { local } => {
                    let was_unbound =
                        self.locals.get(local.0 as usize).is_some_and(Option::is_none);
                    self.declare(*local);
                    if let Some(slot) = self.local_param_deps.get_mut(local.0 as usize) {
                        *slot = 0;
                    }
                    if was_unbound {
                        block_locals.push(*local);
                    }
                }
            }
        }
        let tail = block
            .tail
            .as_ref()
            .map_or((Depth::PURE, ExitPc::NONE, 0), |t| self.expr(t, ambient, pc));
        for id in block_locals {
            if let Some(pos) = self.deferred_asserts.iter().position(|d| *d == id) {
                self.deferred_asserts.swap_remove(pos);
                self.check_deferred_local(id);
            } else {
                self.flush_field_asserts(id);
            }
            self.clear_local(id);
        }
        (tail.0, exit.join(tail.1), tail.2)
    }

    fn expr(&mut self, x: &Expr, ambient: Held, pc: Pc) -> (Depth, ExitPc, u64) {
        let (derived, exit, deps) = match &x.kind {
            // [LIT]. This is why pure code disappears at burial.
            ExprKind::Literal(_) => {
                if self.carries_depth_type(&x.ty) {
                    self.malformed(x.span, "literal cannot fabricate a shaded or function type");
                }
                (Depth::PURE, ExitPc::NONE, 0)
            }
            ExprKind::Break | ExprKind::Continue => {
                if self.in_loop == 0 {
                    self.malformed(x.span, "loop jump outside a loop");
                }
                if x.ty != Type::Unit {
                    self.malformed(x.span, "loop jump produces U0");
                }
                let exit = ExitPc {
                    loop_exit: Some(pc.depth),
                    fn_exit: None,
                    loop_deps: pc.deps,
                    fn_deps: 0,
                };
                (Depth::PURE, exit, 0)
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
                let dep = self.local_param_deps.get(id.0 as usize).copied().unwrap_or(0);
                (d, ExitPc::NONE, dep)
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
                (d, ExitPc::NONE, 0)
            }
            // [ABS]. Building a function that will touch the disk does not
            // touch the disk, so the closure itself is pure.
            ExprKind::Func(id) => self.func_expr(x, *id),
            ExprKind::Prim(p) => {
                self.prim(x, *p);
                (Depth::PURE, ExitPc::NONE, 0)
            }
            ExprKind::Call { callee, args } => self.call_expr(x, callee, args, ambient, pc),
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
                if self.carries_depth_type(&x.ty) || self.carries_depth_type(&operand.ty) {
                    self.malformed(
                        x.span,
                        "unary operation cannot take or fabricate a shaded or function type",
                    );
                }
                self.expr(operand, ambient, pc)
            }
            ExprKind::Field { base, index } => self.field_expr(x, base, *index, ambient, pc),
            ExprKind::Binary { op, lhs, rhs } => self.binary_expr(x, *op, lhs, rhs, ambient, pc),
            ExprKind::Index { base, index } => self.index_expr(x, base, index, ambient, pc),
            ExprKind::Select { cond, then, otherwise } => {
                self.select_expr(x, cond, then, otherwise, ambient, pc)
            }
            ExprKind::Loop { body, step } => self.loop_expr(x, body, step.as_deref(), ambient, pc),
            ExprKind::Return(v) => self.return_expr(x, v.as_deref(), ambient, pc),
            ExprKind::Block(b) => self.block_expr(x, b, ambient, pc),
            ExprKind::Assign { place, value } => self.assign_expr(x, place, value, ambient, pc),
        };

        if derived != x.depth {
            self.fault(x.span, FaultKind::Stated { derived, stated: x.depth });
        }
        (derived, exit, deps)
    }

    fn func_expr(&mut self, x: &Expr, id: FuncId) -> (Depth, ExitPc, u64) {
        match self.unit.func(id) {
            Some(f) => {
                let params =
                    f.params.iter().filter_map(|p| f.local(*p)).map(|l| l.ty.clone()).collect();
                let want = Type::Fn {
                    params,
                    latent: f.latent,
                    result: Box::new(f.ret.clone()),
                    result_depth: f.ret_depth,
                };
                self.expect_ty(x, &want, "this is not the signature of that function");
            }
            None => self.malformed(x.span, "this names no function in the unit"),
        }
        (Depth::PURE, ExitPc::NONE, 0)
    }

    fn field_expr(
        &mut self,
        x: &Expr,
        base: &Expr,
        index: u32,
        ambient: Held,
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let res = self.expr(base, ambient, pc);
        if let Some(fty) = self.field_ty(&base.ty, index) {
            if !self.depth_types_agree(&x.ty, fty) {
                self.malformed(x.span, "shade origin does not match field type");
            }
        } else if self.carries_depth_type(&x.ty) || self.carries_depth_type(&base.ty) {
            self.malformed(x.span, "unknown struct or field");
        }
        res
    }

    fn binary_expr(
        &mut self,
        x: &Expr,
        op: BinOp,
        lhs: &Expr,
        rhs: &Expr,
        ambient: Held,
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        if self.carries_depth_type(&x.ty) {
            self.malformed(x.span, "binary operation cannot fabricate a shaded or function type");
        }
        match op {
            BinOp::Eq | BinOp::Ne => {
                if !self.depth_types_agree(&lhs.ty, &rhs.ty) {
                    self.malformed(x.span, "shade origin does not match across comparison");
                }
            }
            _ => {
                if self.carries_depth_type(&lhs.ty) || self.carries_depth_type(&rhs.ty) {
                    self.malformed(
                        x.span,
                        "binary operation cannot take a shaded or function operand",
                    );
                }
            }
        }
        let (dl, el, l_deps) = self.expr(lhs, ambient, pc);
        let (dr, er, r_deps) = self.expr(rhs, ambient, pc.with_exit(el));
        (dl.join(dr), el.join(er), l_deps | r_deps)
    }

    fn index_expr(
        &mut self,
        x: &Expr,
        base: &Expr,
        index: &Expr,
        ambient: Held,
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let (dl, el, l_deps) = self.expr(base, ambient, pc);
        let (dr, er, r_deps) = self.expr(index, ambient, pc.with_exit(el));
        if self.carries_depth_type(&index.ty) {
            self.malformed(index.span, "array index cannot be a shaded or function type");
        }
        if let Type::Array { elem, .. } = &base.ty {
            if !self.depth_types_agree(&x.ty, elem) {
                self.malformed(x.span, "shade origin does not match element type");
            }
        } else if self.carries_depth_type(&x.ty) || self.carries_depth_type(&base.ty) {
            self.malformed(x.span, "indexed base is not an array");
        }
        (dl.join(dr), el.join(er), l_deps | r_deps)
    }

    fn return_expr(
        &mut self,
        x: &Expr,
        v: Option<&Expr>,
        ambient: Held,
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        if self.func.is_none() {
            self.malformed(x.span, "return outside a function");
        }
        if x.ty != Type::Unit {
            self.malformed(x.span, "return produces U0");
        }
        let (d, ex, v_deps) =
            v.map_or((Depth::PURE, ExitPc::NONE, 0), |v| self.expr(v, ambient, pc));
        if let Some(v) = v {
            self.name_local(v);
            if let Some(f) = self.func
                && !self.depth_types_agree(&v.ty, &f.ret)
            {
                self.malformed(v.span, "shade origin does not match return type");
            }
        } else if let Some(f) = self.func
            && f.ret != Type::Unit
        {
            self.malformed(x.span, "empty return does not match function return type");
        }
        let ret_pc = pc.with_exit(ex);
        self.returns = self.returns.join(d).join(ret_pc.depth);
        let out = ExitPc {
            loop_exit: None,
            fn_exit: Some(ret_pc.depth),
            loop_deps: 0,
            fn_deps: ret_pc.deps,
        };
        (d, ex.join(out), v_deps)
    }

    fn block_expr(&mut self, x: &Expr, b: &Block, ambient: Held, pc: Pc) -> (Depth, ExitPc, u64) {
        let res = self.block(b, ambient, pc);
        match &b.tail {
            Some(tail) if !self.depth_types_agree(&x.ty, &tail.ty) => {
                self.malformed(x.span, "shade origin does not match block tail");
            }
            None if x.ty != Type::Unit => {
                self.malformed(x.span, "block without tail produces U0");
            }
            _ => {}
        }
        res
    }

    /// [APP]. Three depths in the result, not two: the latent depth of the
    /// arrow, the depth of the function value itself, and the arguments'.
    ///
    /// One of the three is the premise. A capability is what it takes to reach
    /// a stratum, and the latent depth is the only term that reaches one: the
    /// other two are facts about where those values have already been.
    fn call_expr(
        &mut self,
        x: &Expr,
        callee: &Expr,
        args: &[Expr],
        ambient: Held,
        mut pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let (d_f, mut ex, f_deps) = self.expr(callee, ambient, pc);
        pc.advance(ex);
        let track_args = self.has_shade_bounds && !matches!(callee.kind, ExprKind::Prim(_));
        let mut arg_info = if track_args { Vec::with_capacity(args.len()) } else { Vec::new() };
        let mut d_a = Depth::PURE;
        let mut a_deps = 0u64;
        for a in args {
            let (d, a_ex, adep) = self.expr(a, ambient, pc);
            // Passing a local hands its value somewhere this function cannot
            // see, which is naming it immediately before later arguments
            // evaluate. §5.4.
            self.name_local(a);
            if track_args {
                arg_info.push((d, adep));
            }
            d_a = d_a.join(d);
            a_deps |= adep;
            ex = ex.join(a_ex);
            pc.advance(a_ex);
        }
        let (latent, result_depth) =
            if let Type::Fn { params, latent, result, result_depth } = &callee.ty {
                if args.len() != params.len() {
                    self.malformed(x.span, "wrong number of arguments");
                }
                for (arg, param) in args.iter().zip(params) {
                    if !self.depth_types_agree(&arg.ty, param) {
                        self.malformed(arg.span, "shade origin does not match parameter type");
                    }
                }
                if !self.depth_types_agree(&x.ty, result) {
                    self.malformed(x.span, "shade origin does not match call result type");
                }
                self.resolve_call_sig(
                    callee,
                    (*latent, *result_depth),
                    args,
                    &arg_info,
                    track_args,
                    pc.deps,
                )
            } else {
                self.malformed(callee.span, "this is applied and is not a function");
                (Held::NONE, Depth::PURE)
            };
        if !latent.subset_of(ambient) {
            // Inside a function body this is not a fault: it is what the
            // function asks of whoever calls it. At the top level of a file
            // there is nobody to ask.
            let missing = latent.without(ambient);
            if let Some(needs) = self.needs {
                self.needs = Some(needs.union(missing));
            } else {
                let blame = self.name_of(callee).map(|what| Blame { span: x.span, what });
                let kind = FaultKind::Ungranted { needed: missing.deepest(), held: ambient };
                self.fault_blaming(x.span, kind, blame);
            }
        }
        (result_depth.join(d_f).join(d_a), ex, f_deps | a_deps)
    }

    fn resolve_call_sig(
        &mut self,
        callee: &Expr,
        fallback: (Held, Depth),
        args: &[Expr],
        arg_info: &[(Depth, u64)],
        track_args: bool,
        pc_deps: u64,
    ) -> (Held, Depth) {
        if let ExprKind::Func(id) = callee.kind {
            if track_args {
                self.check_callee_shade_bounds(id, args, arg_info, pc_deps);
            }
            return self.func_sigs.get(id.0 as usize).map_or(fallback, |&(dr, dl)| (dl, dr));
        }
        if matches!(callee.kind, ExprKind::Prim(_)) {
            return fallback;
        }
        let mut targets = Vec::new();
        Self::callee_funcs(callee, &mut targets);
        if targets.is_empty() {
            return fallback;
        }
        let (mut derived_ret, mut derived_latent) = (Depth::PURE, Held::NONE);
        for fid in targets {
            if track_args {
                self.check_callee_shade_bounds(fid, args, arg_info, pc_deps);
            }
            if let Some(&(dr, dl)) = self.func_sigs.get(fid.0 as usize) {
                derived_ret = derived_ret.join(dr);
                derived_latent = derived_latent.union(dl);
            }
        }
        (derived_latent, derived_ret)
    }

    /// §5.4: the arms are exclusive, so neither precedes the other and a naming
    /// on one does not freeze the other. Each is walked from where the block
    /// started; what they name is joined afterwards, because it is named for
    /// everything *after* the block.
    fn select_expr(
        &mut self,
        x: &Expr,
        cond: &Expr,
        then: &Expr,
        otherwise: &Expr,
        ambient: Held,
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let (d_cond, e_cond, c_deps) = self.expr(cond, ambient, pc);
        if self.carries_depth_type(&cond.ty) {
            self.malformed(cond.span, "branch condition cannot be a shaded or function type");
        }
        let mut branch_pc = pc.with_exit(e_cond);
        branch_pc.depth = branch_pc.depth.join(d_cond);
        branch_pc.deps |= c_deps;
        let (named, locals, deps) =
            (self.named.clone(), self.locals.clone(), self.local_param_deps.clone());
        let (d_then, e_then, t_deps) = self.expr(then, ambient, branch_pc);
        let from_then = core::mem::replace(&mut self.named, named);
        let locals_then = core::mem::replace(&mut self.locals, locals);
        let deps_then = core::mem::replace(&mut self.local_param_deps, deps);
        let (d_else, e_else, el_deps) = self.expr(otherwise, ambient, branch_pc);
        for (slot, was) in self.named.iter_mut().zip(&from_then) {
            *slot = slot.or(*was);
        }
        for (s, w) in self.locals.iter_mut().zip(&locals_then) {
            *s = Depth::join_opt(*s, *w);
        }
        for (s, w) in self.local_param_deps.iter_mut().zip(&deps_then) {
            *s |= *w;
        }
        if !self.depth_types_agree(&then.ty, &x.ty) || !self.depth_types_agree(&otherwise.ty, &x.ty)
        {
            self.malformed(x.span, "shade origin does not match branch arms");
        }
        (
            d_cond.join(d_then).join(d_else),
            e_cond.join(e_then).join(e_else),
            c_deps | t_deps | el_deps,
        )
    }

    /// §5.4: a loop body runs again, so a naming anywhere in it precedes an
    /// assignment anywhere in it — including one written above it. A turn
    /// starts where the last one ended, so the body is walked until a turn ends
    /// where it started, and only that walk's faults are kept.
    fn loop_expr(
        &mut self,
        x: &Expr,
        body: &Expr,
        step: Option<&Expr>,
        ambient: Held,
        mut loop_pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        if x.ty != Type::Unit {
            self.malformed(x.span, "loop produces U0");
        }
        let mut inner = bound_inside(body);
        if let Some(s) = step {
            inner.extend(bound_inside(s));
        }
        // Only clear locals that were unbound on loop entry so an outer local
        // reused inside a loop is not silently wiped.
        inner.retain(|id| self.locals.get(id.0 as usize).is_some_and(Option::is_none));
        self.in_loop += 1;
        let res = loop {
            let mark = self.faults.len();
            let was = (self.carried(&inner), loop_pc);
            for &id in &inner {
                self.clear_local(id);
            }
            let (d_body, e_body, b_deps) = self.expr(body, ambient, loop_pc);
            let step_pc = loop_pc.with_exit(e_body);
            let (d, e_step, s_deps) = match step {
                Some(s) => {
                    let (ds, es, step_deps) = self.expr(s, ambient, step_pc);
                    (d_body.join(ds), es, b_deps | step_deps)
                }
                None => (d_body, ExitPc::NONE, b_deps),
            };
            let combined = e_body.join(e_step);
            loop_pc.advance(combined);
            let out_exit = ExitPc {
                loop_exit: None,
                fn_exit: combined.fn_exit,
                loop_deps: 0,
                fn_deps: combined.fn_deps,
            };
            if (self.carried(&inner), loop_pc) == was {
                break (d, out_exit, s_deps);
            }
            self.fixpoint_steps += 1;
            if self.fixpoint_steps > MAX_FIXPOINT_STEPS {
                self.faults.truncate(mark);
                self.malformed(x.span, "loop depth inference exceeded iteration limit");
                break (d, out_exit, s_deps);
            }
            self.faults.truncate(mark);
        };
        self.in_loop -= 1;
        res
    }

    /// A write into a binding deepens it. Nothing lowers a depth, so a binding
    /// is as deep as the deepest thing ever written into it, and no program can
    /// observe the difference: a value that has been read has already been
    /// named. `spec/05-types.md` §5.4.
    fn assign_expr(
        &mut self,
        x: &Expr,
        place: &Place,
        value: &Expr,
        ambient: Held,
        mut pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let mut ex = ExitPc::NONE;
        let mut d_path = Depth::PURE;
        let mut path_deps = 0u64;
        for p in &place.path {
            if let Proj::Index(i) = p {
                let (di, ei, idep) = self.expr(i, ambient, pc);
                if self.carries_depth_type(&i.ty) {
                    self.malformed(i.span, "array index cannot be a shaded or function type");
                }
                path_deps |= idep;
                d_path = d_path.join(di);
                ex = ex.join(ei);
                pc.advance(ei);
            }
        }
        let (d_value, e_val, val_deps) = self.expr(value, ambient, pc);
        ex = ex.join(e_val);
        pc.advance(e_val);
        let mut field_asserts = Vec::new();
        if let Some((pty, asserts)) = self.place_ty_and_asserts(place) {
            if !self.depth_types_agree(&value.ty, pty) {
                self.malformed(value.span, "shade origin does not match assigned place");
            }
            field_asserts = asserts;
        } else if self.carries_depth_type(&value.ty) {
            self.malformed(x.span, "assigned place has no valid type");
        }
        if x.ty != Type::Unit {
            self.malformed(x.span, "assignment produces U0");
        }
        if let Some(Some(named)) = self.named.get(place.local.0 as usize).copied() {
            self.fault(x.span, FaultKind::Frozen { named });
        }
        let written = d_value.join(d_path).join(pc.depth);
        for (path, asserted, is_leaf) in field_asserts {
            if is_leaf {
                self.asserted_lazy(x.span, written, Some(asserted), |this| {
                    this.blame(value, written)
                });
            } else if let Some(entry) = self
                .deferred_field_asserts
                .iter_mut()
                .find(|e| e.local == place.local && e.path == path)
            {
                entry.depth = entry.depth.join(written);
            } else {
                self.deferred_field_asserts.push(DeferredFieldAssert {
                    local: place.local,
                    path,
                    asserted,
                    depth: written,
                    span: x.span,
                });
            }
        }
        if let Some(slot) = self.local_param_deps.get_mut(place.local.0 as usize) {
            *slot |= val_deps | path_deps | pc.deps;
        }
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
        (d_value.join(d_path), ex, val_deps | path_deps)
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
                    if let Some(slot) = self.named.get_mut(id.0 as usize)
                        && slot.is_none()
                    {
                        *slot = Some(cur.span);
                    }
                }
                ExprKind::Rite { rite: Rite::Opaque, operand } => todo.push(operand),
                ExprKind::Descend { body, .. } => todo.push(body),
                ExprKind::Block(b) => todo.extend(b.tail.as_deref()),
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
        pc: Pc,
    ) -> (Depth, ExitPc, u64) {
        let (d, ex, op_deps) = self.expr(operand, ambient, pc);
        // `seal` and `shade` take the value itself; `look` and `opaque` pass
        // one through without giving it a name of its own.
        if matches!(rite, Rite::Seal | Rite::Shade) {
            self.name_local(operand);
        }
        let (derived, out_deps) = match rite {
            // A cairn is a name, and a name is pure whatever it names.
            Rite::Seal => {
                if x.ty != Type::Cairn {
                    self.malformed(x.span, "`seal` produces a cairn");
                }
                (Depth::PURE, 0)
            }
            // The origin is carried in the type, which is what makes [LOOK]
            // checkable at all.
            Rite::Shade => {
                let want = Type::Shade { origin: d, inner: Box::new(operand.ty.clone()) };
                self.expect_ty(x, &want, "this shade claims an origin its value never had");
                let deps = op_deps | pc.deps;
                if deps != 0 {
                    self.record_param_shade_bound(deps, d);
                }
                (Depth::PURE, 0)
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
                        let blame = self.blame_shade(operand, *origin);
                        let kind = FaultKind::Orpheus { origin: *origin, held: ambient };
                        self.fault_blaming(x.span, kind, blame);
                    }
                    // Deep for two independent reasons: where the value came
                    // from, and where the shade itself came from.
                    (origin.join(d), op_deps)
                } else {
                    self.malformed(operand.span, "`look` needs a shade");
                    (d, op_deps)
                }
            }
            // Opaque is a fact about evaluation and none about the calculus.
            Rite::Opaque => {
                if !self.depth_types_agree(&x.ty, &operand.ty) {
                    self.malformed(x.span, "shade origin does not match opaque operand");
                }
                (d, op_deps)
            }
        };
        (derived, ex, out_deps)
    }

    /// A prelude reference states an instantiated signature. Only the latent
    /// depth and shade-origin preservation on polymorphic forms are checked:
    /// five of the thirty-two are polymorphic in `T`, and the IR's types
    /// deliberately have no variables.
    fn prim(&mut self, x: &Expr, p: Prim) {
        // Every prelude function hands back what it went and got, so its two
        // depths are the same one. Nothing else in the language has to be.
        let Type::Fn { params, latent, result, result_depth } = &x.ty else {
            self.malformed(x.span, "a prelude function is a function");
            return;
        };
        if *latent != Held::of(p.latent()) || *result_depth != p.latent() {
            self.malformed(x.span, "this prelude function has another stratum");
            return;
        }
        match p {
            Prim::Answered
                if params.len() != 1
                    || !matches!(result.as_ref(), Type::Answer(inner) if self.depth_types_agree(&params[0], inner)) =>
            {
                self.malformed(x.span, "shade origin does not match `answered` result");
            }
            Prim::Must
                if params.len() != 1
                    || !matches!(&params[0], Type::Answer(inner) if self.depth_types_agree(inner, result)) =>
            {
                self.malformed(x.span, "shade origin does not match `must` result");
            }
            Prim::Refused
                if params.len() != 1
                    || self.carries_depth_type(&params[0])
                    || !matches!(result.as_ref(), Type::Answer(_)) =>
            {
                self.malformed(x.span, "`refused` produces an `Answer<T>`");
            }
            Prim::Given | Prim::Refusal
                if params.len() != 1
                    || !matches!(&params[0], Type::Answer(_))
                    || self.carries_depth_type(result) =>
            {
                self.malformed(
                    x.span,
                    "answer inspector takes `Answer<T>` and returns no shade or function",
                );
            }
            Prim::Answered | Prim::Must | Prim::Refused | Prim::Given | Prim::Refusal => {}
            _ if self.carries_depth_type(result)
                || params.iter().any(|t| self.carries_depth_type(t)) =>
            {
                self.malformed(
                    x.span,
                    "monomorphic prelude function cannot take or return a shade or function",
                );
            }
            _ => {}
        }
    }

    fn carries_depth_type(&self, ty: &Type) -> bool {
        self.carries_depth_inner(ty, &mut Vec::new())
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
                self.unit
                    .struct_def(name)
                    .is_some_and(|s| s.fields.iter().any(|f| self.carries_depth_inner(&f.ty, seen)))
            }
            _ => false,
        }
    }

    /// Whether `got` agrees with `want` on every depth-bearing type (`Shade`
    /// origin, `Fn` latent/result depth, containers, fixed array length, and
    /// nominal `Struct`s).
    fn depth_types_agree(&self, got: &Type, want: &Type) -> bool {
        match (got, want) {
            (Type::Shade { origin: a, inner: ia }, Type::Shade { origin: b, inner: ib }) => {
                a == b && self.depth_types_agree(ia, ib)
            }
            (Type::Answer(a), Type::Answer(b)) => self.depth_types_agree(a, b),
            (Type::Array { elem: a, len: la }, Type::Array { elem: b, len: lb }) => {
                (la == lb || lb.is_none()) && self.depth_types_agree(a, b)
            }
            (Type::Struct(a), Type::Struct(b)) => a == b,
            (Type::Struct(_), _) | (_, Type::Struct(_)) => false,
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
