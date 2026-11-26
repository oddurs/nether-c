use nether_core::{Block, Depth, Expr, ExprKind, Literal, Stmt, Type, children};
use nether_ledger::Value;

use crate::residue::Halt;
use crate::state::{Burial, Checkpoint, Env, Flow, Kind, LocalMap, LocalSet, Val, ValLit};
use crate::{analysis, assign_local, fold};

/// Snapshot taken before an effectful statement that binds/assigns a non-closed
/// value (`Answer`/`Shade`/aggregate) so that if the enclosing block later
/// starves we can roll back its deposits and mutations without false starvation
/// when the block finishes cleanly.
struct DirtyRollback {
    index: usize,
    cp: Checkpoint,
    env: Env,
    kept_len: usize,
    eval_lets: LocalSet,
    let_originals: LocalMap<Expr>,
}

/// Block-local evaluation context threaded through block reduction helpers.
struct BlockCtx<'a> {
    b: &'a Block,
    saved_pc: Depth,
    entry_gen: u64,
    eval_lets: LocalSet,
    let_originals: LocalMap<Expr>,
    dirty: Option<DirtyRollback>,
    outer_dirty: Vec<u32>,
}

/// Liveness and depth-guard signals accumulated while pruning a residual block.
#[derive(Default)]
struct PruneInfo {
    read: LocalSet,
    shaded_reads: LocalSet,
    has_opaque: bool,
    has_assign: bool,
    min_ret: Option<Depth>,
}

impl PruneInfo {
    fn record(&mut self, value: &Expr) {
        analysis::reads(value, &mut self.read);
        analysis::shade_reads(value, &mut self.shaded_reads);
        self.has_opaque |= analysis::has_opaque(value);
        self.has_assign |= analysis::has_assign(value);
        if let Some(rd) = analysis::min_ret_depth(value) {
            self.min_ret = Some(self.min_ret.map_or(rd, |m| m.min(rd)));
        }
    }
}

impl Burial<'_> {
    /// §6.5: a block reduces its statements in order. Once a statement starves,
    /// the statements and tail after it are reduced speculatively in place,
    /// with any starved binding kept as an unknown local.
    ///
    /// The residue goes into `Burial::starved` rather than coming back as a
    /// value, because a block's value is what starved in it and its residue is
    /// the statements around that.
    pub(crate) fn block(&mut self, b: &Block, env: &mut Env) -> Result<Val, Halt> {
        let mut ctx = BlockCtx {
            b,
            saved_pc: self.pc,
            entry_gen: env.writes,
            eval_lets: LocalSet::default(),
            let_originals: LocalMap::default(),
            dirty: None,
            outer_dirty: std::mem::take(&mut self.nonclosed_dirty),
        };
        let mut kept: Vec<Stmt> = Vec::new();
        for (i, stmt) in b.stmts.iter().enumerate() {
            let cand =
                (ctx.dirty.is_none() && Self::may_be_nonclosed(stmt)).then(|| DirtyRollback {
                    index: i,
                    cp: self.checkpoint(),
                    env: env.clone(),
                    kept_len: kept.len(),
                    eval_lets: ctx.eval_lets.clone(),
                    let_originals: ctx.let_originals.clone(),
                });
            let v = self.eval_block_stmt(stmt, &mut kept, &mut ctx, env)?;
            if let Some(c) = cand {
                if !self.nonclosed_dirty.is_empty() {
                    ctx.dirty = Some(c);
                } else if self.starved.is_none() {
                    self.starved = c.cp.starved;
                }
            }
            if v.is_stuck() || self.flow.is_some() {
                if let Some(Flow::Returned(ret)) = &self.flow
                    && self.is_stuck_return(ret, b.span)
                {
                    let ret = ret.clone();
                    return Ok(self.finish_returned(&ret, stmt, i, kept, ctx, env));
                }
                if v.is_stuck() && self.flow.is_none() {
                    return self.block_starved_stmt(ctx, i, &v, kept, env);
                }
                self.pc = ctx.saved_pc;
                self.merge_dirty(ctx.outer_dirty);
                self.bubble_outer_guards(b, &kept);
                return Ok(v);
            }
        }
        self.block_tail(ctx, kept, env)
    }

    fn block_tail(
        &mut self,
        mut ctx: BlockCtx<'_>,
        mut kept: Vec<Stmt>,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let out = match &ctx.b.tail {
            Some(tail) => {
                let mark = self.settling.len();
                let v = self.expr(tail, env)?;
                self.drain_settling(mark, &mut kept);
                if let Some(Flow::Returned(ret)) = &self.flow
                    && self.is_stuck_return(ret, tail.span)
                {
                    let ret = ret.clone();
                    let stmt = Stmt::Expr((**tail).clone());
                    let from = ctx.b.stmts.len();
                    return Ok(self.finish_returned(&ret, &stmt, from, kept, ctx, env));
                }
                if v.is_stuck() && self.flow.is_none() {
                    let from = ctx.b.stmts.len();
                    let (bound, dirty) = self.finish_dirty(&mut ctx, from);
                    self.pc = ctx.saved_pc;
                    if let Some(dirty) = dirty {
                        return Ok(self.rollback_dirty(dirty, kept, &mut ctx, &v, env));
                    }
                    analysis::flush_stale_since(Some(&bound), ctx.entry_gen, env, &mut kept);
                    let tail = Some(Box::new(Self::residual(v.clone(), tail)));
                    return Ok(self.residue_pruned(kept, tail, &ctx, &v));
                }
                v
            }
            None => Val::unit(),
        };
        self.pc = ctx.saved_pc;
        self.merge_dirty(ctx.outer_dirty);
        self.bubble_outer_guards(ctx.b, &kept);
        Ok(out)
    }

    fn eval_block_stmt(
        &mut self,
        s: &Stmt,
        kept: &mut Vec<Stmt>,
        ctx: &mut BlockCtx<'_>,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let mark = self.settling.len();
        match s {
            Stmt::Let { local, value } => {
                let deposits_before = self.deposits.len();
                let acts_before = self.acts_served;
                let writes_before = env.writes;
                let v = self.expr(value, env)?;
                self.drain_settling(mark, kept);
                if self.flow.is_some() {
                    return Ok(v);
                }
                let effects_or_mut = self.deposits.len() > deposits_before
                    || self.acts_served != acts_before
                    || env.writes != writes_before;
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
                        ctx.let_originals.insert(local.0, value.clone());
                    }
                    return Ok(v);
                }
                ctx.eval_lets.insert(local.0);
                let res = Self::residual_settled(v, value, kept, env);
                if value.depth > res.depth {
                    ctx.let_originals.insert(local.0, value.clone());
                }
                kept.push(Stmt::Let { local: *local, value: res });
                Ok(Val::unit())
            }
            Stmt::Declare { local } => {
                env.bind(local.0 as usize, None);
                kept.push(Stmt::Declare { local: *local });
                Ok(Val::unit())
            }
            Stmt::Expr(x) => {
                let v = self.expr(x, env)?;
                self.drain_settling(mark, kept);
                if self.flow.is_some() || matches!(v.kind, Kind::Known(ValLit::Unit)) {
                    return Ok(v);
                }
                let Some(work) = Self::depositable_work(&v) else {
                    return Ok(v);
                };
                if self.speculating {
                    let e = Box::new(Self::residual(v.clone(), x));
                    return Ok(Val { kind: Kind::Stuck(e), ty: v.ty, depth: v.depth });
                }
                self.burn_bytes(work, x.span)?;
                if let Some(value) = fold::as_value(&v) {
                    let at = self.deposit(value, x.span);
                    self.deposits.push(at);
                }
                Ok(v)
            }
        }
    }

    fn finish_dirty(
        &mut self,
        ctx: &mut BlockCtx<'_>,
        from: usize,
    ) -> (LocalSet, Option<DirtyRollback>) {
        let cur_dirty =
            std::mem::replace(&mut self.nonclosed_dirty, std::mem::take(&mut ctx.outer_dirty));
        let bound = analysis::block_bound(ctx.b);
        let dirty = ctx.dirty.take().filter(|_| {
            Self::dirty_locals_live(&cur_dirty, &bound, &ctx.b.stmts[from..], ctx.b.tail.as_deref())
        });
        (bound, dirty)
    }

    fn block_starved_stmt(
        &mut self,
        mut ctx: BlockCtx<'_>,
        i: usize,
        v: &Val,
        mut kept: Vec<Stmt>,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let (bound, dirty) = self.finish_dirty(&mut ctx, i);
        if let Some(dirty) = dirty {
            self.pc = ctx.saved_pc;
            return Ok(self.rollback_dirty(dirty, kept, &mut ctx, v, env));
        }
        analysis::flush_stale_since(Some(&bound), ctx.entry_gen, env, &mut kept);
        let halted = Self::halted(v.clone(), &ctx.b.stmts[i]);
        if let Stmt::Let { value: ex, .. } | Stmt::Expr(ex) = &halted {
            self.advance_pc_from_expr(ex);
        }
        kept.push(halted);
        let res = self.after_starved(&ctx, i + 1, kept, v, &bound, env);
        self.pc = ctx.saved_pc;
        if res.is_ok() {
            self.mark_act_blocked_if_reaches(ctx.b, i + 1);
        }
        res
    }

    /// Reduce the statements and tail after a starved statement in a block.
    fn after_starved(
        &mut self,
        ctx: &BlockCtx<'_>,
        from: usize,
        mut kept: Vec<Stmt>,
        starved: &Val,
        bound: &LocalSet,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let b = ctx.b;
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
                self.leave_remaining_block(b, i, &mut kept, env);
                return Ok(self.residue_pruned(kept, b.tail.clone(), ctx, starved));
            };
            if let Some(Flow::Returned(r)) = self.flow.take() {
                self.speculating = was;
                return Ok(self.returned_residue(&r, s, kept, ctx, env));
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
                        return Ok(self.returned_residue(&r, &s, kept, ctx, env));
                    }
                    let tail_expr = Self::residual_settled(v.clone(), t, &mut kept, env);
                    analysis::flush_stale_since(Some(bound), tail_gen, env, &mut kept);
                    let tail = Some(Box::new(tail_expr));
                    return Ok(self.residue_pruned(kept, tail, ctx, &v));
                }
                kept.extend(analysis::leave(&[&**t], self.pc, env));
                b.tail.clone()
            }
            None => None,
        };
        self.speculating = was;
        Ok(self.residue_pruned(kept, tail, ctx, starved))
    }

    fn keep_starved_stmt(
        &mut self,
        s: &Stmt,
        v: Val,
        stmt_gen: u64,
        bound: &LocalSet,
        kept: &mut Vec<Stmt>,
        env: &mut Env,
    ) {
        match s {
            Stmt::Let { local, value } => {
                env.bind(local.0 as usize, Some(v.clone()));
                let res = Self::residual_settled(v, value, kept, env);
                analysis::flush_stale_since(Some(bound), stmt_gen, env, kept);
                self.advance_pc_from_expr(&res);
                kept.push(Stmt::Let { local: *local, value: res });
            }
            Stmt::Expr(x)
                if v.is_stuck() || fold::as_value(&v).is_some_and(|val| val != Value::Unit) =>
            {
                let res = Self::residual_settled(v, x, kept, env);
                analysis::flush_stale_since(Some(bound), stmt_gen, env, kept);
                self.advance_pc_from_expr(&res);
                kept.push(Stmt::Expr(res));
            }
            Stmt::Expr(_) | Stmt::Declare { .. } => {
                analysis::flush_stale_since(Some(bound), stmt_gen, env, kept);
            }
        }
    }

    fn finish_returned(
        &mut self,
        ret: &Val,
        stmt: &Stmt,
        from: usize,
        kept: Vec<Stmt>,
        mut ctx: BlockCtx<'_>,
        env: &mut Env,
    ) -> Val {
        self.pc = ctx.saved_pc;
        let (_, dirty) = self.finish_dirty(&mut ctx, from);
        if let Some(dirty) = dirty {
            return self.rollback_dirty(dirty, kept, &mut ctx, ret, env);
        }
        self.returned_residue(ret, stmt, kept, &ctx, env)
    }

    /// Build a residual block ending in `return r`, settling any stale locals
    /// referenced by the fallback return expression and updating `self.flow`.
    fn returned_residue(
        &mut self,
        r: &Val,
        s: &Stmt,
        mut kept: Vec<Stmt>,
        ctx: &BlockCtx<'_>,
        env: &mut Env,
    ) -> Val {
        let ret_stmt = Self::returning(r.clone(), s);
        if let Stmt::Expr(ref rx) = ret_stmt
            && Self::closed_residual(r, rx.span).is_none()
        {
            kept.extend(analysis::settle(&[rx], env));
        }
        kept.push(ret_stmt);
        let out = self.residue_pruned(kept, None, ctx, r);
        self.flow = Some(Flow::Returned(out.clone()));
        out
    }

    fn leave_remaining_block(&self, b: &Block, from: usize, kept: &mut Vec<Stmt>, env: &mut Env) {
        let rem = Block { stmts: b.stmts[from..].to_vec(), tail: b.tail.clone(), span: b.span };
        let rem_expr =
            Expr { kind: ExprKind::Block(rem), ty: Type::Unit, depth: Depth::PURE, span: b.span };
        kept.extend(analysis::leave(&[&rem_expr], self.pc, env));
        kept.extend(b.stmts[from..].iter().cloned());
    }

    fn rollback_dirty(
        &mut self,
        dirty: DirtyRollback,
        mut kept: Vec<Stmt>,
        ctx: &mut BlockCtx<'_>,
        starved: &Val,
        env: &mut Env,
    ) -> Val {
        self.rollback(dirty.cp);
        *env = dirty.env;
        kept.truncate(dirty.kept_len);
        self.leave_remaining_block(ctx.b, dirty.index, &mut kept, env);
        ctx.eval_lets = dirty.eval_lets;
        ctx.let_originals = dirty.let_originals;
        self.residue_pruned(kept, ctx.b.tail.clone(), ctx, starved)
    }

    /// Build a residual block, dropping already-evaluated `Stmt::Let` bindings
    /// that no surviving statement reads and preserving original depth via a
    /// non-executing `if (false)` guard when a surviving `shade` depends on a
    /// folded binding's depth.
    fn residue_pruned(
        &mut self,
        stmts: Vec<Stmt>,
        mut tail: Option<Box<Expr>>,
        ctx: &BlockCtx<'_>,
        v: &Val,
    ) -> Val {
        let mut info = PruneInfo::default();
        if let Some(t) = &tail {
            info.record(t);
        }
        let mut kept: Vec<Stmt> = Vec::with_capacity(stmts.len());
        for s in stmts.into_iter().rev() {
            if let Stmt::Let { local, value } = &s
                && ctx.eval_lets.contains(local.0)
                && !info.read.contains(local.0)
                && !matches!(value.kind, ExprKind::Literal(_))
            {
                continue;
            }
            if let Stmt::Expr(e) = &s
                && analysis::is_dead_branch_guard(e)
            {
                let (writes, ex) = analysis::write_depths(&[e]);
                let writes_used = writes.iter().any(|(id, _)| info.read.contains(id));
                let exit_used = ex.any().is_some_and(|ed| {
                    info.has_opaque
                        || info.has_assign
                        || !info.shaded_reads.is_empty()
                        || info.min_ret.is_some_and(|rd| ed > rd)
                });
                if !writes_used && !exit_used {
                    continue;
                }
            }
            if let Stmt::Let { local, value } = &s
                && info.shaded_reads.contains(local.0)
                && let Some(orig) = ctx.let_originals.get(local.0)
                && orig.depth > value.depth
            {
                let assign_orig = assign_local(*local, orig.clone(), orig.span);
                let guard = analysis::dead_branch_guard(false, &assign_orig, orig.span);
                analysis::reads(&guard, &mut info.read);
                analysis::reads(&guard, &mut info.shaded_reads);
                kept.push(Stmt::Expr(guard));
            }
            match &s {
                Stmt::Let { local, value } => {
                    info.record(value);
                    if info.shaded_reads.contains(local.0) {
                        analysis::reads(value, &mut info.shaded_reads);
                    }
                }
                Stmt::Expr(value) => info.record(value),
                Stmt::Declare { .. } => {}
            }
            kept.push(s);
        }
        kept.reverse();
        if !ctx.let_originals.is_empty() {
            let mut depths = LocalMap::default();
            let mut steps = 0usize;
            analysis::refresh_stmts(&mut kept, &mut tail, ctx.saved_pc, &mut steps, &mut depths);
        }
        self.residue(kept, tail, ctx.b, v)
    }

    /// What a block that did not finish residualises as.
    fn residue(&mut self, stmts: Vec<Stmt>, tail: Option<Box<Expr>>, b: &Block, v: &Val) -> Val {
        let (ty, depth) = match &tail {
            Some(t) => (t.ty.clone(), t.depth),
            None => (Type::Unit, Depth::PURE),
        };
        let block = Block { stmts, tail, span: b.span };
        self.starved = Some(block.clone());
        let e = Expr { kind: ExprKind::Block(block), ty, depth, span: b.span };
        Val { kind: Kind::Stuck(Box::new(e)), ty: v.ty.clone(), depth: v.depth }
    }

    /// Preserve dead-branch depth guards and outer-local non-literal assignments
    /// emitted inside a finished inner block when they target locals not bound
    /// in `b`. Deduplicated so loops stay O(1).
    fn bubble_outer_guards(&mut self, b: &Block, kept: &[Stmt]) {
        let bound = analysis::block_bound(b);
        for s in kept {
            let Stmt::Expr(e) = s else { continue };
            let mut e_reads = LocalSet::default();
            analysis::reads(e, &mut e_reads);
            if e_reads.any(|id| bound.contains(id)) {
                continue;
            }
            match &e.kind {
                ExprKind::Select { cond, .. }
                    if e.ty == Type::Unit
                        && matches!(cond.kind, ExprKind::Literal(Literal::Bool(_))) =>
                {
                    let (writes, ex) = analysis::write_depths(&[e]);
                    if (ex.any().is_some() || writes.iter().any(|(id, _)| !bound.contains(id)))
                        && !self.settling.contains(s)
                    {
                        self.settling.push(s.clone());
                    }
                }
                ExprKind::Assign { place, .. }
                    if place.path.is_empty() && !bound.contains(place.local.0) =>
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

    /// Residualise `v`, settling stale locals into `kept` when `closed_residual`
    /// falls back to `original`.
    fn residual_settled(v: Val, original: &Expr, kept: &mut Vec<Stmt>, env: &mut Env) -> Expr {
        Self::closed_residual(&v, original.span).unwrap_or_else(|| {
            let r = Self::residual(v, original);
            kept.extend(analysis::settle(&[&r], env));
            r
        })
    }

    fn drain_settling(&mut self, mark: usize, kept: &mut Vec<Stmt>) {
        for s in self.settling.drain(mark..) {
            if let Stmt::Expr(e) = &s {
                self.pc = analysis::write_depths(&[e]).1.advance(self.pc);
            }
            kept.push(s);
        }
    }

    pub(crate) fn strip_loop_exit_guards(&mut self, from: usize) {
        let mut write = from;
        for read in from..self.settling.len() {
            let drop_it = matches!(
                &self.settling[read],
                Stmt::Expr(e) if analysis::write_depths(&[e]).1.loop_exit.is_some()
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

    fn mark_act_blocked_if_reaches(&mut self, b: &Block, from: usize) {
        if b.stmts[from..]
            .iter()
            .filter_map(analysis::stmt_expr)
            .chain(b.tail.as_deref())
            .any(|e| analysis::reaches_act(e, self.unit))
        {
            self.act_blocked = true;
        }
    }

    fn advance_pc_from_expr(&mut self, e: &Expr) {
        self.pc = analysis::write_depths(&[e]).1.advance(self.pc);
    }

    fn dirty_locals_live(
        dirty_locals: &[u32],
        bound: &LocalSet,
        remaining_stmts: &[Stmt],
        tail: Option<&Expr>,
    ) -> bool {
        if dirty_locals.is_empty() {
            return false;
        }
        let mut live = LocalSet::default();
        for e in remaining_stmts.iter().filter_map(analysis::stmt_expr).chain(tail) {
            analysis::reads(e, &mut live);
        }
        dirty_locals.iter().any(|&id| !bound.contains(id) || live.contains(id))
    }

    fn merge_dirty(&mut self, mut outer: Vec<u32>) {
        for id in self.nonclosed_dirty.drain(..) {
            if !outer.contains(&id) {
                outer.push(id);
            }
        }
        self.nonclosed_dirty = outer;
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
        let inner = match inner.kind {
            ExprKind::Return(Some(e)) => *e,
            ExprKind::Block(_) | ExprKind::Descend { .. } if analysis::ends_returning(&inner) => {
                return Stmt::Expr(inner);
            }
            _ => inner,
        };
        let depth = inner.depth;
        let kind = ExprKind::Return(Some(Box::new(inner)));
        Stmt::Expr(Expr { kind, ty: Type::Unit, depth, span: x.span })
    }
}
