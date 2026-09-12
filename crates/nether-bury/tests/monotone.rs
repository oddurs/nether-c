//! The property the whole design rests on, tested generatively.
//!
//! > **Law.** No evaluation step lowers the depth of a value.
//! > `spec/01-strata.md` §1.2
//!
//! Four claims are held over every generated program:
//!
//! 1. **The checker agrees with the generator.** Programs are built bottom-up
//!    with their depths computed as they go, by a second implementation of the
//!    rules in §2.2. A disagreement is one of the two being wrong.
//! 2. **The checker is not agreeing with everything.** Change one stated depth
//!    anywhere and it must complain, or claim 1 is vacuous.
//! 3. **Burial never raises a depth.** A depth in a type is an upper bound on
//!    how far into the world a value's history reaches, and reduction can only
//!    ever find out that something was shallower than it looked. §2.4.
//! 4. **A residue is a program.** What comes back out of burial goes back in
//!    to the checker clean, which is what §6.5 means by calling it complete.
//!
//! The generator is seeded, so a failure is reproducible from the seed printed
//! beside it. `scripts/task fuzz` runs the long version.

use nether_bury::bury;
use nether_core::{
    BinOp, Capability, Demand, Depth, Expr, ExprKind, Literal, Prim, Rite, Span, Type, UnOp, Unit,
    check,
};
use nether_ledger::Cairn;

/// xorshift64*. Five lines, deterministic, and nothing to install.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 { 0 } else { usize::try_from(self.next() % n as u64).unwrap_or(0) }
    }
}

/// What a position wants. Only the types a generated program can both produce
/// and consume, which keeps every program well-typed by construction.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Want {
    Int,
    Bool,
    Bytes,
    Str,
}

impl Want {
    fn ty(self) -> Type {
        match self {
            Self::Int => Type::Int,
            Self::Bool => Type::Bool,
            Self::Bytes => Type::Bytes,
            Self::Str => Type::Str,
        }
    }
}

// ── the generator ───────────────────────────────────────────────────────────
//
// Every node states the depth the rules give it, computed here rather than
// read off the checker. That is the point: two implementations of §2.2, and
// they have to agree.

struct Gen {
    rng: Rng,
}

impl Gen {
    fn at(kind: ExprKind, ty: Type, depth: Depth) -> Expr {
        Expr { kind, ty, depth, span: Span::default() }
    }

    fn literal(&mut self, want: Want) -> Expr {
        // [LIT]: a literal is at depth 0, whatever it says.
        let n = self.rng.next();
        let l = match want {
            Want::Int => Literal::Int(i64::from_ne_bytes(n.to_ne_bytes())),
            Want::Bool => Literal::Bool(n & 1 == 0),
            Want::Bytes => Literal::Bytes(vec![(n & 0xff) as u8; self.rng.below(4)]),
            Want::Str => Literal::Str("x".repeat(self.rng.below(4))),
        };
        Self::at(ExprKind::Literal(l), want.ty(), Depth::PURE)
    }

    fn prim(p: Prim, params: Vec<Type>, result: Type) -> Expr {
        let ty = Type::Fn {
            params,
            latent: p.latent(),
            result: Box::new(result),
            result_depth: p.latent(),
        };
        Self::at(ExprKind::Prim(p), ty, Depth::PURE)
    }

    /// [APP]: `max(d_r, d_f, d_a)`, and a prelude function is a pure value so
    /// `d_f` is 0.
    fn call(callee: Expr, args: Vec<Expr>, result: Type) -> Expr {
        let Type::Fn { result_depth, .. } = callee.ty else { unreachable!() };
        let depth = args.iter().fold(result_depth.join(callee.depth), |acc, a| acc.join(a.depth));
        Self::at(ExprKind::Call { callee: Box::new(callee), args }, result, depth)
    }

    fn expr(&mut self, ambient: Depth, want: Want, fuel: usize) -> Expr {
        if fuel == 0 {
            return self.literal(want);
        }
        let fuel = fuel - 1;
        let roll = self.rng.below(12);
        if roll <= 4 || roll == 11 {
            return self.any_type(ambient, want, fuel, roll);
        }
        self.this_type(ambient, want, fuel, roll)
    }

    /// The forms that work at any type: they take their operands at the type
    /// they are producing.
    fn any_type(&mut self, ambient: Depth, want: Want, fuel: usize, roll: usize) -> Expr {
        match roll {
            0 => self.literal(want),

            // [DESCEND]: the only rule that raises the ambient, and only
            // inside its own body. Two slots, because a corpus that never
            // leaves the surface proves nothing about depth.
            1 | 11 => {
                let capability = Capability::ALL[self.rng.below(Capability::ALL.len())];
                let body = self.expr(ambient.join(capability.stratum()), want, fuel);
                let (ty, depth) = (body.ty.clone(), body.depth);
                Self::at(ExprKind::Descend { capability, body: Box::new(body) }, ty, depth)
            }

            // [OPAQUE]: changes what burial does and not what the rules give.
            2 => {
                let operand = self.expr(ambient, want, fuel);
                let (ty, depth) = (operand.ty.clone(), operand.depth);
                Self::at(
                    ExprKind::Rite { rite: Rite::Opaque, operand: Box::new(operand) },
                    ty,
                    depth,
                )
            }

            // [SHADE] then [LOOK], which is the only way to consume a shade.
            // The look is legal only where the origin is already held, so an
            // operand that went deeper than the ambient is handed back as it
            // is rather than shaded and looked at illegally.
            3 => {
                let inner = self.expr(ambient, want, fuel);
                if inner.depth > ambient {
                    return inner;
                }
                let origin = inner.depth;
                let shade_ty = Type::Shade { origin, inner: Box::new(inner.ty.clone()) };
                let shaded = Self::at(
                    ExprKind::Rite { rite: Rite::Shade, operand: Box::new(inner) },
                    shade_ty,
                    Depth::PURE,
                );
                // `look s` : `τ @ max(d, d′)`, and a fresh shade is at 0.
                Self::at(
                    ExprKind::Rite { rite: Rite::Look, operand: Box::new(shaded) },
                    want.ty(),
                    origin,
                )
            }

            // [PRIM]: the maximum of the parts, through a branch.
            _ => {
                let cond = self.expr(ambient, Want::Bool, fuel);
                let then = self.expr(ambient, want, fuel);
                let otherwise = self.expr(ambient, want, fuel);
                let depth = cond.depth.join(then.depth).join(otherwise.depth);
                Self::at(
                    ExprKind::Select {
                        cond: Box::new(cond),
                        then: Box::new(then),
                        otherwise: Box::new(otherwise),
                    },
                    want.ty(),
                    depth,
                )
            }
        }
    }

    /// The forms that depend on what is being produced.
    fn this_type(&mut self, ambient: Depth, want: Want, fuel: usize, roll: usize) -> Expr {
        match (want, roll) {
            (Want::Int, 5 | 6) => {
                let lhs = self.expr(ambient, Want::Int, fuel);
                let rhs = self.expr(ambient, Want::Int, fuel);
                let depth = lhs.depth.join(rhs.depth);
                // Never `/` or `%`: dividing by zero starves, and a starved
                // burial proves nothing about depth.
                let op = [BinOp::Add, BinOp::Sub, BinOp::Mul, BinOp::BitXor][self.rng.below(4)];
                Self::at(
                    ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                    Type::Int,
                    depth,
                )
            }
            (Want::Int, 7) => {
                let operand = self.expr(ambient, Want::Int, fuel);
                let depth = operand.depth;
                Self::at(
                    ExprKind::Unary { op: UnOp::Neg, operand: Box::new(operand) },
                    Type::Int,
                    depth,
                )
            }
            (Want::Int, _) => {
                let b = self.expr(ambient, Want::Bytes, fuel);
                Self::call(Self::prim(Prim::Len, vec![Type::Bytes], Type::Int), vec![b], Type::Int)
            }

            (Want::Bool, _) => {
                let lhs = self.expr(ambient, Want::Int, fuel);
                let rhs = self.expr(ambient, Want::Int, fuel);
                let depth = lhs.depth.join(rhs.depth);
                let op = [BinOp::Eq, BinOp::Ne, BinOp::Lt, BinOp::Ge][self.rng.below(4)];
                Self::at(
                    ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) },
                    Type::Bool,
                    depth,
                )
            }

            // A question for the world, where the ambient allows one. This is
            // where a generated program gets deep for a reason other than
            // having been told to.
            (Want::Bytes, 5..=8) if ambient >= Depth::DISK => {
                let p = if ambient >= Depth::NET && self.rng.below(2) == 0 {
                    Prim::Get
                } else {
                    Prim::Read
                };
                let path = self.expr(ambient, Want::Str, fuel);
                let answer = Type::Answer(Box::new(Type::Bytes));
                let asked = Self::call(
                    Self::prim(p, vec![Type::Str], answer.clone()),
                    vec![path],
                    answer.clone(),
                );
                Self::call(
                    Self::prim(Prim::Must, vec![answer], Type::Bytes),
                    vec![asked],
                    Type::Bytes,
                )
            }
            (Want::Bytes, _) => {
                let a = self.expr(ambient, Want::Bytes, fuel);
                let b = self.expr(ambient, Want::Bytes, fuel);
                Self::call(
                    Self::prim(Prim::Concat, vec![Type::Bytes, Type::Bytes], Type::Bytes),
                    vec![a, b],
                    Type::Bytes,
                )
            }

            (Want::Str, _) => self.literal(Want::Str),
        }
    }

    fn unit(&mut self) -> Unit {
        let wants = [Want::Int, Want::Bool, Want::Bytes, Want::Str];
        let demands = (0..=self.rng.below(3))
            .map(|_| {
                let want = wants[self.rng.below(wants.len())];
                let fuel = 2 + self.rng.below(4);
                Demand { value: self.expr(Depth::PURE, want, fuel), span: Span::default() }
            })
            .collect();
        Unit { demands, ..Unit::default() }
    }
}

// ── the claims ──────────────────────────────────────────────────────────────

/// Bump the depth of the `nth` node in pre-order. `true` if there was one.
fn perturb(e: &mut Expr, nth: &mut usize) -> bool {
    if *nth == 0 {
        e.depth = if e.depth == Depth::MAX {
            Depth::PURE
        } else {
            Depth::new(e.depth.get() + 1).unwrap()
        };
        return true;
    }
    *nth -= 1;
    let children: Vec<&mut Expr> = match &mut e.kind {
        ExprKind::Unary { operand, .. } | ExprKind::Rite { operand, .. } => vec![operand],
        ExprKind::Descend { body, .. } => vec![body],
        ExprKind::Binary { lhs, rhs, .. } => vec![lhs, rhs],
        ExprKind::Select { cond, then, otherwise } => vec![cond, then, otherwise],
        ExprKind::Call { callee, args } => {
            let mut out = vec![&mut **callee];
            out.extend(args.iter_mut());
            out
        }
        _ => Vec::new(),
    };
    for c in children {
        if perturb(c, nth) {
            return true;
        }
    }
    false
}

fn source() -> Cairn {
    Cairn::of_encoded(b"generated")
}

/// Every claim, over one program. Returns the complaint if one is broken.
fn hold(unit: &Unit, rng: &mut Rng) -> Option<String> {
    // 1. The checker agrees with the generator.
    let faults = check(unit);
    if !faults.is_empty() {
        return Some(format!("the generator and the checker disagree: {faults:?}"));
    }

    // 2. And it is not agreeing with everything.
    let mut bent = unit.clone();
    let mut nth = rng.below(12);
    if bent.demands.iter_mut().any(|d| perturb(&mut d.value, &mut nth)) && check(&bent).is_empty() {
        return Some("a depth was changed and the checker said nothing".to_string());
    }

    // 3 and 4. Burial does not lower a depth, and what comes back is a program.
    let Ok(residue) = bury(unit, source(), 100_000) else {
        // Out of fuel or out of frames is not a counterexample to anything.
        return None;
    };
    for (before, after) in unit.demands.iter().zip(&residue.demands) {
        if after.depth > before.value.depth {
            return Some(format!(
                "burial raised a depth past its bound: {} became {}",
                before.value.depth, after.depth
            ));
        }
    }
    let again = Unit {
        demands: residue
            .demands
            .iter()
            .map(|value| Demand { value: value.clone(), span: Span::default() })
            .collect(),
        ..Unit::default()
    };
    let faults = check(&again);
    if !faults.is_empty() {
        return Some(format!("the residue is not a program the calculus accepts: {faults:?}"));
    }
    None
}

fn run(seed: u64, rounds: usize) -> usize {
    let mut make = Gen { rng: Rng(seed) };
    let mut deep = 0;
    for i in 0..rounds {
        let unit = make.unit();
        if unit.demands.iter().any(|d| d.value.depth > Depth::PURE) {
            deep += 1;
        }
        let mut rng = Rng(seed ^ (i as u64).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        assert!(
            hold(&unit, &mut rng).is_none(),
            "seed {seed:#x} round {i}: {}",
            hold(&unit, &mut rng).unwrap_or_default()
        );
    }
    deep
}

/// Runs on every commit. Enough to catch a regression the same day it lands.
#[test]
fn a_depth_is_a_bound_evaluation_never_exceeds() {
    let mut programs = 0;
    let mut deep = 0;
    for seed in 1..=6u64 {
        deep += run(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 500);
        programs += 500;
    }
    println!("\n  0052  {programs} programs across 6 seeds, {deep} of them deeper than 0\n");
    // A corpus of nothing but arithmetic would prove nothing about depth.
    assert!(deep > programs / 10, "only {deep} of {programs} reached past the surface");
}

/// The long one. `scripts/task fuzz`.
#[test]
#[ignore = "a soak; run with scripts/task fuzz"]
fn soak() {
    let rounds: usize =
        std::env::var("NETHER_FUZZ_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(10_000_000);
    let per_seed = rounds / 8;
    let mut deep = 0;
    for seed in 1..=8u64 {
        deep += run(0xD16_0000 ^ seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), per_seed);
    }
    println!(
        "\n  0052  soak: {} programs across 8 seeds, {deep} deeper than 0, no counterexample\n",
        per_seed * 8
    );
}
