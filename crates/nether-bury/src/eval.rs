use nether_core::{
    BinOp, Block, Capability, Depth, Expr, ExprKind, FuncDef, FuncId, GlobalId, Held, Literal,
    LocalDef, LocalId, Place, Prim, Rite, Span, Stmt, Type, UnOp,
};
use nether_ledger::{NonEmptyName, Stored, Value};

use crate::fold::{self, PrimErr};
use crate::residue::{Halt, HaltKind, MAX_EXPR_DEPTH, MAX_FRAMES};
use crate::state::{Burial, Checkpoint, Env, Flow, Grind, Kind, LocalSet, Open, Val, ValLit};
use crate::{analysis, assign_local, tail_block, u32_index, unit_block};

impl Burial<'_> {
    /// `given`, `refusal` and `must`, which are how §9.9 says a program
    /// handles a no.
    fn inspect(&mut self, p: Prim, values: &[Val], x: &Expr) -> Result<Option<Val>, Halt> {
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
            (Prim::Must, Kind::Refused(_)) => {
                return Err(Halt::collapsed(x.span, "this answer was refused"));
            }
            (Prim::Given, Kind::Answered(_)) => Some(Val::known(Literal::Bool(true), x)),
            (Prim::Given, Kind::Refused(_)) => Some(Val::known(Literal::Bool(false), x)),
            (Prim::Refusal, Kind::Refused(r)) => Some(Val::known(Literal::Refusal(*r), x)),
            (Prim::Refusal, Kind::Answered(_)) => {
                return Err(Halt::collapsed(x.span, "this answer was given"));
            }
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
            return fold::literal(said).map(|l| Val {
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
            nether_ledger::AnswerOf::Refused(r) => Kind::Refused(fold::code(*r)),
            nether_ledger::AnswerOf::Given(v) => {
                let l = fold::literal(v)?;
                Kind::Answered(Box::new(Val {
                    kind: Kind::Known(ValLit::from_literal(l)),
                    ty: inner_ty,
                    depth: x.depth,
                }))
            }
        };
        Some(Val { kind, ty: x.ty.clone(), depth: x.depth })
    }

    /// A closed, zero-depth expression for a finished value that references no
    /// caller locals, capabilities, or world calls.
    pub(crate) fn closed_residual(v: &Val, span: Span) -> Option<Expr> {
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
                Some(Self::prim_call(Prim::Answered, arg, v.ty.clone(), span))
            }
            Kind::Refused(r) if v.depth == Depth::PURE => {
                let arg = Expr {
                    kind: ExprKind::Literal(Literal::Refusal(*r)),
                    ty: Type::Refusal,
                    depth: Depth::PURE,
                    span,
                };
                Some(Self::prim_call(Prim::Refused, arg, v.ty.clone(), span))
            }
            _ => None,
        }
    }

    fn prim_call(p: Prim, arg: Expr, ty: Type, span: Span) -> Expr {
        let callee = Expr {
            kind: ExprKind::Prim(p),
            ty: Type::Fn {
                params: vec![arg.ty.clone()],
                latent: Held::NONE,
                result: Box::new(ty.clone()),
                result_depth: Depth::PURE,
            },
            depth: Depth::PURE,
            span,
        };
        Expr {
            kind: ExprKind::Call { callee: Box::new(callee), args: vec![arg] },
            ty,
            depth: Depth::PURE,
            span,
        }
    }

    /// The expression a value residualises as.
    ///
    /// A value that can be written down is written down; anything else is the
    /// expression it came from, unreduced.
    pub(crate) fn residual(v: Val, original: &Expr) -> Expr {
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
            // is the question it came from and stays a hole for a later burial.
            Kind::Stuck(e) => *e,
            _ => original.clone(),
        }
    }

    /// `x` left as written: after the assignments it needs, and with what it
    /// writes no longer known here.
    fn unreduced(&mut self, x: &Expr, env: &mut Env) -> Val {
        let settled = analysis::leave(&[x], self.pc, env);
        self.settling.extend(settled);
        Val::stuck(x)
    }

    pub(crate) fn expr(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        if self.expr_depth >= MAX_EXPR_DEPTH {
            return Err(Halt::at(x.span, HaltKind::TooDeep { limit: MAX_EXPR_DEPTH }));
        }
        self.expr_depth += 1;
        let res = self.expr_inner(x, env);
        self.expr_depth -= 1;
        res
    }

    fn expr_inner(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.charge(1, x.span)?;
        Ok(match &x.kind {
            ExprKind::Literal(l) => Val::known(l.clone(), x),
            ExprKind::Local(id) => match env.get(id.0 as usize).and_then(Clone::clone) {
                Some(v) if v.is_stuck() => Val::rebuild(ExprKind::Local(*id), x, v.depth),
                Some(v) => v,
                None => Val::stuck(x),
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
            ExprKind::Loop { .. } => self.eval_loop(x, env)?,
            ExprKind::Descend { capability, body } => self.descend(x, *capability, body, env)?,
            ExprKind::Rite { rite, operand } => self.rite(x, *rite, operand, env)?,
            ExprKind::Assign { place, value } => self.assign(x, place, value, env)?,
            ExprKind::Break => {
                self.flow = Some(Flow::Broke);
                Val::unit()
            }
            ExprKind::Continue => {
                self.flow = Some(Flow::Continued);
                Val::unit()
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
                    None => Val::unit(),
                };
                self.flow = Some(Flow::Returned(out.clone()));
                out
            }
            // Indexing and projection need an aggregate, and nothing can build one.
            ExprKind::Field { .. } | ExprKind::Index { .. } => self.unreduced(x, env),
        })
    }

    fn unary(&mut self, x: &Expr, op: UnOp, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let v = self.expr(operand, env)?;
        if self.flow.is_some() {
            return Ok(v);
        }
        Ok(if let Some(l) = fold::unary(op, &v) {
            Val::known(l, x)
        } else {
            let operand = Box::new(Self::residual(v, operand));
            let depth = operand.depth;
            Val::rebuild(ExprKind::Unary { op, operand }, x, depth)
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
            analysis::flush_stale_since(None, rhs_gen, env, &mut rhs_prefix);
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
        let folded = fold::binary(op, &a, &b).map_err(|why| Halt::collapsed(x.span, why))?;
        Ok(if let Some(l) = folded {
            Val::known(l, x)
        } else {
            let lhs = Box::new(Self::residual(a, lhs));
            let rhs = Box::new(Self::wrap_prefix(rhs_prefix, Self::residual(b, rhs)));
            let depth = lhs.depth.join(rhs.depth);
            Val::rebuild(ExprKind::Binary { op, lhs, rhs }, x, depth)
        })
    }

    fn unreduced_select(
        &mut self,
        x: &Expr,
        c: Val,
        (cond, then, otherwise): (&Expr, &Expr, &Expr),
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
    /// speculative, which is what keeps it inside §6.2.
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
            return self.select_undecided(x, c, (cond, then, otherwise), env);
        };
        let (arm, other) = if taken { (then, otherwise) } else { (otherwise, then) };
        let (val_deeper, effects_deeper) = analysis::lowers(arm, other, self.pc);
        if !val_deeper && !effects_deeper {
            return self.expr(arm, env);
        }
        self.select_depth_guarded(x, c, (cond, then, otherwise), taken, effects_deeper, env)
    }

    fn select_undecided(
        &mut self,
        x: &Expr,
        c: Val,
        (cond, then, otherwise): (&Expr, &Expr, &Expr),
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let cond = Box::new(Self::residual(c, cond));
        // An arm that writes a local the branch outlives cannot be folded to
        // its value: the write is the part that matters, and which arm makes
        // it is what the world has not said.
        let branch_pc = self.pc.join(cond.depth);
        let (writes, _) = analysis::write_depths(&[then, otherwise]);
        let arms = if writes.is_empty() {
            let saved_pc = self.pc;
            self.pc = branch_pc;
            let res = (|| Ok([self.speculate(then, env)?, self.speculate(otherwise, env)?]))();
            self.pc = saved_pc;
            res?
        } else {
            [then.clone(), otherwise.clone()]
        };
        if analysis::reaches_act(&arms[0], self.unit) || analysis::reaches_act(&arms[1], self.unit)
        {
            self.act_blocked = true;
        }
        let depth = arms.iter().fold(cond.depth, |d, arm| d.join(arm.depth));
        let settled = analysis::leave(&[&arms[0], &arms[1]], branch_pc, env);
        self.settling.extend(settled);
        let [then, otherwise] = arms.map(Box::new);
        Ok(Val::rebuild(ExprKind::Select { cond, then, otherwise }, x, depth))
    }

    /// §6.5, a residue keeps its depths: taking the shallower arm would leave
    /// a residue shallower than the branch, and a `look` that held the branch's
    /// depth may not hold the arm's.
    fn select_depth_guarded(
        &mut self,
        x: &Expr,
        c: Val,
        arms @ (cond, then, otherwise): (&Expr, &Expr, &Expr),
        taken: bool,
        effects_deeper: bool,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let (arm, other) = if taken { (then, otherwise) } else { (otherwise, then) };
        if effects_deeper && other.ty != Type::Unit {
            return Ok(self.unreduced_select(x, c, arms, env));
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
                    analysis::dead_branch_guard(taken, other, x.span)
                };
                let guard_stmt = Stmt::Expr(guard.clone());
                if !self.settling.contains(&guard_stmt) {
                    // Settle against `ran` before emitting the guard so any
                    // local named in the dead arm is not assigned after it.
                    self.settling.extend(analysis::settle(&[&guard], &mut ran));
                    self.settling.push(guard_stmt);
                }
            }
            self.starved = cp.starved;
            *env = ran;
            return Ok(v);
        }
        if self.acts_served_since(&cp) && v.is_stuck() {
            // An act was already served inside `arm`; keep the reduced `arm` in
            // the branch instead of rolling back and duplicating the act.
            let arm_res = Self::residual(v, arm);
            let (new_then, new_other) =
                if taken { (arm_res, other.clone()) } else { (other.clone(), arm_res) };
            let cond_res = Self::residual(c, cond);
            let depth = cond_res.depth.join(new_then.depth).join(new_other.depth);
            *env = ran;
            self.starved = cp.starved;
            return Ok(Val::rebuild(
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
        Ok(self.unreduced_select(x, c, arms, env))
    }

    /// One arm of an undecided branch, reduced as far as speculation allows.
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
        if self.settling.len() > cp.settling {
            self.rollback(cp);
            return Ok(arm.clone());
        }
        let flow = self.flow.take();
        if let Some(Flow::Returned(r)) = flow {
            if matches!(&r.kind, Kind::Stuck(e) if analysis::ends_returning(e)) {
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
                    ExprKind::Block(_) => unit_block(vec![Stmt::Expr(ret)], arm.span),
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
    pub(crate) fn try_expr(&mut self, x: &Expr, env: &mut Env) -> Result<Option<Val>, Halt> {
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
        let block = unit_block(inner, span);
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
            if self.is_stuck_return(r, body.span) {
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
        Ok(Val::rebuild(ExprKind::Descend { capability, body }, x, depth))
    }

    /// A unit-level binding, evaluated once and only if something reaches it.
    fn global(&mut self, id: GlobalId, x: &Expr) -> Result<Val, Halt> {
        self.shares()?;
        if let Some(v) = self.globals.get(id.0 as usize).and_then(Clone::clone) {
            return Ok(v);
        }
        let Some(g) = self.unit.global(id) else {
            return Ok(Val::stuck(x));
        };
        if self.eval_globals.contains(&id) {
            return Err(Halt::collapsed(x.span, "unit-level binding refers to itself"));
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
        let had_effects = self.had_effects(&cp);
        let keep_ref = !self.unit.locals.is_empty()
            || had_effects
            || analysis::reaches_act(&g.value, self.unit)
            || analysis::reaches_deposit(&g.value, self.unit);
        let (v, residual, rollback) = if v.is_stuck() {
            if !keep_ref {
                (v, None, false)
            } else if had_effects && !self.speculating {
                (Val::stuck(x), Some(Self::residual(v, &g.value)), false)
            } else {
                (Val::stuck(x), None, true)
            }
        } else if let Some(closed) = Self::closed_residual(&v, g.span) {
            (v, (!self.speculating).then_some(closed), false)
        } else if had_effects {
            if self.acts_served_since(&cp) && !self.speculating {
                (Val::stuck(x), Some(Self::residual(v, &g.value)), false)
            } else {
                (Val::stuck(x), None, true)
            }
        } else {
            (v, None, false)
        };
        if let Some(expr) = residual
            && let Some(slot) = self.globals_residual.get_mut(id.0 as usize)
        {
            *slot = Some(expr);
        }
        if rollback {
            self.rollback(cp);
        } else {
            self.restore_frame(cp);
        }
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
        let span = res.span;
        tail_block(prefix, res, span)
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
        let Some(args) = values.iter().map(fold::as_value).collect::<Option<Vec<_>>>() else {
            if !shared {
                self.act_blocked = true;
            }
            return Ok(None);
        };
        let arg_bytes: usize =
            values.iter().map(Self::val_byte_work).fold(0usize, usize::saturating_add);
        self.burn_bytes(arg_bytes, x.span)?;
        let Ok(function) = NonEmptyName::new(p.name()) else {
            return Ok(None);
        };
        let asked = nether_ledger::Call::new(
            function,
            args.into_iter().map(|v| self.remember(Stored::Value(v))).collect(),
        );
        let nth = if shared {
            0
        } else {
            self.served.get(&asked).copied().unwrap_or(0)
                + self.reserved_acts.get(&asked).copied().unwrap_or(0)
        };
        if let Some(said) = self.answers.get(&asked, nth).cloned() {
            if let Some(ans) = Self::answered(&said, x) {
                self.answers_used += 1;
                if !shared {
                    *self.served.entry(asked).or_default() += 1;
                    self.acts_served += 1;
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
                analysis::flush_stale_since(None, arg_gen, env, &mut prefix);
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
                let byte_work = fold::prim_byte_work(*p, &values);
                self.burn_bytes(byte_work, x.span)?;
                match fold::prim(*p, &values) {
                    Ok(l) => l.map(|l| Val::known(l, x)),
                    Err(PrimErr::Collapsed(why)) => return Err(Halt::collapsed(x.span, why)),
                    Err(PrimErr::TooLarge(limit)) => {
                        return Err(Halt::at(x.span, HaltKind::TooLarge { limit }));
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
        Ok(Val::rebuild(ExprKind::Call { callee, args }, x, depth))
    }

    /// Evaluate a function body against known arguments.
    fn apply(
        &mut self,
        id: FuncId,
        args: &[Val],
        exprs: &[Expr],
        site: &Expr,
    ) -> Result<Option<Val>, Halt> {
        let Some(f) = self.unit.func(id) else { return Ok(None) };
        if args.iter().any(Val::is_stuck) || args.len() != f.params.len() {
            if analysis::reaches_act(site, self.unit) {
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
            return Err(Halt::at(f.span, HaltKind::TooDeep { limit: MAX_FRAMES }));
        }
        self.grind.push(Grind { span: f.span, what: Open::Call(id) });
        let cp = self.checkpoint();
        self.pc = Depth::PURE;
        let fell = self.block(&f.body, &mut frame)?;
        self.grind.pop();
        let out = match self.flow.take() {
            Some(Flow::Broke | Flow::Continued) => {
                self.rollback(cp);
                return Ok(None);
            }
            Some(Flow::Returned(v)) => v,
            None => fell,
        };
        let body = self.starved.take();
        if !out.is_stuck() {
            self.restore_frame(cp);
            return Ok(Some(out));
        }
        let Some(body) = body else {
            self.rollback(cp);
            return Ok(None);
        };
        if analysis::ret_depth(&body) < site.depth {
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
    fn specialise(
        &mut self,
        id: FuncId,
        f: &FuncDef,
        args: &[Val],
        exprs: &[Expr],
        site: &Expr,
        body: Block,
    ) -> Option<Val> {
        let mut used_in_body = LocalSet::default();
        analysis::reads_block(&body, &mut used_in_body);
        let mut stmts: Vec<Stmt> = Vec::with_capacity(f.params.len() + body.stmts.len());
        for ((slot, v), e) in f.params.iter().zip(args).zip(exprs) {
            if !used_in_body.contains(slot.0) {
                continue;
            }
            // Only splice closed, zero-depth expressions into the callee frame.
            let val_expr = Self::closed_residual(v, e.span)?;
            stmts.push(Stmt::Let { local: *slot, value: val_expr });
        }
        stmts.extend(body.stmts);
        let mut body = Block { stmts, tail: body.tail, span: body.span };
        analysis::prune(&mut body);
        if body == f.body {
            return None;
        }
        let depth = analysis::ret_depth(&body);
        let latent = analysis::latent_of_block(&body, Held::NONE);

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
            let minted = FuncId(u32_index(base + self.specialised.len()));
            if let Some(k) = key {
                self.minted.entry(k).or_insert(minted);
            }
            let name = self.unique_func_name(&f.name, minted);
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
        Some(Val::rebuild(kind, site, depth))
    }

    /// A name for a minted function that collides with no function, global,
    /// local, struct, or prelude binding in the residue.
    fn unique_func_name(&self, from: &str, minted: FuncId) -> String {
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
    fn constants(args: &[Val]) -> Option<Vec<Literal>> {
        args.iter()
            .map(|v| match &v.kind {
                Kind::Known(l) => Some(l.to_literal()),
                _ => None,
            })
            .collect()
    }

    /// Unroll a loop as far as it will go, and abandon the unrolling whole if
    /// it stops going.
    fn eval_loop(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.grind.push(Grind { span: x.span, what: Open::Loop { turns: 0 } });
        let out = self.unroll(x, env)?;
        self.grind.pop();
        Ok(out)
    }

    fn unroll_flow(
        &mut self,
        entry_settling: usize,
        saved_starved: &mut Option<Block>,
    ) -> Option<Val> {
        match self.flow.take() {
            Some(Flow::Broke) => {
                self.strip_loop_exit_guards(entry_settling);
                self.starved = saved_starved.take();
                Some(Val::unit())
            }
            // A `return` out of a loop keeps going up.
            Some(Flow::Returned(mut ret)) => {
                self.strip_loop_exit_guards(entry_settling);
                self.starved = saved_starved.take();
                if let Kind::Stuck(ref mut expr) = ret.kind {
                    analysis::strip_expr_loop_guards(expr);
                }
                self.flow = Some(Flow::Returned(ret.clone()));
                Some(ret)
            }
            // A `continue` still runs the step.
            Some(Flow::Continued) | None => None,
        }
    }

    fn unroll(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let ExprKind::Loop { body, step } = &x.kind else {
            return Ok(Val::stuck(x));
        };
        let mut turn_before = env.clone();
        let mut turn_cp = self.checkpoint();
        let entry_settling = turn_cp.settling;
        loop {
            let v = self.expr(body, env)?;
            if let Some(out) = self.unroll_flow(entry_settling, &mut turn_cp.starved) {
                return Ok(out);
            }
            if v.is_stuck() {
                if self.acts_served_since(&turn_cp) {
                    let body_res = Self::residual(v, body);
                    if analysis::write_depths(&[&body_res]).1.loop_exit.is_none() {
                        let mut stmts = vec![Stmt::Expr(body_res)];
                        if let Some(s) = step {
                            stmts.push(Stmt::Expr((**s).clone()));
                        }
                        stmts.push(Stmt::Expr(x.clone()));
                        let blk = unit_block(stmts, x.span);
                        self.starved = turn_cp.starved;
                        return Ok(self.unreduced(&blk, env));
                    }
                }
                *env = turn_before;
                self.rollback(turn_cp);
                if analysis::reaches_act(x, self.unit) {
                    self.act_blocked = true;
                }
                return Ok(self.unreduced(x, env));
            }
            if let Some(s) = step {
                let sv = self.expr(s, env)?;
                if let Some(out) = self.unroll_flow(entry_settling, &mut turn_cp.starved) {
                    return Ok(out);
                }
                if sv.is_stuck() {
                    *env = turn_before;
                    self.rollback(turn_cp);
                    return Ok(self.unreduced(x, env));
                }
            }
            if self.acts_served_since(&turn_cp) {
                let saved_starved = turn_cp.starved;
                turn_before = env.clone();
                turn_cp = self.checkpoint();
                turn_cp.starved = saved_starved;
            }
            if let Some(Grind { what: Open::Loop { turns }, .. }) = self.grind.last_mut() {
                *turns += 1;
            }
        }
    }

    fn rite(&mut self, x: &Expr, rite: Rite, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        match rite {
            // `opaque` is never burned through, so its operand is not evaluated
            // at all, but stale local writes must be settled before it. §6.4.
            Rite::Opaque => Ok(self.unreduced(x, env)),
            Rite::Shade => self.rite_shade(x, operand, env),
            Rite::Seal | Rite::Look => {
                let v = self.expr(operand, env)?;
                if self.flow.is_some() {
                    return Ok(v);
                }
                if rite == Rite::Seal {
                    self.rite_seal(x, operand, v)
                } else {
                    Ok(Self::rite_look(x, operand, v))
                }
            }
        }
    }

    fn rite_shade(&mut self, x: &Expr, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
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
            let kind = ExprKind::Rite { rite: Rite::Shade, operand: Box::new(op_res) };
            let e = Expr { kind, ty: ty.clone(), depth: Depth::PURE, span: x.span };
            return Ok(Val { kind: Kind::Stuck(Box::new(e)), ty, depth: Depth::PURE });
        }
        self.starved = cp.starved;
        let ty = Type::Shade { origin: orig_origin, inner: Box::new(v.ty.clone()) };
        Ok(Val { kind: Kind::Shade(Box::new(v)), ty, depth: Depth::PURE })
    }

    fn rite_seal(&mut self, x: &Expr, operand: &Expr, v: Val) -> Result<Val, Halt> {
        // §1.5: `seal` on a shade names the shade; §7.1 puts its origin in its
        // encoding, so `seal shade e` is not `seal e`.
        if let Some(work) = Self::depositable_work(&v) {
            self.burn_bytes(work, x.span)?;
            if let Some(value) = fold::as_value(&v) {
                let cairn = if self.speculating {
                    Stored::Value(value).cairn()
                } else {
                    self.remember(Stored::Value(value))
                };
                return Ok(Val::known(Literal::Cairn(*cairn.as_bytes()), x));
            }
        }
        let operand = Box::new(Self::residual(v, operand));
        Ok(Val::rebuild(ExprKind::Rite { rite: Rite::Seal, operand }, x, Depth::PURE))
    }

    fn rite_look(x: &Expr, operand: &Expr, v: Val) -> Val {
        if let Kind::Shade(inner) = v.kind {
            return *inner;
        }
        let operand = Box::new(Self::residual(v, operand));
        let origin = match &operand.ty {
            Type::Shade { origin, .. } => *origin,
            _ => Depth::PURE,
        };
        let depth = origin.join(operand.depth);
        Val::rebuild(ExprKind::Rite { rite: Rite::Look, operand }, x, depth)
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
                        self.starved = self.starved.take().or(assign_cp.starved);
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
            self.starved = self.starved.take().or(assign_cp.starved);
            return Ok(v);
        }
        let effects_or_mut = self.had_effects(&assign_cp) || env.writes != assign_writes;
        if !place.path.is_empty() || v.is_stuck() {
            return Ok(self.assign_stuck_or_path(
                (x, place.local, value),
                v,
                (path, depth),
                (effects_or_mut, assign_cp, assign_env),
                env,
            ));
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
        Ok(Val::unit())
    }

    fn assign_stuck_or_path(
        &mut self,
        (x, local, value): (&Expr, LocalId, &Expr),
        v: Val,
        (path, mut depth): (Vec<nether_core::Proj>, Depth),
        (effects_or_mut, assign_cp, assign_env): (bool, Checkpoint, Option<Env>),
        env: &mut Env,
    ) -> Val {
        let has_path = !path.is_empty();
        if !v.is_stuck() && Self::closed_residual(&v, value.span).is_none() && effects_or_mut {
            self.rollback(assign_cp);
            if let Some(prev) = assign_env {
                *env = prev;
            }
            return self.unreduced(x, env);
        }
        if !v.is_stuck() {
            self.starved = assign_cp.starved;
        }
        let val_res = Self::residual(v, value);
        let (val_ty, val_depth) = (val_res.ty.clone(), val_res.depth);
        depth = depth.join(val_depth);
        let rebuilt = Expr {
            kind: ExprKind::Assign { place: Place { local, path }, value: Box::new(val_res) },
            ty: x.ty.clone(),
            depth,
            span: x.span,
        };
        if has_path {
            return self.unreduced(&rebuilt, env);
        }
        let prev_depth =
            env.get(local.0 as usize).and_then(Option::as_ref).map_or(Depth::PURE, |pv| pv.depth);
        let next_depth = prev_depth.join(val_depth).join(self.pc);
        env.bind(local.0 as usize, Some(Val::stuck_local(local, val_ty, next_depth, x.span)));
        Val::stuck(&rebuilt)
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
            self.settling.extend(analysis::settle(&[&r], env));
            r
        });
        self.settling.push(Stmt::Expr(assign_local(local, val_res, x.span)));
        env.bind(local.0 as usize, Some(v));
        Val::unit()
    }
}
