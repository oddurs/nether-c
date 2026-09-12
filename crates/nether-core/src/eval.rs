//! Burial: the only form of evaluation Nether C has.
//!
//! `spec/06-evaluation.md`. Burial evaluates every expression it can and stops
//! at every expression it cannot, and what survives is the **residue**: a
//! complete program that can be buried again.
//!
//! What it can evaluate is decided by depth and nothing else. A depth-0
//! expression is arithmetic on things that are already here, so it is reduced;
//! anything deeper is a question for the world, and no capability has been
//! granted to anything yet, so it stays. `spec/06-evaluation.md` §6.1.
//!
//! Evaluation is demand-driven. A unit-level binding is evaluated the first
//! time something reaches it and never otherwise, and a function body is
//! evaluated when it is called. Nothing a `demand` does not transitively
//! require is touched — not as an optimisation but because evaluating it could
//! reach the world and leave a witness the program never asked for (§6.2).
//!
//! Three things are deliberately not here yet.
//!
//! - **Holes.** A world-question residualises as the expression that asked it
//!   rather than as a `Hole` node. Giving it a name and a place in the ledger
//!   is *Holes and residualisation*.
//! - **`seal`.** Naming a value means encoding it and hashing it, which is the
//!   ledger, and this crate does not depend on the ledger. It residualises.
//! - **Aggregates.** No prelude function that builds an array or a struct is
//!   folded, because there is no way to write one down and a value that cannot
//!   be written down cannot be residualised. See the item *a struct cannot be
//!   constructed*.

use crate::depth::{Capability, Depth};
use crate::ir::{BinOp, Block, Expr, ExprKind, FuncId, Literal, Place, Rite, Span, Stmt, UnOp};
use crate::prim::Prim;
use crate::report::{Diagnostic, grouped};
use crate::ty::Type;
use crate::unit::Unit;

/// What a burial produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residue {
    /// One residual expression per demand, in source order. A demand that
    /// reduced all the way is a literal.
    pub demands: Vec<Expr>,
    /// Evaluation steps spent. Deterministic for a given unit and budget.
    pub fuel_spent: u64,
    /// The deepest stratum anything in the residue still reaches.
    pub depth: Depth,
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

/// The two ways a burial does not finish.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HaltKind {
    /// An expression that cannot produce a value and never will. Not
    /// catchable, and never will be: every way to starve is a mistake in the
    /// program rather than a fact about the world.
    /// `spec/09-prelude.md` §9.9.
    Starved(&'static str),
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
}

/// How deep a chain of calls this burial can hold.
///
/// One of the two limits §6.4 requires an implementation to state. The other
/// is [`STACK`], and they are stated together because neither means anything
/// alone: burial evaluates by recursion, so a frame here is a handful of the
/// host's, and the limit is only a limit if the stack underneath it is known.
pub const MAX_FRAMES: u32 = 2048;

/// The stack a burial is given.
///
/// Burial does not run on the stack it was called on. An unoptimised build
/// spends something like sixteen kilobytes of host stack per frame, and the
/// smallest stack a caller is likely to have — a test harness thread — holds
/// about a hundred of those, which is not a depth a language can offer. So it
/// runs on a stack of its own, sized for [`MAX_FRAMES`] with room to spare,
/// and the number is written down here rather than inherited from whoever
/// called.
pub const STACK: usize = 64 << 20;

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
    /// This halt, ready to print.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        let (headline, label, note) = match self.kind {
            HaltKind::Starved(why) => (
                why.to_string(),
                None,
                Some(
                    "starvation is not catchable, and there will be no recovery form. \
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
        Diagnostic { span: self.span, headline, label, note }
    }
}

/// Bury a unit with a budget of evaluation steps.
///
/// # Errors
///
/// Starvation and exhausted fuel, which are different things: the first is a
/// mistake in the program and the second is a bound that was too small.
pub fn bury(unit: &Unit, fuel: u64) -> Result<Residue, Halt> {
    let run = || burrow(unit, fuel);
    std::thread::scope(|s| {
        match std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, run) {
            // A burial that panicked is a bug in this crate, and the caller
            // should see it as one.
            Ok(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
            // Nothing left to spawn with. Better a shallower burial than none.
            Err(_) => burrow(unit, fuel),
        }
    })
}

fn burrow(unit: &Unit, fuel: u64) -> Result<Residue, Halt> {
    let mut b = Burial {
        unit,
        left: fuel,
        spent: 0,
        globals: vec![None; unit.globals.len()],
        flow: None,
        grind: Vec::new(),
    };
    let mut demands = Vec::with_capacity(unit.demands.len());
    // In source order, which §6.2 fixes: two burials of the same input produce
    // the same trace, and that includes the order things were discovered in.
    for d in &unit.demands {
        let v = match b.expr(&d.value, &mut Vec::new()) {
            Ok(v) => v,
            Err(halt) => return Err(b.blame(halt)),
        };
        demands.push(Burial::residual(v, &d.value));
    }
    let depth = demands.iter().fold(Depth::PURE, |acc, d| acc.join(d.depth));
    Ok(Residue { demands, fuel_spent: b.spent, depth })
}

/// A value, as burial holds one.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Val {
    kind: Kind,
    ty: Type,
    depth: Depth,
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum Kind {
    /// Something that is here.
    Known(Literal),
    /// A value carried up out of a deeper stratum, still held.
    Shade(Box<Val>),
    /// A function, as a value.
    Func(FuncId),
    /// A prelude function, as a value.
    Prim(Prim),
    /// Something that is not here, and the expression that would produce it.
    Stuck(Box<Expr>),
}

impl Val {
    fn is_stuck(&self) -> bool {
        matches!(self.kind, Kind::Stuck(_))
    }

    fn int(&self) -> Option<i64> {
        match &self.kind {
            Kind::Known(Literal::Int(n)) => Some(*n),
            _ => None,
        }
    }

    fn bool(&self) -> Option<bool> {
        match &self.kind {
            Kind::Known(Literal::Bool(b)) => Some(*b),
            _ => None,
        }
    }

    fn bytes(&self) -> Option<&[u8]> {
        match &self.kind {
            Kind::Known(Literal::Bytes(b)) => Some(b),
            Kind::Known(Literal::Str(s)) => Some(s.as_bytes()),
            _ => None,
        }
    }
}

/// A jump that is in flight.
///
/// Held on the burial rather than returned, because every form has to be able
/// to stop when one is set and threading it through each return type says the
/// same thing four times.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Flow {
    Broke,
    Continued,
    Returned(Val),
}

struct Burial<'a> {
    unit: &'a Unit,
    left: u64,
    spent: u64,
    /// Unit-level bindings, evaluated the first time something reaches one.
    globals: Vec<Option<Val>>,
    /// A `break`, `continue` or `return` that has happened and not yet landed.
    flow: Option<Flow>,
    /// The loops and calls currently open, innermost last. Nothing pops it on
    /// the way out of an error, which is the point: when the budget runs out
    /// this is what evaluation was going round in.
    grind: Vec<Grind>,
}

/// One open loop or call.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Grind {
    span: Span,
    what: Open,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Open {
    Loop { turns: u64 },
    Call(FuncId),
}

/// A frame: one slot per local of the function being evaluated.
type Env = Vec<Option<Val>>;

impl Burial<'_> {
    /// Where a halt should point, and what it was going round in.
    ///
    /// `burn` reports the leaf it was holding, which is almost never the
    /// interesting place. A loop that went round two hundred thousand times is
    /// the answer to *why did this not stop*; the expression it happened to be
    /// inside is not.
    fn blame(&self, halt: Halt) -> Halt {
        if matches!(halt.kind, HaltKind::Starved(_)) {
            return halt;
        }
        let looping = self
            .grind
            .iter()
            .rev()
            .filter_map(|g| match g.what {
                Open::Loop { turns } => Some((g.span, turns)),
                Open::Call(_) => None,
            })
            .max_by_key(|(_, turns)| *turns);
        let calls = self.grind.iter().filter(|g| matches!(g.what, Open::Call(_))).count() as u64;

        match looping {
            Some((span, turns)) if turns >= calls => {
                Halt { span, grinding: Some(Grinding::Loop { turns }), ..halt }
            }
            _ => {
                let first = self.grind.iter().find_map(|g| match g.what {
                    Open::Call(id) => Some((g.span, id)),
                    Open::Loop { .. } => None,
                });
                match first {
                    Some((span, id)) => {
                        let name = self
                            .unit
                            .func(id)
                            .map_or_else(|| "a function".to_string(), |f| f.name.clone());
                        Halt { span, grinding: Some(Grinding::Calls { deep: calls, name }), ..halt }
                    }
                    None => halt,
                }
            }
        }
    }

    fn burn(&mut self, span: Span) -> Result<(), Halt> {
        if self.left == 0 {
            let kind = HaltKind::OutOfFuel { spent: self.spent };
            return Err(Halt { span, kind, grinding: None });
        }
        self.left -= 1;
        self.spent += 1;
        Ok(())
    }

    fn known(kind: Literal, x: &Expr) -> Val {
        Val { kind: Kind::Known(kind), ty: x.ty.clone(), depth: Depth::PURE }
    }

    fn unit() -> Val {
        Val { kind: Kind::Known(Literal::Unit), ty: Type::Unit, depth: Depth::PURE }
    }

    fn stuck(x: &Expr) -> Val {
        Val { kind: Kind::Stuck(Box::new(x.clone())), ty: x.ty.clone(), depth: x.depth }
    }

    /// The expression a value residualises as.
    ///
    /// A value that can be written down is written down; anything else is the
    /// expression it came from, unreduced. That is why no fold produces an
    /// array or a struct: there is no way to write one, so a residue holding
    /// one could not be a program.
    fn residual(v: Val, original: &Expr) -> Expr {
        let span = original.span;
        match v.kind {
            Kind::Known(l) => {
                Expr { kind: ExprKind::Literal(l), ty: v.ty, depth: Depth::PURE, span }
            }
            Kind::Func(id) => Expr { kind: ExprKind::Func(id), ty: v.ty, depth: Depth::PURE, span },
            Kind::Prim(p) => Expr { kind: ExprKind::Prim(p), ty: v.ty, depth: Depth::PURE, span },
            Kind::Shade(inner) => {
                let operand = Self::residual(*inner, original);
                Expr {
                    kind: ExprKind::Rite { rite: Rite::Shade, operand: Box::new(operand) },
                    ty: v.ty,
                    depth: Depth::PURE,
                    span,
                }
            }
            Kind::Stuck(e) => *e,
        }
    }

    /// A residual expression with a new shape, built from reduced parts.
    fn rebuild(kind: ExprKind, x: &Expr, depth: Depth) -> Val {
        let e = Expr { kind, ty: x.ty.clone(), depth, span: x.span };
        Val { kind: Kind::Stuck(Box::new(e)), ty: x.ty.clone(), depth }
    }

    fn expr(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.burn(x.span)?;
        Ok(match &x.kind {
            ExprKind::Literal(l) => Self::known(l.clone(), x),

            ExprKind::Local(id) => match env.get(id.0 as usize).and_then(Clone::clone) {
                Some(v) => v,
                None => Self::stuck(x),
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

            ExprKind::Loop { .. } => self.loop_(x, env)?,

            ExprKind::Descend { capability, body } => self.descend(x, *capability, body, env)?,

            ExprKind::Rite { rite, operand } => self.rite(x, *rite, operand, env)?,

            ExprKind::Assign { place, value } => self.assign(x, place, value, env)?,

            ExprKind::Break => {
                self.flow = Some(Flow::Broke);
                Self::unit()
            }
            ExprKind::Continue => {
                self.flow = Some(Flow::Continued);
                Self::unit()
            }
            ExprKind::Return(v) => {
                let out = match v {
                    Some(v) => self.expr(v, env)?,
                    None => Self::unit(),
                };
                self.flow = Some(Flow::Returned(out.clone()));
                out
            }

            // Neither has a value form to fold into. Indexing and projection
            // need an aggregate, and nothing can build one; `sizeof` needs a
            // size, and nothing has defined one.
            ExprKind::Field { .. } | ExprKind::Index { .. } | ExprKind::SizeOf(_) => Self::stuck(x),
        })
    }

    fn unary(&mut self, x: &Expr, op: UnOp, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let v = self.expr(operand, env)?;
        Ok(if let Some(l) = unary(op, &v) {
            Self::known(l, x)
        } else {
            let operand = Box::new(Self::residual(v, operand));
            let depth = operand.depth;
            Self::rebuild(ExprKind::Unary { op, operand }, x, depth)
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
        let b = self.expr(rhs, env)?;
        let folded = binary(op, &a, &b).map_err(|why| Halt {
            span: x.span,
            kind: HaltKind::Starved(why),
            grinding: None,
        })?;
        Ok(if let Some(l) = folded {
            Self::known(l, x)
        } else {
            let lhs = Box::new(Self::residual(a, lhs));
            let rhs = Box::new(Self::residual(b, rhs));
            let depth = lhs.depth.join(rhs.depth);
            Self::rebuild(ExprKind::Binary { op, lhs, rhs }, x, depth)
        })
    }

    /// A branch on something that is not here residualises whole, both arms
    /// unevaluated. Evaluating the arm that will not be taken is exactly what
    /// §6.2 forbids.
    fn select(
        &mut self,
        x: &Expr,
        cond: &Expr,
        then: &Expr,
        otherwise: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let c = self.expr(cond, env)?;
        Ok(match c.bool() {
            Some(true) => self.expr(then, env)?,
            Some(false) => self.expr(otherwise, env)?,
            None => {
                let cond = Box::new(Self::residual(c, cond));
                let kind = ExprKind::Select {
                    cond,
                    then: Box::new(then.clone()),
                    otherwise: Box::new(otherwise.clone()),
                };
                Self::rebuild(kind, x, x.depth)
            }
        })
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
        let v = self.expr(body, env)?;
        if !v.is_stuck() {
            return Ok(v);
        }
        let body = Box::new(Self::residual(v, body));
        let depth = body.depth;
        Ok(Self::rebuild(ExprKind::Descend { capability, body }, x, depth))
    }

    /// A unit-level binding, evaluated once and only if something reaches it.
    fn global(&mut self, id: crate::ir::GlobalId, x: &Expr) -> Result<Val, Halt> {
        if let Some(v) = self.globals.get(id.0 as usize).and_then(Clone::clone) {
            return Ok(v);
        }
        let Some(g) = self.unit.global(id) else {
            return Ok(Self::stuck(x));
        };
        let v = self.expr(&g.value, &mut Vec::new())?;
        if let Some(slot) = self.globals.get_mut(id.0 as usize) {
            *slot = Some(v.clone());
        }
        Ok(v)
    }

    /// [APP]. A call to a function every one of whose arguments is here is
    /// evaluated; anything else residualises as the call it was.
    ///
    /// A function is not unfolded when it cannot be finished. The residue is
    /// smaller for it, and §6.2's `compile(src)` — pure, and starving on a
    /// hole — is the call it was written as rather than an inlined body with
    /// the same hole in it.
    fn call(&mut self, x: &Expr, callee: &Expr, args: &[Expr], env: &mut Env) -> Result<Val, Halt> {
        let f = self.expr(callee, env)?;
        let mut values = Vec::with_capacity(args.len());
        for a in args {
            values.push(self.expr(a, env)?);
        }

        let folded = match &f.kind {
            Kind::Prim(p) => match prim(*p, &values) {
                Ok(l) => l.map(|l| Self::known(l, x)),
                Err(why) => {
                    let kind = HaltKind::Starved(why);
                    return Err(Halt { span: x.span, kind, grinding: None });
                }
            },
            Kind::Func(id) => self.apply(*id, &values)?,
            _ => None,
        };
        if let Some(v) = folded {
            return Ok(v);
        }

        let callee = Box::new(Self::residual(f, callee));
        let args: Vec<Expr> =
            values.into_iter().zip(args).map(|(v, a)| Self::residual(v, a)).collect();
        let depth = args.iter().fold(x.depth, |acc, a| acc.join(a.depth));
        Ok(Self::rebuild(ExprKind::Call { callee, args }, x, depth))
    }

    /// Evaluate a function body against known arguments.
    fn apply(&mut self, id: FuncId, args: &[Val]) -> Result<Option<Val>, Halt> {
        let Some(f) = self.unit.func(id) else { return Ok(None) };
        if args.iter().any(Val::is_stuck) || args.len() != f.params.len() {
            return Ok(None);
        }

        let mut frame: Env = vec![None; f.locals.len()];
        for (slot, v) in f.params.iter().zip(args) {
            if let Some(cell) = frame.get_mut(slot.0 as usize) {
                *cell = Some(v.clone());
            }
        }
        let frames = self.grind.iter().filter(|g| matches!(g.what, Open::Call(_))).count();
        if u32::try_from(frames).is_ok_and(|n| n >= MAX_FRAMES) {
            let kind = HaltKind::TooDeep { limit: MAX_FRAMES };
            return Err(Halt { span: f.span, kind, grinding: None });
        }
        self.grind.push(Grind { span: f.span, what: Open::Call(id) });
        let fell = self.block(&f.body, &mut frame)?;
        self.grind.pop();
        let out = match self.flow.take() {
            Some(Flow::Returned(v)) => v,
            // A `break` outside a loop is not a program this can run.
            Some(Flow::Broke | Flow::Continued) => return Ok(None),
            None => fell,
        };
        Ok(if out.is_stuck() { None } else { Some(out) })
    }

    fn block(&mut self, b: &Block, env: &mut Env) -> Result<Val, Halt> {
        for s in &b.stmts {
            let v = match s {
                Stmt::Let { local, value } => {
                    let v = self.expr(value, env)?;
                    let stuck = v.is_stuck();
                    if let Some(slot) = env.get_mut(local.0 as usize) {
                        *slot = Some(v.clone());
                    }
                    if stuck { v } else { Self::unit() }
                }
                Stmt::Expr(x) => self.expr(x, env)?,
            };
            // A statement that is not here takes the block with it: everything
            // after it depends on a world that has not answered yet. A jump
            // takes it too, and carries its own value.
            if v.is_stuck() || self.flow.is_some() {
                return Ok(v);
            }
        }
        match &b.tail {
            Some(t) => self.expr(t, env),
            None => Ok(Self::unit()),
        }
    }

    /// Unroll a loop as far as it will go, and abandon the unrolling whole if
    /// it stops going.
    ///
    /// Abandoning is safe because nothing evaluated so far can have reached
    /// the world: reaching the world is what made it stop.
    fn loop_(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        self.grind.push(Grind { span: x.span, what: Open::Loop { turns: 0 } });
        // `?` on the way out of `unroll` leaves the frame where it is, which
        // is how the halt finds out what was going round.
        let out = self.unroll(x, env)?;
        self.grind.pop();
        Ok(out)
    }

    fn unroll(&mut self, x: &Expr, env: &mut Env) -> Result<Val, Halt> {
        let ExprKind::Loop { body, step } = &x.kind else {
            return Ok(Self::stuck(x));
        };
        let before = env.clone();
        loop {
            let v = self.expr(body, env)?;
            match self.flow.take() {
                Some(Flow::Broke) => return Ok(Self::unit()),
                // A `return` out of a loop keeps going up.
                Some(f @ Flow::Returned(_)) => {
                    self.flow = Some(f);
                    return Ok(v);
                }
                // A `continue` still runs the step. That is the whole reason
                // the step is a field of the loop and not the last statement
                // of its body.
                Some(Flow::Continued) | None => {}
            }
            if v.is_stuck() {
                *env = before;
                return Ok(Self::stuck(x));
            }
            if let Some(s) = step {
                let sv = self.expr(s, env)?;
                if sv.is_stuck() {
                    *env = before;
                    return Ok(Self::stuck(x));
                }
            }
            // Counted at the bottom, so that a turn the fuel cut short is not
            // one this claims to have finished.
            if let Some(Grind { what: Open::Loop { turns }, .. }) = self.grind.last_mut() {
                *turns += 1;
            }
        }
    }

    fn rite(&mut self, x: &Expr, rite: Rite, operand: &Expr, env: &mut Env) -> Result<Val, Halt> {
        // `opaque` is never burned through, so its operand is not evaluated at
        // all. That is the whole of what it is for. §6.4.
        if rite == Rite::Opaque {
            return Ok(Self::stuck(x));
        }
        let v = self.expr(operand, env)?;
        Ok(match rite {
            // Naming a value is encoding it and hashing it, and that is the
            // ledger, which this crate does not know about.
            Rite::Seal => Self::stuck(x),
            Rite::Opaque => unreachable!("handled above"),
            Rite::Shade => {
                if v.is_stuck() {
                    let operand = Box::new(Self::residual(v, operand));
                    Self::rebuild(ExprKind::Rite { rite, operand }, x, Depth::PURE)
                } else {
                    Val { kind: Kind::Shade(Box::new(v)), ty: x.ty.clone(), depth: Depth::PURE }
                }
            }
            Rite::Look => {
                if let Kind::Shade(inner) = v.kind {
                    *inner
                } else {
                    let operand = Box::new(Self::residual(v, operand));
                    let depth = x.depth;
                    Self::rebuild(ExprKind::Rite { rite, operand }, x, depth)
                }
            }
        })
    }

    fn assign(
        &mut self,
        x: &Expr,
        place: &Place,
        value: &Expr,
        env: &mut Env,
    ) -> Result<Val, Halt> {
        let v = self.expr(value, env)?;
        // A path into an aggregate needs an aggregate, and nothing can build
        // one. Until that is settled the binding simply stops being known.
        if !place.path.is_empty() || v.is_stuck() {
            if let Some(slot) = env.get_mut(place.local.0 as usize) {
                *slot = None;
            }
            return Ok(Self::stuck(x));
        }
        if let Some(slot) = env.get_mut(place.local.0 as usize) {
            *slot = Some(v);
        }
        Ok(Val { kind: Kind::Known(Literal::Unit), ty: Type::Unit, depth: Depth::PURE })
    }
}

fn unary(op: UnOp, v: &Val) -> Option<Literal> {
    Some(match op {
        // Arithmetic wraps. An implementation MUST NOT make overflow
        // undefined: a canonical encoding cannot be built on top of behaviour
        // that varies by compiler. `spec/05-types.md` §5.1.
        UnOp::Neg => Literal::Int(v.int()?.wrapping_neg()),
        UnOp::BitNot => Literal::Int(!v.int()?),
        UnOp::Not => Literal::Bool(!v.bool()?),
    })
}

fn binary(op: BinOp, a: &Val, b: &Val) -> Result<Option<Literal>, &'static str> {
    if let (Some(x), Some(y)) = (a.int(), b.int()) {
        return Ok(Some(match op {
            BinOp::Add => Literal::Int(x.wrapping_add(y)),
            BinOp::Sub => Literal::Int(x.wrapping_sub(y)),
            BinOp::Mul => Literal::Int(x.wrapping_mul(y)),
            // Dividing by zero cannot produce a value and never will.
            BinOp::Div | BinOp::Rem if y == 0 => return Err("this divides by zero"),
            BinOp::Div => Literal::Int(x.wrapping_div(y)),
            BinOp::Rem => Literal::Int(x.wrapping_rem(y)),
            BinOp::BitAnd => Literal::Int(x & y),
            BinOp::BitOr => Literal::Int(x | y),
            BinOp::BitXor => Literal::Int(x ^ y),
            BinOp::Shl => Literal::Int(x.wrapping_shl(shift(y)?)),
            BinOp::Shr => Literal::Int(x.wrapping_shr(shift(y)?)),
            BinOp::Eq => Literal::Bool(x == y),
            BinOp::Ne => Literal::Bool(x != y),
            BinOp::Lt => Literal::Bool(x < y),
            BinOp::Le => Literal::Bool(x <= y),
            BinOp::Gt => Literal::Bool(x > y),
            BinOp::Ge => Literal::Bool(x >= y),
        }));
    }
    // Everything else compares, and compares structurally: two values of the
    // same type are equal exactly when their encodings are.
    // `spec/05-types.md` §5.3.
    Ok(match (&a.kind, &b.kind, op) {
        (Kind::Known(x), Kind::Known(y), BinOp::Eq) => Some(Literal::Bool(x == y)),
        (Kind::Known(x), Kind::Known(y), BinOp::Ne) => Some(Literal::Bool(x != y)),
        (Kind::Shade(x), Kind::Shade(y), BinOp::Eq) => Some(Literal::Bool(x == y)),
        (Kind::Shade(x), Kind::Shade(y), BinOp::Ne) => Some(Literal::Bool(x != y)),
        _ => None,
    })
}

/// A shift by something no shift can mean.
fn shift(n: i64) -> Result<u32, &'static str> {
    u32::try_from(n).map_err(|_| "this shifts by a width that is not one")
}

/// The prelude functions burial can finish.
///
/// Every one of them is at depth 0 and produces something with a written form.
/// The rest either need the world, need the ledger, or produce an aggregate
/// that cannot be written down. `spec/09-prelude.md` §9.2.
fn prim(p: Prim, args: &[Val]) -> Result<Option<Literal>, &'static str> {
    let int = |i: usize| args.get(i).and_then(Val::int);
    let bytes = |i: usize| args.get(i).and_then(Val::bytes);
    let two = |f: fn(i64, i64) -> i64| int(0).zip(int(1)).map(|(a, b)| Literal::Int(f(a, b)));
    Ok(match p {
        Prim::Min => two(i64::min),
        Prim::Max => two(i64::max),
        Prim::Abs => int(0).map(|n| Literal::Int(n.wrapping_abs())),
        Prim::Len => match bytes(0).map(|b| i64::try_from(b.len())) {
            Some(Ok(n)) => Some(Literal::Int(n)),
            Some(Err(_)) => return Err("this is longer than an I64 can count"),
            None => None,
        },
        Prim::Concat => bytes(0).zip(bytes(1)).map(|(a, b)| {
            let mut out = a.to_vec();
            out.extend_from_slice(b);
            Literal::Bytes(out)
        }),
        Prim::Slice => match (bytes(0), int(1), int(2)) {
            (Some(b), Some(from), Some(to)) => {
                let range = usize::try_from(from).ok().zip(usize::try_from(to).ok());
                // An out-of-range slice cannot produce a value and never will.
                // `spec/09-prelude.md` §9.9.
                match range.and_then(|(f, t)| b.get(f..t)) {
                    Some(cut) => Some(Literal::Bytes(cut.to_vec())),
                    None => return Err("this slice is outside what it is slicing"),
                }
            }
            _ => None,
        },
        Prim::StartsWith => bytes(0).zip(bytes(1)).map(|(b, p)| Literal::Bool(b.starts_with(p))),
        Prim::Raw => bytes(0).map(|b| Literal::Bytes(b.to_vec())),
        _ => None,
    })
}
