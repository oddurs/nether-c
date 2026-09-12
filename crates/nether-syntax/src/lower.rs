//! AST to IR: name resolution, desugaring, and where `@n` stops being syntax.
//!
//! Three things happen here and nowhere else.
//!
//! **Names become positions.** An identifier is a local, a unit-level binding,
//! a function, a prelude function or a refusal constant, and after this nobody
//! asks again.
//!
//! **Spelling is folded away.** `&&` and `||` and `if` become one branch,
//! `while` and `for` become one loop, `x += e` becomes an assignment of a sum.
//! The folds are listed in `nether_core::ir`, and the reason they happen here
//! rather than in the parser is that a parser that folds reports errors about
//! a program nobody wrote.
//!
//! **Depths are derived.** Every IR node states one, and this is where it
//! comes from — by the rules in `spec/02-calculus.md` §2.2, implemented a
//! second time. The checker implements them too, and `nether_core::check` on
//! the result is two implementations agreeing rather than one agreeing with
//! itself.

use std::collections::HashMap;

use nether_core::{self as ir, Asserted, Capability, Depth, Prim, Refusal, Rite, Span, UnOp};

use crate::ast;
use crate::lex::{Fault, FaultKind};

/// Lower a parsed unit.
///
/// # Errors
///
/// Every name it could not resolve and every shape it could not make sense
/// of, in source order.
pub fn lower(unit: &ast::Unit) -> Result<ir::Unit, Vec<Fault>> {
    let mut l = Lowering::new(unit);
    let out = l.unit(unit);
    if l.faults.is_empty() { Ok(out) } else { Err(l.faults) }
}

struct Lowering<'a> {
    faults: Vec<Fault>,
    /// `typedef` bodies, by the name they were given.
    aliases: HashMap<&'a str, &'a ast::Type>,
    /// Struct declarations, by name, for resolving a field to a position.
    structs: HashMap<&'a str, &'a [ast::Field]>,
    /// Unit-level functions and bindings, by name.
    funcs: HashMap<&'a str, (ir::FuncId, ir::Type)>,
    globals: HashMap<&'a str, (ir::GlobalId, ir::Type, Depth)>,
    /// The locals of the function being lowered, innermost scope last.
    scopes: Vec<Vec<(String, ir::LocalId)>>,
    locals: Vec<ir::LocalDef>,
    /// The depth of every local bound so far, by position.
    depths: Vec<Depth>,
}

impl<'a> Lowering<'a> {
    fn new(unit: &'a ast::Unit) -> Self {
        let mut l = Self {
            faults: Vec::new(),
            aliases: HashMap::new(),
            structs: HashMap::new(),
            funcs: HashMap::new(),
            globals: HashMap::new(),
            scopes: Vec::new(),
            locals: Vec::new(),
            depths: Vec::new(),
        };
        // Declarations are visible to each other regardless of order, so the
        // shapes are collected before anything is lowered.
        for item in &unit.items {
            match item {
                ast::Item::Typedef { ty, name, .. } => {
                    l.aliases.insert(name.text.as_str(), ty);
                }
                ast::Item::Struct { name, fields, .. } => {
                    l.structs.insert(name.text.as_str(), fields.as_slice());
                }
                _ => {}
            }
        }
        l
    }

    fn fault(&mut self, span: Span, kind: FaultKind) {
        self.faults.push(Fault { span, kind });
    }

    // ── types ───────────────────────────────────────────────────────────────

    /// An `ast::Type`, with aliases expanded and prelude names resolved.
    fn ty(&mut self, t: &ast::Type) -> ir::Type {
        let mut inner = self.atom(t, 0);
        for len in t.arrays.iter().rev() {
            let len = len.and_then(|n| u64::try_from(n).ok());
            inner = ir::Type::Array { elem: Box::new(inner), len };
        }
        inner
    }

    fn atom(&mut self, t: &ast::Type, expansions: u8) -> ir::Type {
        let args: Vec<ir::Type> = t.args.iter().map(|a| self.ty(a)).collect();
        match t.name.text.as_str() {
            "U0" => ir::Type::Unit,
            "Bool" => ir::Type::Bool,
            "I64" => ir::Type::Int,
            "Bytes" => ir::Type::Bytes,
            "Str" => ir::Type::Str,
            "Cairn" => ir::Type::Cairn,
            "Refusal" => ir::Type::Refusal,
            "Answer" => {
                if let Some(a) = args.into_iter().next() {
                    ir::Type::Answer(Box::new(a))
                } else {
                    self.fault(t.span, FaultKind::Expected("`Answer<T>` to say what it answers"));
                    ir::Type::Unit
                }
            }
            // §4.3: the origin of a shade is part of its type and is never
            // written. Where it is bound to something, the binding takes the
            // origin from what it holds; where it is a parameter there is
            // nothing to take it from, so it is the deepest there is, and
            // looking at one is legal nowhere. See the item on it.
            "Shade" => {
                if let Some(a) = args.into_iter().next() {
                    ir::Type::Shade { origin: Depth::UNRECORDED, inner: Box::new(a) }
                } else {
                    self.fault(t.span, FaultKind::Expected("`Shade<T>` to say what it hides"));
                    ir::Type::Unit
                }
            }
            name => {
                if let Some(alias) = self.aliases.get(name).copied() {
                    if expansions > 16 {
                        self.fault(t.span, FaultKind::Unknown("a typedef that names itself"));
                        return ir::Type::Unit;
                    }
                    let mut expanded = self.atom(alias, expansions + 1);
                    for len in alias.arrays.iter().rev() {
                        let len = len.and_then(|n| u64::try_from(n).ok());
                        expanded = ir::Type::Array { elem: Box::new(expanded), len };
                    }
                    return expanded;
                }
                if self.structs.contains_key(name) {
                    return ir::Type::Struct(name.to_string());
                }
                self.fault(t.span, FaultKind::Unknown("a type"));
                ir::Type::Unit
            }
        }
    }

    fn asserted(t: &ast::Type) -> Asserted {
        t.depth.and_then(Depth::new)
    }

    // ── the unit ────────────────────────────────────────────────────────────

    fn unit(&mut self, unit: &'a ast::Unit) -> ir::Unit {
        let mut structs = Vec::new();
        for item in &unit.items {
            if let ast::Item::Struct { name, fields, span } = item {
                let fields = fields
                    .iter()
                    .map(|f| ir::Field {
                        name: f.name.text.clone(),
                        ty: self.ty(&f.ty),
                        asserted: Self::asserted(&f.ty),
                        span: f.span,
                    })
                    .collect();
                structs.push(ir::StructDef { name: name.text.clone(), fields, span: *span });
            }
        }

        // Signatures before bodies, so a function can call one declared later.
        let declarations: Vec<&ast::Func> = unit
            .items
            .iter()
            .filter_map(|i| match i {
                ast::Item::Func(f) => Some(f),
                _ => None,
            })
            .collect();
        for (i, f) in declarations.iter().enumerate() {
            let params = f.params.iter().map(|p| self.ty(&p.ty)).collect();
            let ret = self.ty(&f.ret);
            let latent = f.latent.and_then(Depth::new).unwrap_or(Depth::PURE);
            let arrow = ir::Type::Fn {
                params,
                latent,
                result: Box::new(ret),
                // What a function hands back is its body's depth, and the body
                // has not been lowered yet. Filled in below.
                result_depth: Depth::PURE,
            };
            let id = ir::FuncId(u32::try_from(i).unwrap_or(0));
            self.funcs.insert(f.name.text.as_str(), (id, arrow));
        }

        // A call lowered before its callee's body was is holding the wrong
        // result depth, so the whole thing is lowered again with the arrows
        // filled in, until nothing moves. A chain of calls settles in as many
        // passes as the chain is long, and a bound keeps a cycle finite.
        let mut globals = Vec::new();
        let mut funcs: Vec<ir::FuncDef> = Vec::new();
        for _ in 0..8 {
            self.faults.clear();
            globals = self.unit_bindings(unit);
            funcs = declarations.iter().map(|f| self.func(f)).collect();
            let mut moved = false;
            for (i, f) in declarations.iter().enumerate() {
                let Some(entry) = self.funcs.get_mut(f.name.text.as_str()) else { continue };
                let ir::Type::Fn { result_depth, latent, .. } = &mut entry.1 else { continue };
                let derived = funcs[i].ret_depth;
                let needs = funcs[i].latent;
                moved |= *result_depth != derived || *latent != needs;
                *result_depth = derived;
                *latent = needs;
            }
            if !moved {
                break;
            }
        }
        let demands = self.demands(unit);
        ir::Unit { structs, funcs, globals, demands }
    }

    fn unit_bindings(&mut self, unit: &'a ast::Unit) -> Vec<ir::GlobalDef> {
        self.globals.clear();
        let mut out = Vec::new();
        for item in &unit.items {
            let ast::Item::Let(binding) = item else { continue };
            self.scopes.push(Vec::new());
            let value = self.expr(&binding.value);
            self.scopes.pop();
            let ty = self.binding_type(&binding.ty, &value);
            let id = ir::GlobalId(u32::try_from(out.len()).unwrap_or(0));
            self.globals.insert(binding.name.text.as_str(), (id, ty.clone(), value.depth));
            out.push(ir::GlobalDef {
                name: binding.name.text.clone(),
                ty,
                asserted: Self::asserted(&binding.ty),
                value,
                span: binding.span,
            });
        }
        out
    }

    fn demands(&mut self, unit: &'a ast::Unit) -> Vec<ir::Demand> {
        let mut out = Vec::new();
        for item in &unit.items {
            if let ast::Item::Demand { value, span } = item {
                self.scopes.push(Vec::new());
                let value = self.expr(value);
                self.scopes.pop();
                out.push(ir::Demand { value, span: *span });
            }
        }
        out
    }

    /// The declared type of a binding, with a shade's origin taken from what
    /// it is bound to. §4.3: the origin is always inferred.
    fn binding_type(&mut self, declared: &ast::Type, value: &ir::Expr) -> ir::Type {
        let ty = self.ty(declared);
        match (&ty, &value.ty) {
            (ir::Type::Shade { inner, .. }, ir::Type::Shade { origin, .. }) => {
                ir::Type::Shade { origin: *origin, inner: inner.clone() }
            }
            _ => ty,
        }
    }

    // ── functions ───────────────────────────────────────────────────────────

    fn func(&mut self, f: &'a ast::Func) -> ir::FuncDef {
        self.locals = Vec::new();
        self.depths = Vec::new();
        self.scopes = vec![Vec::new()];

        let params: Vec<ir::LocalId> = f
            .params
            .iter()
            .map(|p| {
                let ty = self.ty(&p.ty);
                let asserted = Self::asserted(&p.ty);
                // [ABS]: a parameter is bound at depth 0, and its real depth
                // arrives at the call site.
                self.bind(&p.name, ty, asserted, Depth::PURE, p.span)
            })
            .collect();

        let body = self.block(&f.body);
        let ret_depth = body.tail.as_ref().map_or(Depth::PURE, |t| t.depth);
        ir::FuncDef {
            name: f.name.text.clone(),
            params,
            ret: self.ty(&f.ret),
            ret_depth: ret_depth.join(Self::returns(&body)),
            asserted_ret: Self::asserted(&f.ret),
            latent: Self::needs(&body),
            asserted_latent: f.latent.and_then(Depth::new),
            locals: std::mem::take(&mut self.locals),
            body,
            span: f.span,
        }
    }

    /// The deepest value any `return` in this body carries out.
    fn returns(block: &ir::Block) -> Depth {
        fn walk(x: &ir::Expr, acc: Depth) -> Depth {
            let acc = match &x.kind {
                ir::ExprKind::Return(Some(v)) => acc.join(v.depth),
                _ => acc,
            };
            children(x).iter().fold(acc, |a, c| walk(c, a))
        }
        block_children(block).iter().fold(Depth::PURE, |acc, x| walk(x, acc))
    }

    /// The least ambient depth at which this body checks: what an application
    /// in it asked for and the surface could not give. [ABS].
    fn needs(block: &ir::Block) -> Depth {
        fn walk(x: &ir::Expr, ambient: Depth, acc: Depth) -> Depth {
            let mut acc = acc;
            if let ir::ExprKind::Call { callee, .. } = &x.kind {
                if let ir::Type::Fn { latent, .. } = callee.ty {
                    if latent > ambient {
                        acc = acc.join(latent);
                    }
                }
            }
            let inside = match &x.kind {
                ir::ExprKind::Descend { capability, .. } => ambient.join(capability.stratum()),
                _ => ambient,
            };
            children(x).iter().fold(acc, |a, c| walk(c, inside, a))
        }
        block_children(block).iter().fold(Depth::PURE, |acc, x| walk(x, Depth::PURE, acc))
    }

    fn bind(
        &mut self,
        name: &ast::Name,
        ty: ir::Type,
        asserted: Asserted,
        depth: Depth,
        span: Span,
    ) -> ir::LocalId {
        let id = ir::LocalId(u32::try_from(self.locals.len()).unwrap_or(0));
        self.locals.push(ir::LocalDef { name: name.text.clone(), ty, asserted, span });
        self.depths.push(depth);
        if let Some(scope) = self.scopes.last_mut() {
            scope.push((name.text.clone(), id));
        }
        id
    }

    fn lookup(&self, name: &str) -> Option<ir::LocalId> {
        self.scopes
            .iter()
            .rev()
            .find_map(|s| s.iter().rev().find(|(n, _)| n == name).map(|(_, id)| *id))
    }
}

/// The subexpressions of an expression.
fn children(x: &ir::Expr) -> Vec<&ir::Expr> {
    use ir::ExprKind as K;
    match &x.kind {
        K::Literal(_)
        | K::Local(_)
        | K::Global(_)
        | K::Func(_)
        | K::Prim(_)
        | K::Break
        | K::Continue
        | K::SizeOf(_) => Vec::new(),
        K::Call { callee, args } => {
            let mut out = vec![&**callee];
            out.extend(args);
            out
        }
        K::Unary { operand, .. }
        | K::Rite { operand, .. }
        | K::Descend { body: operand, .. }
        | K::Field { base: operand, .. } => vec![operand],
        K::Binary { lhs, rhs, .. } | K::Index { base: lhs, index: rhs } => vec![lhs, rhs],
        K::Select { cond, then, otherwise } => vec![cond, then, otherwise],
        K::Loop { body, step } => {
            let mut out = vec![&**body];
            out.extend(step.as_deref());
            out
        }
        K::Return(v) => v.as_deref().into_iter().collect(),
        K::Block(b) => block_children(b),
        K::Assign { place, value } => {
            let mut out: Vec<&ir::Expr> = place
                .path
                .iter()
                .filter_map(|p| match p {
                    ir::Proj::Index(i) => Some(&**i),
                    ir::Proj::Field(_) => None,
                })
                .collect();
            out.push(value);
            out
        }
    }
}

fn block_children(b: &ir::Block) -> Vec<&ir::Expr> {
    let mut out: Vec<&ir::Expr> = b
        .stmts
        .iter()
        .map(|s| match s {
            ir::Stmt::Let { value, .. } | ir::Stmt::Expr(value) => value,
        })
        .collect();
    out.extend(b.tail.as_deref());
    out
}

// ── statements and expressions ──────────────────────────────────────────────

impl Lowering<'_> {
    fn at(kind: ir::ExprKind, ty: ir::Type, depth: Depth, span: Span) -> ir::Expr {
        ir::Expr { kind, ty, depth, span }
    }

    fn unit_at(span: Span) -> ir::Expr {
        Self::at(ir::ExprKind::Literal(ir::Literal::Unit), ir::Type::Unit, Depth::PURE, span)
    }

    fn block(&mut self, b: &ast::Block) -> ir::Block {
        self.scopes.push(Vec::new());
        let mut stmts = Vec::new();
        for s in &b.stmts {
            self.stmt(s, &mut stmts);
        }
        let tail = b.tail.as_ref().map(|t| Box::new(self.expr(t)));
        self.scopes.pop();
        ir::Block { stmts, tail, span: b.span }
    }

    fn stmt(&mut self, s: &ast::Stmt, out: &mut Vec<ir::Stmt>) {
        match s {
            ast::Stmt::Let(binding) => {
                let value = self.expr(&binding.value);
                let ty = self.binding_type(&binding.ty, &value);
                let asserted = Self::asserted(&binding.ty);
                let depth = value.depth;
                let local = self.bind(&binding.name, ty, asserted, depth, binding.span);
                out.push(ir::Stmt::Let { local, value });
            }
            ast::Stmt::Expr(e) => {
                let value = self.expr(e);
                out.push(ir::Stmt::Expr(value));
            }
            ast::Stmt::Block(b) => {
                let block = self.block(b);
                let (ty, depth) = block
                    .tail
                    .as_ref()
                    .map_or((ir::Type::Unit, Depth::PURE), |t| (t.ty.clone(), t.depth));
                out.push(ir::Stmt::Expr(Self::at(ir::ExprKind::Block(block), ty, depth, b.span)));
            }
            ast::Stmt::If { .. } => {
                let value = self.branch(s);
                out.push(ir::Stmt::Expr(value));
            }
            ast::Stmt::While { cond, body, span } => {
                let looping = self.looping(Some(cond), None, body, *span);
                out.push(ir::Stmt::Expr(looping));
            }
            ast::Stmt::For { init, cond, step, body, span } => {
                // The counter belongs to the loop and not to what follows it,
                // so it gets a scope of its own — and the loop goes inside it.
                self.scopes.push(Vec::new());
                let mut inner = Vec::new();
                if let Some(init) = init {
                    self.stmt(init, &mut inner);
                }
                let looping = self.looping(cond.as_ref(), step.as_ref(), body, *span);
                self.scopes.pop();
                if inner.is_empty() {
                    out.push(ir::Stmt::Expr(looping));
                } else {
                    inner.push(ir::Stmt::Expr(looping));
                    let block = ir::Block { stmts: inner, tail: None, span: *span };
                    out.push(ir::Stmt::Expr(Self::at(
                        ir::ExprKind::Block(block),
                        ir::Type::Unit,
                        Depth::PURE,
                        *span,
                    )));
                }
            }
            ast::Stmt::Return { value, span } => {
                let value = value.as_ref().map(|v| Box::new(self.expr(v)));
                let depth = value.as_ref().map_or(Depth::PURE, |v| v.depth);
                out.push(ir::Stmt::Expr(Self::at(
                    ir::ExprKind::Return(value),
                    ir::Type::Unit,
                    depth,
                    *span,
                )));
            }
            ast::Stmt::Break(span) => out.push(ir::Stmt::Expr(Self::at(
                ir::ExprKind::Break,
                ir::Type::Unit,
                Depth::PURE,
                *span,
            ))),
            ast::Stmt::Continue(span) => out.push(ir::Stmt::Expr(Self::at(
                ir::ExprKind::Continue,
                ir::Type::Unit,
                Depth::PURE,
                *span,
            ))),
        }
    }

    /// `if`, `else if` and `else`, all of them one branch.
    fn branch(&mut self, s: &ast::Stmt) -> ir::Expr {
        let ast::Stmt::If { cond, then, otherwise, span } = s else {
            return Self::unit_at(Span::default());
        };
        let cond = self.expr(cond);
        let then_block = self.block(then);
        let then = Self::as_expr(then_block, then.span);
        let otherwise = match otherwise.as_deref() {
            Some(ast::Else::Block(b)) => {
                let block = self.block(b);
                Self::as_expr(block, b.span)
            }
            Some(ast::Else::If(inner)) => self.branch(inner),
            None => Self::unit_at(*span),
        };
        let depth = cond.depth.join(then.depth).join(otherwise.depth);
        let ty = then.ty.clone();
        Self::at(
            ir::ExprKind::Select {
                cond: Box::new(cond),
                then: Box::new(then),
                otherwise: Box::new(otherwise),
            },
            ty,
            depth,
            *span,
        )
    }

    fn as_expr(block: ir::Block, span: Span) -> ir::Expr {
        let (ty, depth) =
            block.tail.as_ref().map_or((ir::Type::Unit, Depth::PURE), |t| (t.ty.clone(), t.depth));
        Self::at(ir::ExprKind::Block(block), ty, depth, span)
    }

    /// `while` and `for` are one loop. The condition becomes the guard the
    /// body opens with, and the step stays a field so that `continue` runs it.
    fn looping(
        &mut self,
        cond: Option<&ast::Expr>,
        step: Option<&ast::Expr>,
        body: &ast::Block,
        span: Span,
    ) -> ir::Expr {
        let guard = cond.map(|c| {
            let c = self.expr(c);
            let depth = c.depth;
            let negated = Self::at(
                ir::ExprKind::Unary { op: UnOp::Not, operand: Box::new(c) },
                ir::Type::Bool,
                depth,
                span,
            );
            // A block, because `if (c) { break; }` written out is a block and
            // the two have to lower to the same thing.
            let breaking = Self::at(
                ir::ExprKind::Block(ir::Block {
                    stmts: vec![ir::Stmt::Expr(Self::at(
                        ir::ExprKind::Break,
                        ir::Type::Unit,
                        Depth::PURE,
                        span,
                    ))],
                    tail: None,
                    span,
                }),
                ir::Type::Unit,
                Depth::PURE,
                span,
            );
            ir::Stmt::Expr(Self::at(
                ir::ExprKind::Select {
                    cond: Box::new(negated),
                    then: Box::new(breaking),
                    otherwise: Box::new(Self::unit_at(span)),
                },
                ir::Type::Unit,
                depth,
                span,
            ))
        });

        let mut inner = self.block(body);
        if let Some(guard) = guard {
            inner.stmts.insert(0, guard);
        }
        let body = Self::as_expr(inner, body.span);
        let step = step.map(|s| Box::new(self.expr(s)));
        let depth = body.depth.join(step.as_ref().map_or(Depth::PURE, |s| s.depth));
        Self::at(ir::ExprKind::Loop { body: Box::new(body), step }, ir::Type::Unit, depth, span)
    }
}

// ── expressions ─────────────────────────────────────────────────────────────

impl Lowering<'_> {
    fn expr(&mut self, e: &ast::Expr) -> ir::Expr {
        let span = e.span;
        match &e.kind {
            ast::ExprKind::Int(n) => Self::at(
                ir::ExprKind::Literal(ir::Literal::Int(*n)),
                ir::Type::Int,
                Depth::PURE,
                span,
            ),
            ast::ExprKind::Str(s) => Self::at(
                ir::ExprKind::Literal(ir::Literal::Str(s.clone())),
                ir::Type::Str,
                Depth::PURE,
                span,
            ),
            ast::ExprKind::Bytes(b) => Self::at(
                ir::ExprKind::Literal(ir::Literal::Bytes(b.clone())),
                ir::Type::Bytes,
                Depth::PURE,
                span,
            ),
            ast::ExprKind::Bool(b) => Self::at(
                ir::ExprKind::Literal(ir::Literal::Bool(*b)),
                ir::Type::Bool,
                Depth::PURE,
                span,
            ),
            ast::ExprKind::Name(name) => self.name(name, &[]),
            ast::ExprKind::SizeOf(t) => {
                let t = self.ty(t);
                Self::at(ir::ExprKind::SizeOf(t), ir::Type::Int, Depth::PURE, span)
            }
            ast::ExprKind::Block(b) => {
                let block = self.block(b);
                Self::as_expr(block, span)
            }
            ast::ExprKind::Descend { capability, body } => {
                let Some(cap) = Capability::from_name(&capability.text) else {
                    self.fault(capability.span, FaultKind::Unknown("a capability"));
                    return Self::unit_at(span);
                };
                let block = self.block(body);
                let inner = Self::as_expr(block, body.span);
                let (ty, depth) = (inner.ty.clone(), inner.depth);
                Self::at(
                    ir::ExprKind::Descend { capability: cap, body: Box::new(inner) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Unary { op, operand } => {
                let operand = self.expr(operand);
                let ty = if *op == UnOp::Not { ir::Type::Bool } else { operand.ty.clone() };
                let depth = operand.depth;
                Self::at(
                    ir::ExprKind::Unary { op: *op, operand: Box::new(operand) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Rite { rite, operand } => self.rite(*rite, operand, span),
            ast::ExprKind::Binary { op, lhs, rhs } => self.binary(*op, lhs, rhs, span),
            ast::ExprKind::Assign { op, place, value } => self.assign(*op, place, value, span),
            ast::ExprKind::Call { callee, args } => self.call(callee, args, span),
            ast::ExprKind::Index { base, index } => {
                let base = self.expr(base);
                let index = self.expr(index);
                let ty = if let ir::Type::Array { elem, .. } = &base.ty {
                    (**elem).clone()
                } else {
                    self.fault(span, FaultKind::Unknown("an array to index"));
                    ir::Type::Unit
                };
                let depth = base.depth.join(index.depth);
                Self::at(
                    ir::ExprKind::Index { base: Box::new(base), index: Box::new(index) },
                    ty,
                    depth,
                    span,
                )
            }
            ast::ExprKind::Field { base, name } => {
                let base = self.expr(base);
                let (index, ty) = self.field_of(&base.ty, name);
                let depth = base.depth;
                Self::at(ir::ExprKind::Field { base: Box::new(base), index }, ty, depth, span)
            }
        }
    }

    /// An identifier: a local, a unit-level binding, a function, a prelude
    /// function, or one of the six refusal constants. In that order, so that a
    /// binding shadows a prelude name rather than colliding with it.
    fn name(&mut self, name: &ast::Name, args: &[ir::Type]) -> ir::Expr {
        let span = name.span;
        if let Some(id) = self.lookup(&name.text) {
            let (ty, depth) =
                self.locals.get(id.0 as usize).map_or((ir::Type::Unit, Depth::PURE), |l| {
                    (l.ty.clone(), self.depths[id.0 as usize])
                });
            return Self::at(ir::ExprKind::Local(id), ty, depth, span);
        }
        if let Some((id, ty, depth)) = self.globals.get(name.text.as_str()).cloned() {
            return Self::at(ir::ExprKind::Global(id), ty, depth, span);
        }
        if let Some((id, ty)) = self.funcs.get(name.text.as_str()).cloned() {
            return Self::at(ir::ExprKind::Func(id), ty, Depth::PURE, span);
        }
        if let Some(p) = Prim::from_name(&name.text) {
            return Self::at(ir::ExprKind::Prim(p), prim_arrow(p, args), Depth::PURE, span);
        }
        if let Some(r) = Refusal::from_name(&name.text) {
            return Self::at(
                ir::ExprKind::Literal(ir::Literal::Refusal(r)),
                ir::Type::Refusal,
                Depth::PURE,
                span,
            );
        }
        self.fault(span, FaultKind::Unknown("this name"));
        Self::unit_at(span)
    }

    fn field_of(&mut self, ty: &ir::Type, name: &ast::Name) -> (u32, ir::Type) {
        let ir::Type::Struct(sname) = ty else {
            self.fault(name.span, FaultKind::Unknown("a struct to take a field from"));
            return (0, ir::Type::Unit);
        };
        let Some(fields) = self.structs.get(sname.as_str()).copied() else {
            self.fault(name.span, FaultKind::Unknown("that struct"));
            return (0, ir::Type::Unit);
        };
        if let Some(i) = fields.iter().position(|f| f.name.text == name.text) {
            let ty = self.ty(&fields[i].ty);
            (u32::try_from(i).unwrap_or(0), ty)
        } else {
            self.fault(name.span, FaultKind::Unknown("a field of that name"));
            (0, ir::Type::Unit)
        }
    }

    fn rite(&mut self, rite: Rite, operand: &ast::Expr, span: Span) -> ir::Expr {
        let operand = self.expr(operand);
        let (ty, depth) = match rite {
            Rite::Seal => (ir::Type::Cairn, Depth::PURE),
            Rite::Shade => (
                ir::Type::Shade { origin: operand.depth, inner: Box::new(operand.ty.clone()) },
                Depth::PURE,
            ),
            Rite::Look => {
                if let ir::Type::Shade { origin, inner } = &operand.ty {
                    ((**inner).clone(), origin.join(operand.depth))
                } else {
                    self.fault(span, FaultKind::Unknown("a shade to look at"));
                    (ir::Type::Unit, operand.depth)
                }
            }
            Rite::Opaque => (operand.ty.clone(), operand.depth),
        };
        Self::at(ir::ExprKind::Rite { rite, operand: Box::new(operand) }, ty, depth, span)
    }

    fn binary(&mut self, op: ast::BinOp, l: &ast::Expr, r: &ast::Expr, span: Span) -> ir::Expr {
        // `&&` and `||` short-circuit, so they are control flow rather than
        // operators and become the one branching form.
        if matches!(op, ast::BinOp::And | ast::BinOp::Or) {
            let lhs = self.expr(l);
            let rhs = self.expr(r);
            let constant = ir::Literal::Bool(op == ast::BinOp::Or);
            let short =
                Self::at(ir::ExprKind::Literal(constant), ir::Type::Bool, Depth::PURE, span);
            let depth = lhs.depth.join(rhs.depth);
            let (then, otherwise) = if op == ast::BinOp::And { (rhs, short) } else { (short, rhs) };
            return Self::at(
                ir::ExprKind::Select {
                    cond: Box::new(lhs),
                    then: Box::new(then),
                    otherwise: Box::new(otherwise),
                },
                ir::Type::Bool,
                depth,
                span,
            );
        }
        let lhs = self.expr(l);
        let rhs = self.expr(r);
        let op = arithmetic(op);
        let ty = if comparison(op) { ir::Type::Bool } else { lhs.ty.clone() };
        let depth = lhs.depth.join(rhs.depth);
        Self::at(
            ir::ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
            ty,
            depth,
            span,
        )
    }

    fn assign(
        &mut self,
        op: Option<ast::BinOp>,
        target: &ast::Expr,
        value: &ast::Expr,
        span: Span,
    ) -> ir::Expr {
        // `x op= e` is an assignment of `x op e`, which means reading the
        // place as well as writing it.
        let value = match op {
            Some(op) => self.binary(op, target, value, span),
            None => self.expr(value),
        };
        let Some(place) = self.place(target) else {
            self.fault(target.span, FaultKind::NotAPlace);
            return Self::unit_at(span);
        };
        let depth = place.path.iter().fold(value.depth, |acc, p| match p {
            ir::Proj::Index(i) => acc.join(i.depth),
            ir::Proj::Field(_) => acc,
        });
        Self::at(
            ir::ExprKind::Assign { place, value: Box::new(value) },
            ir::Type::Unit,
            depth,
            span,
        )
    }

    /// The left of an assignment: a local, and a path of fields and indices
    /// from it. There is no other root, because there is no way to name a
    /// location. `spec/05-types.md` §5.2.
    fn place(&mut self, e: &ast::Expr) -> Option<ir::Place> {
        match &e.kind {
            ast::ExprKind::Name(name) => {
                let local = self.lookup(&name.text)?;
                Some(ir::Place { local, path: Vec::new() })
            }
            ast::ExprKind::Field { base, name } => {
                let mut place = self.place(base)?;
                let ty = self.expr(base).ty;
                let (index, _) = self.field_of(&ty, name);
                place.path.push(ir::Proj::Field(index));
                Some(place)
            }
            ast::ExprKind::Index { base, index } => {
                let mut place = self.place(base)?;
                let index = self.expr(index);
                place.path.push(ir::Proj::Index(Box::new(index)));
                Some(place)
            }
            _ => None,
        }
    }

    fn call(&mut self, callee: &ast::Expr, args: &[ast::Expr], span: Span) -> ir::Expr {
        // Arguments first: three prelude functions are polymorphic in what
        // they are handed, and the callee's type is not known until they are.
        let args: Vec<ir::Expr> = args.iter().map(|a| self.expr(a)).collect();
        let types: Vec<ir::Type> = args.iter().map(|a| a.ty.clone()).collect();
        let callee = match &callee.kind {
            ast::ExprKind::Name(name) => self.name(name, &types),
            _ => self.expr(callee),
        };
        let (ty, result_depth) = if let ir::Type::Fn { result, result_depth, .. } = &callee.ty {
            ((**result).clone(), *result_depth)
        } else {
            self.fault(span, FaultKind::Unknown("a function to apply"));
            (ir::Type::Unit, Depth::PURE)
        };
        // [APP]: the maximum of what comes back, what the function value cost,
        // and what the arguments cost. The latent depth is the premise, not a
        // term, which is the checker's business rather than this one's.
        let depth = args.iter().fold(result_depth.join(callee.depth), |acc, a| acc.join(a.depth));
        Self::at(ir::ExprKind::Call { callee: Box::new(callee), args }, ty, depth, span)
    }
}

fn comparison(op: ir::BinOp) -> bool {
    matches!(
        op,
        ir::BinOp::Eq
            | ir::BinOp::Ne
            | ir::BinOp::Lt
            | ir::BinOp::Le
            | ir::BinOp::Gt
            | ir::BinOp::Ge
    )
}

fn arithmetic(op: ast::BinOp) -> ir::BinOp {
    match op {
        ast::BinOp::Mul => ir::BinOp::Mul,
        ast::BinOp::Div => ir::BinOp::Div,
        ast::BinOp::Rem => ir::BinOp::Rem,
        ast::BinOp::Add => ir::BinOp::Add,
        ast::BinOp::Sub => ir::BinOp::Sub,
        ast::BinOp::Shl => ir::BinOp::Shl,
        ast::BinOp::Shr => ir::BinOp::Shr,
        ast::BinOp::Lt => ir::BinOp::Lt,
        ast::BinOp::Le => ir::BinOp::Le,
        ast::BinOp::Gt => ir::BinOp::Gt,
        ast::BinOp::Ge => ir::BinOp::Ge,
        ast::BinOp::Eq => ir::BinOp::Eq,
        ast::BinOp::Ne => ir::BinOp::Ne,
        ast::BinOp::BitXor => ir::BinOp::BitXor,
        ast::BinOp::BitOr => ir::BinOp::BitOr,
        // `&&` and `||` never reach here: they are a branch and not an
        // operator, and `binary` takes them before this is called.
        ast::BinOp::BitAnd | ast::BinOp::And | ast::BinOp::Or => ir::BinOp::BitAnd,
    }
}

/// The prelude, instantiated. `spec/09-prelude.md`.
///
/// The names and the strata are `nether_core::Prim`'s; the shapes are here,
/// because three of them are polymorphic in `T` and the IR's types have no
/// variables. `T` comes from what the call was handed.
fn prim_arrow(p: Prim, args: &[ir::Type]) -> ir::Type {
    use ir::Type as T;
    let answer = |t: T| T::Answer(Box::new(t));
    let strs = || T::Array { elem: Box::new(T::Str), len: None };
    let held = || match args.first() {
        Some(T::Answer(inner)) => (**inner).clone(),
        _ => T::Unit,
    };
    let (params, result) = match p {
        Prim::Min | Prim::Max => (vec![T::Int, T::Int], T::Int),
        Prim::Abs => (vec![T::Int], T::Int),
        Prim::Len => (vec![T::Bytes], T::Int),
        Prim::Slice => (vec![T::Bytes, T::Int, T::Int], T::Bytes),
        Prim::Concat => (vec![T::Bytes, T::Bytes], T::Bytes),
        Prim::StartsWith => (vec![T::Bytes, T::Bytes], T::Bool),
        Prim::Utf8 => (vec![T::Bytes], answer(T::Str)),
        Prim::Raw => (vec![T::Str], T::Bytes),
        Prim::Join => (vec![strs(), T::Str], T::Str),
        Prim::Split => (vec![T::Str, T::Str], strs()),
        Prim::CairnOf => (vec![T::Bytes], T::Cairn),
        Prim::Hex => (vec![T::Cairn], T::Str),
        Prim::Given => (vec![answer(held())], T::Bool),
        Prim::Refusal => (vec![answer(held())], T::Refusal),
        Prim::Must => (vec![answer(held())], held()),
        Prim::FetchNode => (vec![T::Cairn], answer(T::Bytes)),
        Prim::HasNode => (vec![T::Cairn], T::Bool),
        Prim::Env => (vec![T::Str], answer(T::Str)),
        Prim::Clock => (Vec::new(), T::Int),
        Prim::Target => (Vec::new(), T::Str),
        // The same shape from two different strata, which is the whole of the
        // difference between reading a file and fetching a page.
        Prim::Read | Prim::Get => (vec![T::Str], answer(T::Bytes)),
        Prim::List => (vec![T::Str], answer(strs())),
        Prim::Exists => (vec![T::Str], T::Bool),
        Prim::Write => (vec![T::Str, T::Bytes], answer(T::Unit)),
        Prim::Remove => (vec![T::Str], answer(T::Unit)),
        Prim::Post | Prim::CallForeign => (vec![T::Str, T::Bytes], answer(T::Bytes)),
        Prim::Draw => (vec![T::Int], T::Bytes),
    };
    // Every prelude function hands back what it went and got, so its two
    // depths are the same one.
    T::Fn { params, latent: p.latent(), result: Box::new(result), result_depth: p.latent() }
}
