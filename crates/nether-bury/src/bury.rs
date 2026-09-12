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
//! A world-question becomes a `Hole` node with a name, and `seal` becomes the
//! cairn of what it named. Both of those are the ledger, which is why this
//! crate depends on it — but nothing is written anywhere. Writing to a store is
//! stratum 1 and burial holds no capability at all, so what comes back is the
//! nodes a rite will write when one is given the capability to.
//!
//! One thing is deliberately not here yet. **Aggregates**: no prelude function
//! that builds an array or a struct is folded, because there is no way to write
//! one down and a value that cannot be written down cannot be residualised.
//! 0116.

use std::collections::{HashMap, HashSet};

use nether_core::{
    BinOp, Block, Capability, Demand, Depth, Diagnostic, Expr, ExprKind, FuncId, GlobalId, Literal,
    Place, Prim, Refusal as RefusalCode, Rite, Span, Stmt, Type, UnOp, Unit, grouped,
};
use nether_ledger::{Cairn, Node, Stored, Value};

/// What a burial produced.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Residue {
    /// One residual expression per demand, in source order. A demand that
    /// reduced all the way is a literal.
    pub demands: Vec<Expr>,
    /// Everything burial named, in the order it named it — the values it
    /// finished and the nodes it made about them. Nothing has been written
    /// anywhere: writing to a store is stratum 1, and burial holds no
    /// capability at all.
    pub named: Vec<(Cairn, Stored)>,
    /// What the program deposited, in source order.
    ///
    /// A bare expression statement whose value is not `U0` deposits that value
    /// into the trace (§4.7). It is not printed and it is not discarded — a
    /// program holds no capability that reaches a terminal, so depositing is
    /// the only thing it *can* do with a value it wants kept.
    pub deposits: Vec<Cairn>,
    /// The holes, in the order they were discovered — which §6.2 fixes, since
    /// two burials of the same input produce the same trace including that
    /// order.
    pub holes: Vec<Cairn>,
    /// Evaluation steps spent. Deterministic for a given unit and budget.
    pub fuel_spent: u64,
    /// The deepest stratum anything in the residue still reaches.
    ///
    /// Not `Trace`'s `depth`, which is the deepest stratum a witness *reached*
    /// — `spec/07-ledger.md` §7.3.2. A residue that has been answered nothing
    /// still reaches deep; a trace that has done nothing has not.
    pub depth: Depth,
}

impl Residue {
    /// Whatever this burial filed under that name.
    ///
    /// A scan. `named` is in the order things were named, which is what §6.2
    /// fixes and what a writer iterates; this is for looking one up
    /// afterwards, which nothing does in a loop.
    #[must_use]
    pub fn get(&self, cairn: Cairn) -> Option<&Stored> {
        self.named.iter().find(|(c, _)| *c == cairn).map(|(_, s)| s)
    }

    /// The value of that name, if it named a value.
    #[must_use]
    pub fn value(&self, cairn: Cairn) -> Option<&Value> {
        match self.get(cairn) {
            Some(Stored::Value(v)) => Some(v),
            _ => None,
        }
    }

    /// The residue as a program, ready to be printed.
    ///
    /// §6.5 requires that a residue can be written down as source and lowered
    /// again to the same program; without that the staging law is unprovable,
    /// because the second burial would not be burying the first one's residue.
    ///
    /// The declarations come from the unit that was buried. Burial reduces
    /// demands and never touches a `struct`, a function or a global, so
    /// carrying them across loses nothing and an unreduced call still has
    /// something to name.
    ///
    /// # Panics
    ///
    /// If this residue did not come from that unit. Burial produces exactly one
    /// residual per demand, so a mismatch is a bug here rather than a program
    /// that can be reported on — and zipping the two would drop a demand and
    /// break the staging law with nothing said.
    #[must_use]
    pub fn as_unit(&self, buried: &Unit) -> Unit {
        assert_eq!(
            self.demands.len(),
            buried.demands.len(),
            "this residue has {} demands and the unit it is said to come from has {}",
            self.demands.len(),
            buried.demands.len()
        );
        Unit {
            structs: buried.structs.clone(),
            funcs: buried.funcs.clone(),
            globals: buried.globals.clone(),
            demands: self
                .demands
                .iter()
                .zip(&buried.demands)
                .map(|(value, original)| Demand { value: value.clone(), span: original.span })
                .collect(),
        }
    }

    /// What the world is being asked, in order.
    #[must_use]
    pub fn questions(&self) -> Vec<&Node> {
        self.holes
            .iter()
            .filter_map(|c| match self.get(*c) {
                Some(Stored::Node(n)) => Some(n),
                _ => None,
            })
            .collect()
    }
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
    /// catchable, and never will be: every way to collapse is a mistake in
    /// the program rather than a fact about the world.
    ///
    /// Not to be confused with *starving*, which is what an expression does
    /// while it waits on a hole — that is ordinary, and is not a halt at all.
    /// `spec/09-prelude.md` §9.9.
    Collapsed(&'static str),
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
    /// There was no thread to bury on.
    ///
    /// [`MAX_FRAMES`] is a limit against [`STACK`] and means nothing against
    /// anybody else's, so a burial that cannot have its own stack does not run
    /// on the caller's — it says so. §6.4: a limit is reported, not crashed
    /// into.
    NoStack,
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
            HaltKind::Collapsed(why) => (
                why.to_string(),
                None,
                Some(
                    "a collapse is not catchable, and there will be no recovery form. \
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
            HaltKind::NoStack => (
                format!("burial could not get a stack of {} bytes", grouped(STACK as u64)),
                None,
                Some(
                    "burial runs on a stack of its own: its frame limit means \
                     nothing against anybody else's."
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
        Diagnostic { span: self.span, headline, label, cause: None, note }
    }
}

/// Bury a unit with a budget of evaluation steps.
///
/// # Errors
///
/// Starvation and exhausted fuel, which are different things: the first is a
/// mistake in the program and the second is a bound that was too small.
/// What the world has already said, for a burial that is not the first.
///
/// Burial never asks. An answer is in here because a rite that held a grant
/// got it and wrote it to the ledger before this ran — which is §1.4's order,
/// kept by putting the asking somewhere burial cannot reach. Given one, a
/// world-question folds to what was said instead of becoming a hole. §6.6.
#[derive(Debug, Clone, Default)]
pub struct Answers(std::collections::HashMap<nether_ledger::Call, Value>);

impl Answers {
    /// Nothing has been answered. Every world-question becomes a hole.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// What the world said to that question, as it was written down.
    #[must_use]
    pub fn and(mut self, asked: nether_ledger::Call, said: Value) -> Self {
        self.0.insert(asked, said);
        self
    }

    fn get(&self, asked: &nether_ledger::Call) -> Option<&Value> {
        self.0.get(asked)
    }

    /// How many questions have been answered.
    #[must_use]
    pub fn len(&self) -> usize {
        self.0.len()
    }

    /// Whether nothing has.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }
}

/// Bury a unit with a budget of evaluation steps.
///
/// `source` names the bytes the unit was lowered from. Spans in the IR are
/// offsets into it and become ledger spans here, because this is where a span
/// stops being a fact about a file somebody has open and starts being a fact
/// about a source that has a name. `spec/07-ledger.md` §7.3.
///
/// # Errors
///
/// Starvation and exhausted fuel, which are different things, and this
/// implementation's own frame limit, which is a third.
pub fn bury(unit: &Unit, source: Cairn, fuel: u64) -> Result<Residue, Halt> {
    bury_with(unit, source, fuel, &Answers::none())
}

/// Bury a unit that the world has already answered some of.
///
/// The first burial of a program is [`bury`], which is this with nothing
/// answered. §6.6's exhumation is this with what a grant got, and §6.5's
/// staging law is the claim that the two routes agree.
///
/// # Errors
///
/// As [`bury`].
pub fn bury_with(
    unit: &Unit,
    source: Cairn,
    fuel: u64,
    answers: &Answers,
) -> Result<Residue, Halt> {
    let run = || burrow(unit, source, fuel, answers);
    std::thread::scope(|s| {
        match std::thread::Builder::new().stack_size(STACK).spawn_scoped(s, run) {
            // A burial that panicked is a bug in this crate, and the caller
            // should see it as one.
            Ok(h) => h.join().unwrap_or_else(|p| std::panic::resume_unwind(p)),
            // Nothing left to spawn with. Not a reason to run on the
            // caller's stack: MAX_FRAMES is thirty-two megabytes of frames
            // against a main thread that has eight, so the fallback would be
            // the overflow this thread exists to prevent.
            Err(_) => Err(Halt { span: Span::default(), kind: HaltKind::NoStack, grinding: None }),
        }
    })
}

fn burrow(unit: &Unit, source: Cairn, fuel: u64, answers: &Answers) -> Result<Residue, Halt> {
    let mut b = Burial {
        unit,
        source,
        left: fuel,
        spent: 0,
        globals: vec![None; unit.globals.len()],
        flow: None,
        grind: Vec::new(),
        answers,
        named: Vec::new(),
        known: HashSet::new(),
        holes: Vec::new(),
        deposits: Vec::new(),
        asked: HashMap::new(),
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
    Ok(Residue {
        demands,
        named: b.named,
        deposits: b.deposits,
        holes: b.holes,
        fuel_spent: b.spent,
        depth,
    })
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
    /// What the world said: §5.1.1's `Given T`.
    ///
    /// There is no literal for an answer — §04 has no syntax to write one down
    /// — so this never becomes a residual. An answer that `must`, `given` or
    /// `refusal` consumes folds through; one that is bound stays the question
    /// it was and remains a hole for a later burial.
    Answered(Box<Val>),
    /// And §5.1.1's `Refused`, which is an answer like any other. §9.9.
    Refused(RefusalCode),
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
    /// What the world already said. See [`Answers`].
    answers: &'a Answers,
    /// The cairn of the bytes the unit was lowered from.
    source: Cairn,
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
    /// Everything named so far, in the order it was named.
    named: Vec<(Cairn, Stored)>,
    /// The same names, for asking whether one is already there.
    ///
    /// `named` keeps the order §6.2 fixes and this keeps the lookup constant:
    /// scanning the vector made naming N values cost N², and §6.6 counts nine
    /// hundred nodes for a program with one hole in it.
    known: HashSet<Cairn>,
    /// The holes, in the order they were found.
    holes: Vec<Cairn>,
    /// What the program deposited, in source order.
    deposits: Vec<Cairn>,
    /// The holes already dug, by the question each one asks.
    ///
    /// §6.3 makes the `call` the identity and not the node. A node carries the
    /// span of the place that asked, so interning on the node would make the
    /// same question asked from two places into two questions.
    asked: HashMap<nether_ledger::Call, Cairn>,
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
    /// Write a node down and hand back its name.
    ///
    /// Nothing is written to a store. Writing to a store is stratum 1 and
    /// burial holds no capability at all; these are the nodes a rite will
    /// write when one is given the capability to.
    fn remember(&mut self, what: Stored) -> Cairn {
        let cairn = what.cairn();
        if self.known.insert(cairn) {
            self.named.push((cairn, what));
        }
        cairn
    }

    /// A question for the world, named and written down.
    ///
    /// Two holes with identical calls in one trace MUST be the same hole,
    /// which is what makes exhumation cheap: reading the same file twice is
    /// one question, asked once. §6.3.
    fn dig(&mut self, p: Prim, args: Vec<Value>, span: Span) -> Cairn {
        // The arguments are named as values rather than wrapped in anything.
        // A value has one name, and `seal` of the same value gives the same
        // one, which is the whole point of addressing by content.
        let args = args.into_iter().map(|v| self.remember(Stored::Value(v))).collect();
        let call = nether_ledger::Call { function: p.name().to_string(), args };
        // §6.3: two holes with identical calls in one trace MUST be the same
        // hole. The second place to ask is not a second question, so it gets
        // the hole the first one made — span and all.
        if let Some(dug) = self.asked.get(&call) {
            return *dug;
        }
        let node = Node::Hole {
            call: call.clone(),
            stratum: p.latent().get(),
            span: nether_ledger::Span {
                source: self.source,
                start: u64::from(span.start),
                end: u64::from(span.end),
            },
        };
        let cairn = self.remember(Stored::Node(node));
        self.asked.insert(call, cairn);
        self.holes.push(cairn);
        cairn
    }

    /// A value the program left behind, named and written down.
    ///
    /// §4.7. Unlike a hole, two identical deposits are two deposits: the
    /// program said the same thing twice and the trace records that it did.
    /// They differ by span, so they differ by cairn.
    fn deposit(&mut self, value: Value, span: Span) -> Cairn {
        let at = self.remember(Stored::Value(value));
        let node = Node::Deposit {
            value: at,
            span: nether_ledger::Span {
                source: self.source,
                start: u64::from(span.start),
                end: u64::from(span.end),
            },
        };
        self.remember(Stored::Node(node))
    }

    /// Where a halt should point, and what it was going round in.
    ///
    /// `burn` reports the leaf it was holding, which is almost never the
    /// interesting place. A loop that went round two hundred thousand times is
    /// the answer to *why did this not stop*; the expression it happened to be
    /// inside is not.
    fn blame(&self, halt: Halt) -> Halt {
        if matches!(halt.kind, HaltKind::Collapsed(_)) {
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

    /// `given`, `refusal` and `must`, which are how §9.9 says a program
    /// handles a no.
    ///
    /// Two of them collapse rather than answer, and §9.9 is explicit that this
    /// is not catchable: "every way to collapse is a mistake in the program
    /// rather than a fact about the world".
    fn inspect(p: Prim, values: &[Val], x: &Expr) -> Result<Option<Val>, Halt> {
        let collapse =
            |why| Err(Halt { span: x.span, kind: HaltKind::Collapsed(why), grinding: None });
        let Some(answer) = values.first() else { return Ok(None) };
        Ok(match (p, &answer.kind) {
            (Prim::Must, Kind::Answered(v)) => Some((**v).clone()),
            (Prim::Must, Kind::Refused(_)) => return collapse("this answer was refused"),
            (Prim::Given, Kind::Answered(_)) => Some(Self::known(Literal::Bool(true), x)),
            (Prim::Given, Kind::Refused(_)) => Some(Self::known(Literal::Bool(false), x)),
            (Prim::Refusal, Kind::Refused(r)) => Some(Self::known(Literal::Refusal(*r), x)),
            (Prim::Refusal, Kind::Answered(_)) => return collapse("this answer was given"),
            _ => None,
        })
    }

    /// What the world said, folded into the expression that asked.
    ///
    /// Only a literal can be folded: an aggregate has no way to be written
    /// down (0116), so an answer that is one residualises as the call it was
    /// and stays a hole for a later burial. §6.5 keeps that sound — a residue
    /// is a program, and this one still asks.
    fn answered(said: &Value, x: &Expr) -> Val {
        let Value::Answer(a) = said else { return Self::stuck(x) };
        let kind = match a.as_ref() {
            nether_ledger::AnswerOf::Refused(r) => Kind::Refused(code(*r)),
            nether_ledger::AnswerOf::Given(v) => match literal(v) {
                Some(l) => Kind::Answered(Box::new(Val {
                    kind: Kind::Known(l),
                    ty: x.ty.clone(),
                    depth: x.depth,
                })),
                // An answer with no literal form — an array of names from
                // `list`, say — cannot be folded and stays the question. 0116.
                None => return Self::stuck(x),
            },
        };
        Val { kind, ty: x.ty.clone(), depth: x.depth }
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
            // §04 has no syntax for an answer, so one that was not consumed
            // is the question it came from and stays a hole for a later
            // burial. That is sound by §6.5: a residue is a program, and this
            // one still asks.
            Kind::Answered(_) | Kind::Refused(_) | Kind::Stuck(_) => match v.kind {
                Kind::Stuck(e) => *e,
                _ => original.clone(),
            },
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

            // Neither has a value form to fold into: indexing and projection
            // need an aggregate, and nothing can build one. 0116.
            ExprKind::Field { .. } | ExprKind::Index { .. } => Self::stuck(x),
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
            kind: HaltKind::Collapsed(why),
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
    fn global(&mut self, id: GlobalId, x: &Expr) -> Result<Val, Halt> {
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

        // A question the world can answer immediately: a prelude function
        // deeper than the surface, and every argument already a value. §6.3
        // is explicit that `read(concat(dir, name))` leaves a hole holding the
        // finished path and not one holding a `concat`.
        // A question the world has already answered folds to what it said.
        // Burial holds no capability and never asks: an answer is here because
        // a rite with a grant got it and wrote it down first (§1.4, §6.6).
        if let Kind::Prim(p) = f.kind {
            if p.latent() > Depth::PURE {
                if let Some(args) = values.iter().map(as_value).collect::<Option<Vec<_>>>() {
                    let asked = nether_ledger::Call {
                        function: p.name().to_string(),
                        args: args
                            .iter()
                            .map(|v| self.remember(Stored::Value(v.clone())))
                            .collect(),
                    };
                    if let Some(said) = self.answers.get(&asked).cloned() {
                        self.remember(Stored::Value(said.clone()));
                        return Ok(Self::answered(&said, x));
                    }
                    self.dig(p, args, x.span);
                }
            }
        }

        // §9.2's three ways to look at an answer. Here rather than in `prim`
        // because what `must` gives back is a value and not a literal.
        if let Kind::Prim(p) = f.kind {
            if let Some(v) = Self::inspect(p, &values, x)? {
                return Ok(v);
            }
        }

        let folded = match &f.kind {
            Kind::Prim(p) => match prim(*p, &values) {
                Ok(l) => l.map(|l| Self::known(l, x)),
                Err(why) => {
                    let kind = HaltKind::Collapsed(why);
                    return Err(Halt { span: x.span, kind, grinding: None });
                }
            },
            Kind::Func(id) => self.apply(*id, &values)?,
            _ => None,
        };
        if let Some(v) = folded {
            return Ok(v);
        }

        // Derived from what is left, not from what was there. An argument that
        // reduced to something shallower makes the call shallower with it, and
        // a residue that states the depth it had before reducing is a residue
        // the calculus rejects. §2.4: a depth is a bound, and reduction can
        // tighten it.
        let callee = Box::new(Self::residual(f, callee));
        let result_depth = match callee.ty {
            Type::Fn { result_depth, .. } => result_depth,
            _ => Depth::PURE,
        };
        let args: Vec<Expr> =
            values.into_iter().zip(args).map(|(v, a)| Self::residual(v, a)).collect();
        let depth = args.iter().fold(result_depth.join(callee.depth), |acc, a| acc.join(a.depth));
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
                // Nothing to evaluate. The local stays empty until something
                // is written to it, and reading it before that collapses,
                // which `Place` handles. §5.4.
                Stmt::Declare { .. } => Self::unit(),
                Stmt::Expr(x) => {
                    let v = self.expr(x, env)?;
                    // §4.7: a value here is deposited, not discarded. `U0` has
                    // nothing to deposit, and one that is still waiting on the
                    // world is not a value yet — it will be deposited by the
                    // exhumation that finishes it.
                    if let Some(value) = as_value(&v) {
                        if value != Value::Unit {
                            let at = self.deposit(value, x.span);
                            self.deposits.push(at);
                        }
                    }
                    v
                }
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
        // Questions asked during an unrolling that is then abandoned are kept.
        // The loop residualises whole, so burying the residue runs it again
        // from the same state and asks the same things — they are questions
        // the residue asks, and dropping them would hide from `nether bury`
        // exactly the capability somebody needs to grant.
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
            // A name is pure, whatever it names, and a value that is here can
            // be named. One that is not stays the expression it was: sealing
            // something the world has not answered yet is a question about a
            // value that does not exist.
            Rite::Seal => {
                // §1.5: `seal` on a shade names the shade. §7.1 puts the
                // stratum it came out of in its encoding, so `seal shade e` is
                // not `seal e` — reaching through an opaque thing for a name
                // would be a hole in it.
                match as_value(&v) {
                    Some(value) => {
                        let cairn = self.remember(Stored::Value(value));
                        Self::known(Literal::Cairn(*cairn.as_bytes()), x)
                    }
                    None => Self::stuck(x),
                }
            }
            Rite::Opaque => unreachable!("handled above"),
            // The origin is whatever the operand turned out to be, which is
            // not always what it was: an arm that was not taken can make an
            // operand shallower than its type said. A shade whose origin came
            // from before the reduction claims an origin its value never had.
            Rite::Shade => {
                let origin = v.depth;
                let ty = Type::Shade { origin, inner: Box::new(v.ty.clone()) };
                if v.is_stuck() {
                    let operand = Box::new(Self::residual(v, operand));
                    let kind = ExprKind::Rite { rite, operand };
                    let e = Expr { kind, ty: ty.clone(), depth: Depth::PURE, span: x.span };
                    Val { kind: Kind::Stuck(Box::new(e)), ty, depth: Depth::PURE }
                } else {
                    Val { kind: Kind::Shade(Box::new(v)), ty, depth: Depth::PURE }
                }
            }
            Rite::Look => {
                if let Kind::Shade(inner) = v.kind {
                    *inner
                } else {
                    let operand = Box::new(Self::residual(v, operand));
                    let origin = match &operand.ty {
                        Type::Shade { origin, .. } => *origin,
                        _ => Depth::PURE,
                    };
                    let depth = origin.join(operand.depth);
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
        // §5.3 compares canonical encodings, and §7.1 puts a shade's origin
        // stratum in its own. So two shades are equal when they came out of
        // the same stratum holding the same value. Comparing them still does
        // not count as looking at them: it reveals only what `seal` on each
        // would reveal anyway.
        (Kind::Shade(x), Kind::Shade(y), BinOp::Eq | BinOp::Ne) => {
            let (Type::Shade { origin: dx, .. }, Type::Shade { origin: dy, .. }) = (&a.ty, &b.ty)
            else {
                return Ok(None);
            };
            Some(Literal::Bool(matches!(op, BinOp::Eq) == (dx == dy && x.kind == y.kind)))
        }
        _ => None,
    })
}

/// The ledger value a burial value is, when it is one.
///
/// `Refusal` is not one, and that is not a decision made here: §5.1 gives it a
/// canonical encoding and §7.1's frozen tag table has no tag for it. Until
/// that is settled a refusal cannot be named, so `seal` of one residualises
/// and a hole cannot take one as an argument.
fn as_value(v: &Val) -> Option<Value> {
    match &v.kind {
        Kind::Known(Literal::Unit) => Some(Value::Unit),
        Kind::Known(Literal::Bool(b)) => Some(Value::Bool(*b)),
        Kind::Known(Literal::Int(n)) => Some(Value::Int(*n)),
        Kind::Known(Literal::Bytes(b)) => Some(Value::Bytes(b.clone())),
        Kind::Known(Literal::Str(s)) => Some(Value::Str(s.clone())),
        Kind::Known(Literal::Cairn(c)) => Some(Value::Cairn(Cairn::from_bytes(*c))),
        // Only the origin and the name survive, which is what a shade is.
        Kind::Shade(inner) => {
            let Type::Shade { origin, .. } = &v.ty else { return None };
            as_value(inner).map(|held| Value::Shade { origin: origin.get(), value: held.cairn() })
        }
        // §5.1.1 gives a refusal and an answer each a tag, so both are values
        // the ledger can hold.
        Kind::Known(Literal::Refusal(r)) => Some(Value::Refusal(ledger_code(*r))),
        Kind::Refused(r) => {
            Some(Value::Answer(Box::new(nether_ledger::AnswerOf::Refused(ledger_code(*r)))))
        }
        Kind::Answered(inner) => as_value(inner)
            .map(|held| Value::Answer(Box::new(nether_ledger::AnswerOf::Given(held)))),
        // A function and a prelude function have no written form at all.
        Kind::Func(_) | Kind::Prim(_) | Kind::Stuck(_) => None,
    }
}

/// §5.1.1's six, from the ledger's spelling to the calculus's.
///
/// Two enumerations of one closed set, because `nether-core` does not depend on
/// the ledger and must not. The order is §5.1.1's and both say so.
fn code(r: nether_ledger::Refusal) -> RefusalCode {
    match r {
        nether_ledger::Refusal::Absent => RefusalCode::Absent,
        nether_ledger::Refusal::Denied => RefusalCode::Denied,
        nether_ledger::Refusal::Malformed => RefusalCode::Malformed,
        nether_ledger::Refusal::Unreachable => RefusalCode::Unreachable,
        nether_ledger::Refusal::Exhausted => RefusalCode::Exhausted,
        nether_ledger::Refusal::Conflict => RefusalCode::Conflict,
    }
}

/// And back the other way.
fn ledger_code(r: RefusalCode) -> nether_ledger::Refusal {
    match r {
        RefusalCode::Absent => nether_ledger::Refusal::Absent,
        RefusalCode::Denied => nether_ledger::Refusal::Denied,
        RefusalCode::Malformed => nether_ledger::Refusal::Malformed,
        RefusalCode::Unreachable => nether_ledger::Refusal::Unreachable,
        RefusalCode::Exhausted => nether_ledger::Refusal::Exhausted,
        RefusalCode::Conflict => nether_ledger::Refusal::Conflict,
    }
}

/// A value the world said, as a literal, when it has a form.
fn literal(v: &Value) -> Option<Literal> {
    Some(match v {
        Value::Unit => Literal::Unit,
        Value::Bool(b) => Literal::Bool(*b),
        Value::Int(n) => Literal::Int(*n),
        Value::Bytes(b) => Literal::Bytes(b.clone()),
        Value::Str(s) => Literal::Str(s.clone()),
        Value::Cairn(c) => Literal::Cairn(*c.as_bytes()),
        Value::Refusal(r) => Literal::Refusal(code(*r)),
        // §04 has no syntax for an aggregate, a shade or an answer, so none of
        // them can be a literal. 0116.
        Value::Struct { .. } | Value::Array(_) | Value::Shade { .. } | Value::Answer(_) => {
            return None;
        }
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
