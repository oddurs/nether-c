//! Stateless IR tree queries used by the checker and downstream passes.

use crate::ir::{Block, Expr, ExprKind, LocalId, Proj, Rite, Stmt};
use crate::ty::Type;

pub(crate) fn block_ends_with_value_select(b: &Block) -> bool {
    matches!(
        b.stmts.last(),
        Some(Stmt::Expr(
            e @ Expr { kind: ExprKind::Select { .. } | ExprKind::Block(_), .. }
        )) if expr_produces_value(e)
    )
}

fn expr_produces_value(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Block(b) => {
            b.tail.is_some() || block_always_returns(b) || block_ends_with_value_select(b)
        }
        ExprKind::Select { then, otherwise, .. } => {
            expr_produces_value(then) && expr_produces_value(otherwise)
        }
        ExprKind::Descend { body, .. } => expr_produces_value(body),
        ExprKind::Return(_) => true,
        _ => x.ty != Type::Unit,
    }
}

pub(crate) fn block_always_returns(b: &Block) -> bool {
    for e in b.children() {
        if always_returns(e) {
            return true;
        }
        if may_jump_loop(e) {
            return false;
        }
    }
    false
}

fn may_jump_loop(x: &Expr) -> bool {
    let mut todo = vec![x];
    while let Some(cur) = todo.pop() {
        match &cur.kind {
            ExprKind::Break | ExprKind::Continue => return true,
            ExprKind::Loop { .. } => {}
            _ => todo.extend(children(cur)),
        }
    }
    false
}

fn always_returns(x: &Expr) -> bool {
    match &x.kind {
        ExprKind::Return(_) => true,
        ExprKind::Descend { body, .. } | ExprKind::Rite { rite: Rite::Opaque, operand: body } => {
            always_returns(body)
        }
        ExprKind::Select { cond, then, otherwise } => {
            always_returns(cond)
                || (!may_jump_loop(cond) && always_returns(then) && always_returns(otherwise))
        }
        ExprKind::Block(b) => block_always_returns(b),
        _ => false,
    }
}

/// Every local a `let` or a declaration binds anywhere inside `x`.
pub(crate) fn bound_inside(x: &Expr) -> Vec<LocalId> {
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

fn scan_block<'a>(block: &'a Block, id: LocalId, todo: &mut Vec<&'a Expr>) -> Option<&'a Expr> {
    for s in &block.stmts {
        match s {
            Stmt::Let { local, value } if *local == id => return Some(value),
            Stmt::Let { value, .. } | Stmt::Expr(value) => todo.push(value),
            Stmt::Declare { .. } => {}
        }
    }
    todo.extend(block.tail.as_deref());
    None
}

/// What bound this local, if anything in this block did.
#[must_use]
pub fn bound_to(block: &Block, id: LocalId) -> Option<&Expr> {
    let mut todo = Vec::new();
    scan_block(block, id, &mut todo).or_else(|| bound_to_stack(todo, id))
}

pub(crate) fn bound_to_expr(x: &Expr, id: LocalId) -> Option<&Expr> {
    bound_to_stack(vec![x], id)
}

fn bound_to_stack(mut todo: Vec<&Expr>, id: LocalId) -> Option<&Expr> {
    while let Some(x) = todo.pop() {
        if let ExprKind::Block(b) = &x.kind {
            if let Some(found) = scan_block(b, id, &mut todo) {
                return Some(found);
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
        ExprKind::Block(b) => b.children().collect(),
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
