//! The depth checker: `spec/02-calculus.md`, and nothing else.
//!
//! Eleven rules. The checker derives a depth for every expression and compares
//! it against the depth the IR states, so a node that claims a depth the
//! calculus does not produce is a fault rather than a fact.
//!
//! Two premises are enforced, because they are the two the specification
//! writes down: [APP]'s `dƒ ≤ δ` and [LOOK]'s `d ≤ δ`. Inside a function body
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
//! This checks depth. It is not a type checker: it reads a type exactly where
//! a depth rule depends on one, which is a shade's origin, an arrow's latent
//! depth, and what the four rites produce.

use core::fmt;

use crate::depth::{Capability, Depth};
use crate::ir::{Block, Expr, ExprKind, LocalId, Proj, Rite, Span, Stmt};
use crate::prim::Prim;
use crate::report::{Cause, Diagnostic};
use crate::ty::Type;
use crate::unit::{Asserted, FuncDef, Unit};

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
        ambient: Depth,
    },
    /// [LOOK]: the Orpheus rule. You may carry a shade up out of any stratum;
    /// you may only look at it by going back down.
    Orpheus {
        /// The stratum the shade came from.
        origin: Depth,
        /// `δ`, which is not deep enough.
        ambient: Depth,
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
        let FaultKind::Asserted { derived, .. } = self.kind else { return None };
        Some(Cause {
            span: blame.span,
            label: format!("`{}` reaches stratum {derived}", blame.what),
        })
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
            FaultKind::Stated { .. } | FaultKind::Malformed(_) => None,
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FaultKind::Ungranted { needed, ambient } => {
                write!(f, "this reaches stratum {needed}, and depth {ambient} is held here")
            }
            FaultKind::Orpheus { origin, ambient } => {
                write!(f, "cannot look at a shade from stratum {origin} at depth {ambient}")
            }
            FaultKind::Stated { derived, stated } => {
                write!(f, "the rules give depth {derived}; this node says {stated}")
            }
            FaultKind::Asserted { derived, asserted } => {
                write!(f, "annotated @{asserted}; inference gives {derived}")
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
        func: None,
        returns: Depth::PURE,
        needs: None,
    };
    // A unit-level binding may name one declared before it and not one
    // declared after, so declaration order is checking order.
    for g in &unit.globals {
        let derived = c.expr(&g.value, Depth::PURE);
        c.globals.push(derived);
        let blame = c.blame(&g.value, derived);
        c.asserted(g.span, derived, g.asserted, blame);
    }
    for f in &unit.funcs {
        c.func(f);
    }
    // [DEMAND] keeps the depth and throws the value away: `demand e : U0@d`.
    for d in &unit.demands {
        c.expr(&d.value, Depth::PURE);
    }
    c.faults
}

struct Checker<'a> {
    unit: &'a Unit,
    faults: Vec<Fault>,
    globals: Vec<Depth>,
    /// The current function's bindings. `None` until bound.
    locals: Vec<Option<Depth>>,
    /// The function being checked, for the spans of its bindings.
    func: Option<&'a FuncDef>,
    /// The deepest value any `return` has carried out of it so far.
    returns: Depth,
    /// While inside a function body: the deepest stratum an application in it
    /// has asked for and not been given. That is the function's latent depth,
    /// and it is a question for its callers rather than a fault here.
    ///
    /// `None` at the top level of a file, where there is nobody to ask.
    needs: Option<Depth>,
}

impl<'a> Checker<'a> {
    fn fault(&mut self, span: Span, kind: FaultKind) {
        self.faults.push(Fault { span, kind, blame: None });
    }

    fn fault_blaming(&mut self, span: Span, kind: FaultKind, blame: Option<Blame>) {
        self.faults.push(Fault { span, kind, blame });
    }

    /// The call that took `x` to `depth`, named.
    ///
    /// Only ever walked on the way to an error, so it may be as slow as it
    /// likes. A local is followed back to what bound it, which is how the
    /// `look` in §1.6 ends up blaming a `get` three lines above it.
    fn blame(&self, x: &Expr, depth: Depth) -> Option<Blame> {
        if let ExprKind::Call { callee, .. } = &x.kind {
            if let Type::Fn { latent, .. } = callee.ty {
                if latent == depth {
                    if let Some(what) = self.name_of(callee) {
                        return Some(Blame { span: x.span, what });
                    }
                }
            }
        }
        if let ExprKind::Local(id) = x.kind {
            if let Some(bound_to) = self.func.and_then(|f| bound_to(&f.body, id)) {
                return self.blame(bound_to, depth);
            }
        }
        if let ExprKind::Global(id) = x.kind {
            if let Some(g) = self.unit.global(id) {
                return self.blame(&g.value, depth);
            }
        }
        if let Some(found) = children(x).into_iter().find_map(|c| self.blame(c, depth)) {
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

    /// What made a block reach a depth.
    fn blame_block(&self, block: &Block, depth: Depth) -> Option<Blame> {
        let each = block.stmts.iter().map(|s| match s {
            Stmt::Let { value, .. } | Stmt::Expr(value) => value,
        });
        each.chain(block.tail.as_deref()).find_map(|x| self.blame(x, depth))
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
    fn against(
        &mut self,
        span: Span,
        derived: Depth,
        stated: Depth,
        asserted: Asserted,
        blame: Option<Blame>,
    ) {
        if derived != stated {
            self.fault(span, FaultKind::Stated { derived, stated });
        }
        self.asserted(span, derived, asserted, blame);
    }

    fn asserted(
        &mut self,
        span: Span,
        derived: Depth,
        asserted: Option<Depth>,
        blame: Option<Blame>,
    ) {
        if let Some(asserted) = asserted {
            if asserted != derived {
                self.fault_blaming(span, FaultKind::Asserted { derived, asserted }, blame);
            }
        }
    }

    fn expect_ty(&mut self, x: &Expr, want: &Type, why: &'static str) {
        if x.ty != *want {
            self.malformed(x.span, why);
        }
    }

    fn bind(&mut self, id: LocalId, depth: Depth, span: Span, blame: Option<Blame>) {
        match self.locals.get_mut(id.0 as usize) {
            Some(slot) => *slot = Some(depth),
            None => self.malformed(span, "this binding names no local"),
        }
        if let Some(l) = self.func.and_then(|f| f.local(id)) {
            let (span, asserted) = (l.span, l.asserted);
            self.asserted(span, depth, asserted, blame);
        }
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
    fn func(&mut self, f: &'a FuncDef) {
        self.func = Some(f);
        self.locals = vec![None; f.locals.len()];
        self.returns = Depth::PURE;
        self.needs = Some(Depth::PURE);
        for p in &f.params {
            self.bind(*p, Depth::PURE, f.span, None);
        }

        let ret_depth = self.block(&f.body, Depth::PURE).join(self.returns);
        let latent = self.needs.take().unwrap_or(Depth::PURE);

        let returned = self.blame_block(&f.body, ret_depth);
        self.against(f.span, ret_depth, f.ret_depth, f.asserted_ret, returned);
        let asked = self.blame_block(&f.body, latent);
        self.against(f.span, latent, f.latent, f.asserted_latent, asked);
        self.func = None;
    }

    fn block(&mut self, block: &Block, ambient: Depth) -> Depth {
        for s in &block.stmts {
            match s {
                Stmt::Let { local, value } => {
                    let d = self.expr(value, ambient);
                    let blame = self.blame(value, d);
                    self.bind(*local, d, value.span, blame);
                }
                Stmt::Expr(x) => {
                    self.expr(x, ambient);
                }
            }
        }
        block.tail.as_ref().map_or(Depth::PURE, |t| self.expr(t, ambient))
    }

    #[expect(clippy::too_many_lines, reason = "eleven rules, and an arm for each")]
    fn expr(&mut self, x: &Expr, ambient: Depth) -> Depth {
        let derived = match &x.kind {
            // [LIT], and the forms that carry nothing out. `sizeof` is a
            // constant of its type; `break` and `continue` hand no value to
            // anybody. This is why pure code disappears at burial.
            ExprKind::Literal(_) | ExprKind::SizeOf(_) | ExprKind::Break | ExprKind::Continue => {
                Depth::PURE
            }

            // [VAR].
            ExprKind::Local(id) => {
                if let Some(d) = self.locals.get(id.0 as usize).copied().flatten() {
                    d
                } else {
                    self.malformed(x.span, "this local is not bound here");
                    x.depth
                }
            }
            ExprKind::Global(id) => {
                if let Some(d) = self.globals.get(id.0 as usize).copied() {
                    d
                } else {
                    self.malformed(x.span, "this unit-level binding is not declared yet");
                    x.depth
                }
            }

            // [ABS]. Building a function that will touch the disk does not
            // touch the disk, so the closure itself is pure.
            ExprKind::Func(id) => {
                match self.unit.func(*id) {
                    Some(f) => {
                        let want = Type::Fn {
                            params: f
                                .params
                                .iter()
                                .filter_map(|p| f.local(*p))
                                .map(|l| l.ty.clone())
                                .collect(),
                            latent: f.latent,
                            result: Box::new(f.ret.clone()),
                            result_depth: f.ret_depth,
                        };
                        self.expect_ty(x, &want, "this is not the signature of that function");
                    }
                    None => self.malformed(x.span, "this names no function in the unit"),
                }
                Depth::PURE
            }
            ExprKind::Prim(p) => {
                self.prim(x, *p);
                Depth::PURE
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
                let d_f = self.expr(callee, ambient);
                let d_a = args.iter().fold(Depth::PURE, |acc, a| acc.join(self.expr(a, ambient)));
                let (latent, result_depth) =
                    if let Type::Fn { latent, result_depth, .. } = callee.ty {
                        (latent, result_depth)
                    } else {
                        self.malformed(callee.span, "this is applied and is not a function");
                        (Depth::PURE, Depth::PURE)
                    };
                if latent > ambient {
                    // Inside a function body this is not a fault: it is what
                    // the function asks of whoever calls it. At the top level
                    // of a file there is nobody to ask.
                    if let Some(needs) = self.needs {
                        self.needs = Some(needs.join(latent));
                    } else {
                        let blame = self.name_of(callee).map(|what| Blame { span: x.span, what });
                        let kind = FaultKind::Ungranted { needed: latent, ambient };
                        self.fault_blaming(x.span, kind, blame);
                    }
                }
                result_depth.join(d_f).join(d_a)
            }

            // [DESCEND]. The only rule that raises δ, and only inside its own
            // premise. Nothing lowers δ; nothing lowers d.
            ExprKind::Descend { capability, body } => {
                self.expr(body, ambient.join(capability.stratum()))
            }

            ExprKind::Rite { rite, operand } => self.rite(x, *rite, operand, ambient),

            // [PRIM], and everything the specification gives no other rule for.
            ExprKind::Unary { operand, .. } => self.expr(operand, ambient),
            ExprKind::Binary { lhs, rhs, .. } => {
                self.expr(lhs, ambient).join(self.expr(rhs, ambient))
            }
            ExprKind::Field { base, .. } => self.expr(base, ambient),
            ExprKind::Index { base, index } => {
                self.expr(base, ambient).join(self.expr(index, ambient))
            }
            ExprKind::Select { cond, then, otherwise } => self
                .expr(cond, ambient)
                .join(self.expr(then, ambient))
                .join(self.expr(otherwise, ambient)),
            ExprKind::Loop { body, step } => {
                let d = self.expr(body, ambient);
                step.as_ref().map_or(d, |s| d.join(self.expr(s, ambient)))
            }
            ExprKind::Return(v) => {
                let d = v.as_ref().map_or(Depth::PURE, |v| self.expr(v, ambient));
                self.returns = self.returns.join(d);
                d
            }
            ExprKind::Block(b) => self.block(b, ambient),

            // A write into a binding deepens it. Nothing lowers a depth, so a
            // binding is as deep as the deepest thing ever written into it,
            // and no program can observe the difference: a value that has been
            // read has already been named. `spec/05-types.md` §5.4.
            ExprKind::Assign { place, value } => {
                let d_value = self.expr(value, ambient);
                let d_path = place.path.iter().fold(Depth::PURE, |acc, p| match p {
                    Proj::Index(i) => acc.join(self.expr(i, ambient)),
                    Proj::Field(_) => acc,
                });
                match self.locals.get_mut(place.local.0 as usize) {
                    Some(slot) => {
                        let was = slot.unwrap_or(Depth::PURE);
                        *slot = Some(was.join(d_value));
                    }
                    None => self.malformed(x.span, "this assigns to no local"),
                }
                d_value.join(d_path)
            }
        };

        if derived != x.depth {
            self.fault(x.span, FaultKind::Stated { derived, stated: x.depth });
        }
        derived
    }

    /// [SEAL], [SHADE], [LOOK] and [OPAQUE].
    fn rite(&mut self, x: &Expr, rite: Rite, operand: &Expr, ambient: Depth) -> Depth {
        let d = self.expr(operand, ambient);
        match rite {
            // A cairn is a name, and a name is pure whatever it names.
            Rite::Seal => Depth::PURE,
            // The origin is carried in the type, which is what makes [LOOK]
            // checkable at all.
            Rite::Shade => {
                let want = Type::Shade { origin: d, inner: Box::new(operand.ty.clone()) };
                self.expect_ty(x, &want, "this shade claims an origin its value never had");
                Depth::PURE
            }
            Rite::Look => {
                if let Type::Shade { origin, .. } = operand.ty {
                    // Unlike [APP]'s premise this does not become a question
                    // for the caller. §1.6: the check is local, it does not
                    // propagate, and it does not stain the enclosing scope —
                    // and a latent depth inferred from a `look` would be that
                    // staining, one scope out.
                    if origin > ambient {
                        let blame = self.blame(operand, origin);
                        self.fault_blaming(x.span, FaultKind::Orpheus { origin, ambient }, blame);
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
            Rite::Opaque => d,
        }
    }

    /// A prelude reference states an instantiated signature. Only the latent
    /// depth is checked: three of the thirty are polymorphic in `T`, and the
    /// IR's types deliberately have no variables.
    fn prim(&mut self, x: &Expr, p: Prim) {
        // Every prelude function hands back what it went and got, so its two
        // depths are the same one. Nothing else in the language has to be.
        match &x.ty {
            Type::Fn { latent, result_depth, .. }
                if *latent == p.latent() && *result_depth == p.latent() => {}
            Type::Fn { .. } => self.malformed(x.span, "this prelude function has another stratum"),
            _ => self.malformed(x.span, "a prelude function is a function"),
        }
    }
}

/// What bound this local, if anything in this block did.
fn bound_to(block: &Block, id: LocalId) -> Option<&Expr> {
    for s in &block.stmts {
        match s {
            Stmt::Let { local, value } if *local == id => return Some(value),
            Stmt::Let { value, .. } | Stmt::Expr(value) => {
                if let Some(found) = children(value).into_iter().find_map(|c| match &c.kind {
                    ExprKind::Block(b) => bound_to(b, id),
                    _ => None,
                }) {
                    return Some(found);
                }
            }
        }
    }
    block.tail.as_deref().and_then(|t| match &t.kind {
        ExprKind::Block(b) => bound_to(b, id),
        _ => None,
    })
}

/// The subexpressions of an expression, in evaluation order.
fn children(x: &Expr) -> Vec<&Expr> {
    match &x.kind {
        ExprKind::Literal(_)
        | ExprKind::Local(_)
        | ExprKind::Global(_)
        | ExprKind::Func(_)
        | ExprKind::Prim(_)
        | ExprKind::Break
        | ExprKind::Continue
        | ExprKind::SizeOf(_) => Vec::new(),
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
                .map(|s| match s {
                    Stmt::Let { value, .. } | Stmt::Expr(value) => value,
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
