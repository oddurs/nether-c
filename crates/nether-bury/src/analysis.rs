use nether_core::{
    Block, Depth, Expr, ExprKind, Held, Literal, LocalId, Rite, Span, Stmt, Type, Unit, children,
};

use crate::state::{Burial, Env, LocalMap, LocalSet, Val};
use crate::{assign_local, u32_index, unit_block};

/// Maximum total loop fixpoint iterations across `walk_writes` and
/// `refresh_loop` so nested loops cannot blow up exponentially.
const MAX_FIXPOINT_STEPS: usize = 4096;

/// Control-dependence exit depths observed while walking an expression or block.
#[derive(Clone, Copy, Default)]
pub(crate) struct ExitPc {
    pub(crate) loop_exit: Option<Depth>,
    pub(crate) fn_exit: Option<Depth>,
}

impl ExitPc {
    const fn join(self, other: Self) -> Self {
        Self {
            loop_exit: join_opt_depth(self.loop_exit, other.loop_exit),
            fn_exit: join_opt_depth(self.fn_exit, other.fn_exit),
        }
    }

    pub(crate) const fn any(self) -> Option<Depth> {
        join_opt_depth(self.loop_exit, self.fn_exit)
    }

    pub(crate) const fn advance(self, pc: Depth) -> Depth {
        match self.any() {
            Some(ed) => pc.join(ed),
            None => pc,
        }
    }
}

const fn join_opt_depth(a: Option<Depth>, b: Option<Depth>) -> Option<Depth> {
    match (a, b) {
        (Some(x), Some(y)) => Some(x.join(y)),
        (Some(x), None) | (None, Some(x)) => Some(x),
        (None, None) => None,
    }
}

pub(crate) fn stmt_expr(s: &Stmt) -> Option<&Expr> {
    match s {
        Stmt::Let { value, .. } | Stmt::Expr(value) => Some(value),
        Stmt::Declare { .. } => None,
    }
}

fn block_exprs(b: &Block) -> impl Iterator<Item = &Expr> {
    b.stmts.iter().filter_map(stmt_expr).chain(b.tail.as_deref())
}

fn visit_exprs(x: &Expr, mut f: impl FnMut(&Expr)) {
    let mut todo = vec![x];
    while let Some(cur) = todo.pop() {
        f(cur);
        todo.extend(children(cur));
    }
}

/// Every local an expression names, including inside `Assign` index paths.
pub(crate) fn reads(x: &Expr, into: &mut LocalSet) {
    visit_exprs(x, |cur| match &cur.kind {
        ExprKind::Local(id) => {
            into.insert(id.0);
        }
        ExprKind::Assign { place, .. } => {
            into.insert(place.local.0);
        }
        _ => {}
    });
}

/// Every local read anywhere inside `b`.
pub(crate) fn reads_block(b: &Block, into: &mut LocalSet) {
    for e in block_exprs(b) {
        reads(e, into);
    }
}

/// Every local a `let` or a declaration anywhere in `x` binds.
pub(crate) fn binds(x: &Expr, into: &mut LocalSet) {
    visit_exprs(x, |cur| {
        if let ExprKind::Block(b) = &cur.kind {
            for s in &b.stmts {
                if let Stmt::Let { local, .. } | Stmt::Declare { local } = s {
                    into.insert(local.0);
                }
            }
        }
    });
}

pub(crate) fn block_bound(b: &Block) -> LocalSet {
    let mut out = LocalSet::default();
    for s in &b.stmts {
        if let Stmt::Let { local, .. } | Stmt::Declare { local } = s {
            out.insert(local.0);
        }
    }
    out
}

/// Capabilities a block reaches outside any enclosing `descend`, matching `check.rs`.
pub(crate) fn latent_of_block(b: &Block, ambient: Held) -> Held {
    block_exprs(b).fold(Held::NONE, |needs, e| needs.union(latent_of_expr(e, ambient)))
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

/// Drop unused literal `let` bindings from a specialised body.
pub(crate) fn prune(body: &mut Block) {
    let mut read = LocalSet::default();
    if let Some(t) = &body.tail {
        reads(t, &mut read);
    }
    let mut kept: Vec<Stmt> = Vec::new();
    for s in body.stmts.iter().rev() {
        if let Stmt::Let { local, value } = s
            && !read.contains(local.0)
            && matches!(value.kind, ExprKind::Literal(_))
        {
            continue;
        }
        if let Some(e) = stmt_expr(s) {
            reads(e, &mut read);
        }
        kept.push(s.clone());
    }
    kept.reverse();
    body.stmts = kept;
}

/// Build a unit-typed dead-branch guard `if (taken) {} else { <other>; }`
/// (or vice versa) that preserves `other`'s outer local write depths and exit
/// `pc` in the checker without running `other` or depositing a value.
pub(crate) fn dead_branch_guard(taken: bool, other: &Expr, span: Span) -> Expr {
    let empty = unit_block(Vec::new(), span);
    let dead = if other.ty == Type::Unit {
        other.clone()
    } else if let ExprKind::Block(b) = &other.kind {
        let mut stmts = b.stmts.clone();
        if let Some(t) = &b.tail
            && !matches!(t.kind, ExprKind::Local(_) | ExprKind::Literal(_))
        {
            stmts.push(Stmt::Expr((**t).clone()));
        }
        unit_block(stmts, other.span)
    } else {
        unit_block(vec![Stmt::Expr(other.clone())], other.span)
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

/// Whether `e` is a unit-typed dead-branch guard `if (true) {} else { .. }`
/// or `if (false) { .. } else {}`.
pub(crate) fn is_dead_branch_guard(e: &Expr) -> bool {
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

pub(crate) fn strip_expr_loop_guards(x: &mut Expr) {
    match &mut x.kind {
        ExprKind::Block(b) => {
            b.stmts.retain(|s| {
                !matches!(
                    s,
                    Stmt::Expr(e)
                        if is_dead_branch_guard(e) && write_depths(&[e]).1.loop_exit.is_some()
                )
            });
            for s in &mut b.stmts {
                match s {
                    Stmt::Let { value, .. } | Stmt::Expr(value) => strip_expr_loop_guards(value),
                    Stmt::Declare { .. } => {}
                }
            }
            if let Some(t) = b.tail.as_deref_mut() {
                strip_expr_loop_guards(t);
            }
        }
        ExprKind::Descend { body, .. }
        | ExprKind::Rite { operand: body, .. }
        | ExprKind::Unary { operand: body, .. }
        | ExprKind::Field { base: body, .. } => strip_expr_loop_guards(body),
        ExprKind::Select { cond, then, otherwise } => {
            strip_expr_loop_guards(cond);
            strip_expr_loop_guards(then);
            strip_expr_loop_guards(otherwise);
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
            strip_expr_loop_guards(lhs);
            strip_expr_loop_guards(rhs);
        }
        ExprKind::Return(Some(v)) => strip_expr_loop_guards(v),
        _ => {}
    }
}

/// Whether dropping `other` for `arm` lowers a depth under ambient `pc`. §6.5.
pub(crate) fn lowers(arm: &Expr, other: &Expr, pc: Depth) -> (bool, bool) {
    let (taken, taken_ex) = write_depths(&[arm]);
    let (dropped, dropped_ex) = write_depths(&[other]);
    let exit_higher = |d: Option<Depth>, t: Option<Depth>| {
        d.is_some_and(|dd| dd.join(pc) > t.map_or(Depth::PURE, |td| td.join(pc)))
    };
    let effects = exit_higher(dropped_ex.loop_exit, taken_ex.loop_exit)
        || exit_higher(dropped_ex.fn_exit, taken_ex.fn_exit)
        || dropped.iter().any(|(id, &dd)| {
            let eff = dd.join(pc);
            taken.get(id).map_or(eff > Depth::PURE, |&td| eff > td.join(pc))
        });
    (other.depth > arm.depth, effects)
}

/// Deepest write depth (including control-dependence `pc`) for every local
/// `code` assigns and does not bind itself, plus exit `pc`.
pub(crate) fn write_depths(code: &[&Expr]) -> (LocalMap<Depth>, ExitPc) {
    let (mut written, mut bound) = (LocalMap::default(), LocalSet::default());
    let mut exits = ExitPc::default();
    let mut steps = 0usize;
    for expr in code {
        exits = exits.join(walk_writes(expr, Depth::PURE, 0, &mut steps, &mut written, &mut bound));
    }
    for id in bound.iter() {
        written.remove(id);
    }
    (written, exits)
}

fn walk_writes_seq(
    items: Vec<&Expr>,
    mut cur: Depth,
    next_d: u32,
    steps: &mut usize,
    written: &mut LocalMap<Depth>,
    bound: &mut LocalSet,
) -> ExitPc {
    let mut exits = ExitPc::default();
    for child in items {
        let ex = walk_writes(child, cur, next_d, steps, written, bound);
        cur = ex.advance(cur);
        exits = exits.join(ex);
    }
    exits
}

fn walk_writes_block(
    blk: &Block,
    pc: Depth,
    next_d: u32,
    steps: &mut usize,
    written: &mut LocalMap<Depth>,
    bound: &mut LocalSet,
) -> ExitPc {
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
    walk_writes_seq(exprs, pc, next_d, steps, written, bound)
}

fn walk_writes(
    expr: &Expr,
    pc: Depth,
    depth: u32,
    steps: &mut usize,
    written: &mut LocalMap<Depth>,
    bound: &mut LocalSet,
) -> ExitPc {
    const MAX_WALK: u32 = if cfg!(target_family = "wasm") { 1024 } else { 2048 };
    if depth >= MAX_WALK || *steps >= MAX_FIXPOINT_STEPS {
        return ExitPc { loop_exit: Some(Depth::UNRECORDED), fn_exit: Some(Depth::UNRECORDED) };
    }
    let next_d = depth + 1;
    match &expr.kind {
        ExprKind::Select { cond, then, otherwise } => {
            let e0 = walk_writes(cond, pc, next_d, steps, written, bound);
            let bpc = e0.advance(pc).join(cond.depth);
            let e1 = walk_writes(then, bpc, next_d, steps, written, bound);
            let e2 = walk_writes(otherwise, bpc, next_d, steps, written, bound);
            e0.join(e1).join(e2)
        }
        ExprKind::Block(blk) => walk_writes_block(blk, pc, next_d, steps, written, bound),
        ExprKind::Loop { body, step } => {
            let mut lpc = pc;
            loop {
                *steps += 1;
                if *steps > MAX_FIXPOINT_STEPS {
                    return ExitPc { loop_exit: None, fn_exit: Some(Depth::UNRECORDED) };
                }
                let e1 = walk_writes(body, lpc, next_d, steps, written, bound);
                let spc = e1.advance(lpc);
                let e2 = step.as_deref().map_or(ExitPc::default(), |s| {
                    walk_writes(s, spc, next_d, steps, written, bound)
                });
                let combined = e1.join(e2);
                let next = combined.advance(lpc);
                if next == lpc {
                    return ExitPc { loop_exit: None, fn_exit: combined.fn_exit };
                }
                lpc = next;
            }
        }
        ExprKind::Break | ExprKind::Continue => ExitPc { loop_exit: Some(pc), fn_exit: None },
        ExprKind::Return(val) => {
            let ex = val.as_deref().map_or(ExitPc::default(), |inner| {
                walk_writes(inner, pc, next_d, steps, written, bound)
            });
            ExitPc { loop_exit: ex.loop_exit, fn_exit: Some(ex.advance(pc)) }
        }
        ExprKind::Assign { place, .. } => {
            let exits = walk_writes_seq(children(expr), pc, next_d, steps, written, bound);
            written.join(place.local.0, expr.depth.join(exits.advance(pc)));
            exits
        }
        _ => walk_writes_seq(children(expr), pc, next_d, steps, written, bound),
    }
}

fn local_assign(id: u32, v: Val) -> Stmt {
    let at = Expr {
        kind: ExprKind::Local(LocalId(id)),
        ty: v.ty.clone(),
        depth: v.depth,
        span: Span::default(),
    };
    Stmt::Expr(assign_local(LocalId(id), Burial::residual(v, &at), Span::default()))
}

pub(crate) fn flush_stale_since(
    exclude: Option<&LocalSet>,
    since_gen: u64,
    env: &mut Env,
    out: &mut Vec<Stmt>,
) {
    if env.writes == since_gen {
        return;
    }
    for idx in 0..env.slots.len() {
        let id = u32_index(idx);
        if exclude.is_some_and(|ex| ex.contains(id))
            || !env.stale[idx]
            || env.write_gen[idx] <= since_gen
        {
            continue;
        }
        let Some(v) = env.slots[idx].clone() else { continue };
        if !v.is_settleable() {
            continue;
        }
        env.stale[idx] = false;
        out.push(local_assign(id, v));
    }
}

/// What code left as written needs before it in the residue.
pub(crate) fn settle(code: &[&Expr], env: &mut Env) -> Vec<Stmt> {
    let (mut read, mut bound) = (LocalSet::default(), LocalSet::default());
    for x in code {
        reads(x, &mut read);
        binds(x, &mut bound);
    }
    let mut out = Vec::new();
    for idx in 0..env.slots.len() {
        let id = u32_index(idx);
        if !env.stale[idx] || !read.contains(id) || bound.contains(id) {
            continue;
        }
        let Some(v) = env.slots[idx].clone() else { continue };
        if !v.is_settleable() {
            continue;
        }
        env.bind(idx, Some(v.clone()));
        out.push(local_assign(id, v));
    }
    out
}

/// Settle stale locals read by `code`, then clear locals `code` writes so
/// subsequent reads refer to the local itself.
pub(crate) fn leave(code: &[&Expr], depth: Depth, env: &mut Env) -> Vec<Stmt> {
    let settled = settle(code, env);
    let (writes, _) = write_depths(code);
    for (local, &written) in writes.iter() {
        let id = local as usize;
        if let Some(Some(v)) = env.get(id) {
            let (ty, depth) = (v.ty.clone(), v.depth.join(depth).join(written));
            env.bind(id, Some(Val::stuck_local(LocalId(local), ty, depth, Span::default())));
        }
    }
    settled
}

/// [ABS]: the return depth of a reduced body is the join of its tail, every
/// `return` value, and the control-dependence `pc` at every `return`.
pub(crate) fn ret_depth(b: &Block) -> Depth {
    let (mut written, mut bound) = (LocalMap::default(), LocalSet::default());
    let exits = walk_writes_block(b, Depth::PURE, 0, &mut 0, &mut written, &mut bound);
    let tail = b.tail.as_ref().map_or(Depth::PURE, |t| t.depth);
    let returns_depth = block_exprs(b).fold(Depth::PURE, |d, e| d.join(returns(e)));
    tail.join(returns_depth).join(exits.fn_exit.unwrap_or(Depth::PURE))
}

fn returns(x: &Expr) -> Depth {
    let mut acc = Depth::PURE;
    visit_exprs(x, |cur| {
        if let ExprKind::Return(Some(v)) = &cur.kind {
            acc = acc.join(v.depth);
        }
    });
    acc
}

/// Recompute stated `Expr.depth` inside a residual block when an earlier
/// `Stmt::Let` folded to a shallower depth, so unreduced forms stay consistent
/// with `nether_core::check`.
pub(crate) fn refresh_stmts(
    stmts: &mut [Stmt],
    tail: &mut Option<Box<Expr>>,
    pc: Depth,
    steps: &mut usize,
    depths: &mut LocalMap<Depth>,
) -> (Depth, ExitPc) {
    let mut cur_pc = pc;
    let mut exit = ExitPc::default();
    for s in stmts {
        match s {
            Stmt::Let { local, value } => {
                let (d, ex) = refresh_expr(value, cur_pc, steps, depths);
                depths.insert(local.0, d);
                exit = exit.join(ex);
                cur_pc = ex.advance(cur_pc);
            }
            Stmt::Declare { local } => {
                depths.insert(local.0, Depth::PURE);
            }
            Stmt::Expr(x) => {
                let (_, ex) = refresh_expr(x, cur_pc, steps, depths);
                exit = exit.join(ex);
                cur_pc = ex.advance(cur_pc);
            }
        }
    }
    let (td, tex) = tail
        .as_mut()
        .map_or((Depth::PURE, ExitPc::default()), |t| refresh_expr(t, cur_pc, steps, depths));
    (td, exit.join(tex))
}

fn refresh_expr(
    x: &mut Expr,
    pc: Depth,
    steps: &mut usize,
    depths: &mut LocalMap<Depth>,
) -> (Depth, ExitPc) {
    let (d, ex) = match &mut x.kind {
        ExprKind::Literal(_) | ExprKind::Func(_) | ExprKind::Prim(_) => {
            (Depth::PURE, ExitPc::default())
        }
        ExprKind::Break | ExprKind::Continue => {
            (Depth::PURE, ExitPc { loop_exit: Some(pc), fn_exit: None })
        }
        ExprKind::Local(id) => (depths.get(id.0).copied().unwrap_or(x.depth), ExitPc::default()),
        ExprKind::Global(_) => (x.depth, ExitPc::default()),
        ExprKind::Unary { operand, .. } | ExprKind::Field { base: operand, .. } => {
            refresh_expr(operand, pc, steps, depths)
        }
        ExprKind::Binary { lhs, rhs, .. } | ExprKind::Index { base: lhs, index: rhs } => {
            let (dl, el) = refresh_expr(lhs, pc, steps, depths);
            let (dr, er) = refresh_expr(rhs, el.advance(pc), steps, depths);
            (dl.join(dr), el.join(er))
        }
        ExprKind::Call { callee, args } => {
            let (df, mut ex) = refresh_expr(callee, pc, steps, depths);
            let mut cur_pc = ex.advance(pc);
            let mut da = Depth::PURE;
            for a in args {
                let (ad, aex) = refresh_expr(a, cur_pc, steps, depths);
                da = da.join(ad);
                ex = ex.join(aex);
                cur_pc = aex.advance(cur_pc);
            }
            let rd = match callee.ty {
                Type::Fn { result_depth, .. } => result_depth,
                _ => Depth::PURE,
            };
            (rd.join(df).join(da), ex)
        }
        ExprKind::Descend { body, .. } => refresh_expr(body, pc, steps, depths),
        ExprKind::Rite { rite, operand } => {
            let (od, ex) = refresh_expr(operand, pc, steps, depths);
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
            let (dc, ec) = refresh_expr(cond, pc, steps, depths);
            let bpc = ec.advance(pc).join(dc);
            let mut else_depths = depths.clone();
            let (dt, et) = refresh_expr(then, bpc, steps, depths);
            let (de, ee) = refresh_expr(otherwise, bpc, steps, &mut else_depths);
            for (id, &d) in else_depths.iter() {
                depths.join(id, d);
            }
            (dc.join(dt).join(de), ec.join(et).join(ee))
        }
        ExprKind::Block(b) => refresh_stmts(&mut b.stmts, &mut b.tail, pc, steps, depths),
        ExprKind::Loop { body, step } => refresh_loop(body, step.as_deref_mut(), pc, steps, depths),
        ExprKind::Return(v) => {
            let (d, ex) = v
                .as_mut()
                .map_or((Depth::PURE, ExitPc::default()), |e| refresh_expr(e, pc, steps, depths));
            (d, ex.join(ExitPc { loop_exit: None, fn_exit: Some(ex.advance(pc)) }))
        }
        ExprKind::Assign { place, value } => {
            let (mut cur_pc, mut ex, mut dp) = (pc, ExitPc::default(), Depth::PURE);
            for p in &mut place.path {
                if let nether_core::Proj::Index(i) = p {
                    let (di, ei) = refresh_expr(i, cur_pc, steps, depths);
                    dp = dp.join(di);
                    ex = ex.join(ei);
                    cur_pc = ei.advance(cur_pc);
                }
            }
            let (dv, ev) = refresh_expr(value, cur_pc, steps, depths);
            ex = ex.join(ev);
            let wd = dp.join(dv).join(ev.advance(cur_pc));
            depths.join(place.local.0, wd);
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
    depths: &mut LocalMap<Depth>,
) -> (Depth, ExitPc) {
    let mut loop_pc = pc;
    let mut res = (Depth::PURE, ExitPc::default());
    for _ in 0..64 {
        if *steps >= MAX_FIXPOINT_STEPS {
            break;
        }
        *steps += 1;
        let before = (depths.clone(), loop_pc);
        let (db, eb) = refresh_expr(body, loop_pc, steps, depths);
        let step_pc = eb.advance(loop_pc);
        let (ds, es) = match step.as_deref_mut() {
            Some(s) => refresh_expr(s, step_pc, steps, depths),
            None => (Depth::PURE, ExitPc::default()),
        };
        let combined = eb.join(es);
        loop_pc = combined.advance(loop_pc);
        res = (db.join(ds), ExitPc { loop_exit: None, fn_exit: combined.fn_exit });
        if (&*depths, loop_pc) == (&before.0, before.1) {
            break;
        }
    }
    res
}

fn reaches(x: &Expr, unit: &Unit, check_block_deposits: bool, pred: fn(&Expr) -> bool) -> bool {
    let mut todo: Vec<&Expr> = vec![x];
    let mut seen_funcs = LocalSet::default();
    let mut seen_globals = LocalSet::default();
    while let Some(cur) = todo.pop() {
        if pred(cur) {
            return true;
        }
        match &cur.kind {
            ExprKind::Func(id) if seen_funcs.insert(id.0) => {
                if let Some(f) = unit.func(*id) {
                    if check_block_deposits
                        && f.body
                            .stmts
                            .iter()
                            .any(|s| matches!(s, Stmt::Expr(e) if e.ty != Type::Unit))
                    {
                        return true;
                    }
                    todo.extend(block_exprs(&f.body));
                }
            }
            ExprKind::Global(id) if seen_globals.insert(id.0) => {
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

/// Whether `x` can deposit a non-`U0` value (directly or through called
/// functions / referenced globals).
pub(crate) fn reaches_deposit(x: &Expr, unit: &Unit) -> bool {
    reaches(
        x,
        unit,
        true,
        |cur| matches!(&cur.kind, ExprKind::Block(b) if b.stmts.iter().any(|s| matches!(s, Stmt::Expr(e) if e.ty != Type::Unit))),
    )
}

/// Whether `x` can reach any act-stratum primitive (directly or through called
/// functions / referenced globals).
pub(crate) fn reaches_act(x: &Expr, unit: &Unit) -> bool {
    reaches(x, unit, false, |cur| {
        matches!(cur.kind, ExprKind::Prim(p) if p.latent() > Depth::PURE && !p.latent().reads())
            || matches!(cur.ty, Type::Fn { latent, .. } if [Depth::DISK_WRITE, Depth::NET_WRITE, Depth::ENTROPY, Depth::UNRECORDED].into_iter().any(|d| latent.holds(d)))
    })
}

fn any_expr(x: &Expr, pred: fn(&Expr) -> bool) -> bool {
    let mut todo = vec![x];
    while let Some(cur) = todo.pop() {
        if pred(cur) {
            return true;
        }
        todo.extend(children(cur));
    }
    false
}

/// Whether `x` contains any `opaque` rite.
pub(crate) fn has_opaque(x: &Expr) -> bool {
    any_expr(x, |c| matches!(c.kind, ExprKind::Rite { rite: Rite::Opaque, .. }))
}

/// Whether `x` contains any `Assign` whose checked depth depends on ambient `pc`.
pub(crate) fn has_assign(x: &Expr) -> bool {
    any_expr(x, |c| matches!(c.kind, ExprKind::Assign { .. }))
}

/// Shallowest `Return` value depth inside `x`, if any.
pub(crate) fn min_ret_depth(x: &Expr) -> Option<Depth> {
    let mut best: Option<Depth> = None;
    visit_exprs(x, |cur| {
        if let ExprKind::Return(v) = &cur.kind {
            let d = v.as_deref().map_or(Depth::PURE, |e| e.depth);
            best = Some(best.map_or(d, |b| b.min(d)));
        }
    });
    best
}

/// Locals referenced directly inside any non-pure-origin `Rite::Shade` operand.
pub(crate) fn shade_reads(x: &Expr, into: &mut LocalSet) {
    visit_exprs(x, |cur| {
        if let ExprKind::Rite { rite: Rite::Shade, operand } = &cur.kind
            && matches!(cur.ty, Type::Shade { origin, .. } if origin > Depth::PURE)
        {
            reads(operand, into);
        }
    });
}

/// Whether a residual block or descent's last word is a `return`.
pub(crate) fn ends_returning(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Return(_) => true,
        ExprKind::Descend { body, .. } => ends_returning(body),
        ExprKind::Block(b) => {
            b.tail.is_none()
                && matches!(b.stmts.last(), Some(Stmt::Expr(last)) if ends_returning(last))
        }
        _ => false,
    }
}
