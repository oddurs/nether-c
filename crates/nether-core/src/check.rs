//! The depth checker: `spec/02-calculus.md`, and nothing else.
//!
//! Eleven rules. The checker derives a depth for every expression and compares
//! it against the depth the IR states, so a node that claims a depth the
//! calculus does not produce is a fault rather than a fact.
//!
//! Two premises are enforced, because they are the two the specification
//! writes down: [APP]'s `dƒ ≤ δ` and [LOOK]'s `d ≤ δ`. The
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

use crate::depth::Depth;
use crate::ir::{Block, Expr, ExprKind, LocalId, Proj, Rite, Span, Stmt};
use crate::prim::Prim;
use crate::ty::Type;
use crate::unit::{FuncDef, Unit};

/// Something the calculus does not derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Where it was written.
    pub span: Span,
    /// What is wrong.
    pub kind: FaultKind,
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
    };
    // A unit-level binding may name one declared before it and not one
    // declared after, so declaration order is checking order.
    for g in &unit.globals {
        let derived = c.expr(&g.value, Depth::PURE);
        c.globals.push(derived);
        c.asserted(g.span, derived, g.asserted);
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
}

impl<'a> Checker<'a> {
    fn fault(&mut self, span: Span, kind: FaultKind) {
        self.faults.push(Fault { span, kind });
    }

    fn malformed(&mut self, span: Span, why: &'static str) {
        self.fault(span, FaultKind::Malformed(why));
    }

    fn asserted(&mut self, span: Span, derived: Depth, asserted: Option<Depth>) {
        if let Some(asserted) = asserted {
            if asserted != derived {
                self.fault(span, FaultKind::Asserted { derived, asserted });
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
        if let Some(l) = self.func.and_then(|f| f.local(id)) {
            let (span, asserted) = (l.span, l.asserted);
            self.asserted(span, depth, asserted);
        }
    }

    /// [ABS]. The body is checked with the parameters at depth 0, and the
    /// arrow's latent depth is what the body reached — including through every
    /// `return`, which is the other way a value leaves.
    fn func(&mut self, f: &'a FuncDef) {
        self.func = Some(f);
        self.locals = vec![None; f.locals.len()];
        self.returns = Depth::PURE;
        for p in &f.params {
            self.bind(*p, Depth::PURE, f.span);
        }

        let latent = self.block(&f.body, Depth::PURE).join(self.returns);
        if latent != f.latent {
            self.fault(f.span, FaultKind::Stated { derived: latent, stated: f.latent });
        }
        self.asserted(f.span, latent, f.asserted);
        self.func = None;
    }

    fn block(&mut self, block: &Block, ambient: Depth) -> Depth {
        for s in &block.stmts {
            match s {
                Stmt::Let { local, value } => {
                    let d = self.expr(value, ambient);
                    self.bind(*local, d, value.span);
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
                let latent = if let Type::Fn { latent, .. } = callee.ty {
                    latent
                } else {
                    self.malformed(callee.span, "this is applied and is not a function");
                    Depth::PURE
                };
                if latent > ambient {
                    self.fault(x.span, FaultKind::Ungranted { needed: latent, ambient });
                }
                latent.join(d_f).join(d_a)
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
                    if origin > ambient {
                        self.fault(x.span, FaultKind::Orpheus { origin, ambient });
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
        match &x.ty {
            Type::Fn { latent, .. } if *latent == p.latent() => {}
            Type::Fn { .. } => self.malformed(x.span, "this prelude function has another stratum"),
            _ => self.malformed(x.span, "a prelude function is a function"),
        }
    }
}
