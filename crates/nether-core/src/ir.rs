//! Terms.
//!
//! Every expression carries its type and its depth, because a depth that has
//! to be looked up is a depth that can be looked up wrongly. The forms here
//! are what `spec/04-grammar.md` means rather than what it says: the IR keeps
//! the constructs that have their own typing rule in `spec/02-calculus.md` and
//! folds away the ones that are spelling.
//!
//! The folds, in full:
//!
//! - `&&` and `||` are [`ExprKind::Select`]. They short-circuit, so they are
//!   control flow rather than operators, and a block has a tail value.
//! - `if`/`else if`/`else` is [`ExprKind::Select`], nested. A missing `else`
//!   is `U0`.
//! - `while` and `for` are both [`ExprKind::Loop`], which carries the step
//!   expression a `for` needs so that `continue` still runs it.
//! - `x += e` is an [`ExprKind::Assign`] of `x + e`.
//! - Parentheses are grouping and leave nothing behind.
//! - A `typedef` is an alias and is replaced by the type it names.

use crate::depth::{Capability, Depth};
use crate::prim::Prim;
use crate::ty::{Refusal, Type};

/// Where in the source a thing was written.
///
/// Byte offsets into the one source a unit was lowered from; there is no
/// `#include`, so a unit has exactly one. The source's cairn is attached where
/// a span reaches the ledger (`spec/07-ledger.md` §7.3), and not before: a
/// path is a fact about one machine at one moment.
///
/// The default is the empty span at offset zero — nowhere in particular, which
/// is where an IR-only node such as the `U0` standing in for an absent `else`
/// comes from.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Span {
    /// Byte offset of the first byte.
    pub start: u32,
    /// Byte offset one past the last. Never before `start`.
    pub end: u32,
}

/// A binding inside a function: a parameter, a `let`, or a `for`'s counter.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct LocalId(pub u32);

/// A function declared at unit level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct FuncId(pub u32);

/// A `let` declared at unit level.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct GlobalId(pub u32);

/// A value that was written down rather than computed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Literal {
    /// `U0`. Not written in source — there is no literal for it — but the IR
    /// needs one wherever a construct has nothing to say.
    Unit,
    /// `true` or `false`.
    Bool(bool),
    /// A decimal, hexadecimal or binary integer literal.
    Int(i64),
    /// `b"…"`.
    Bytes(Vec<u8>),
    /// `"…"`, after escapes are resolved.
    Str(String),
    /// One of the six refusal codes, bound as prelude constants.
    Refusal(Refusal),
    /// A content address. Not written in source — there is no syntax for one
    /// and there should not be, since a cairn a programmer typed is a claim
    /// about a value rather than the value's own name. `seal` produces these,
    /// and burial writes one down when it can finish one.
    ///
    /// Held as its digest, so that the IR still knows nothing about how a
    /// cairn is computed. That is the ledger's business and this crate does
    /// not depend on it.
    Cairn([u8; 32]),
}

/// A binary operator. Short-circuiting `&&` and `||` are not here: they are
/// [`ExprKind::Select`].
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinOp {
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `&`
    BitAnd,
    /// `|`
    BitOr,
    /// `^`
    BitXor,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `==`, compared by cairn.
    Eq,
    /// `!=`
    Ne,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
}

/// A unary operator.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum UnOp {
    /// `-`
    Neg,
    /// `!`
    Not,
    /// `~`
    BitNot,
}

/// One of the four unary rites.
///
/// They are grouped because they are what `spec/02-calculus.md` calls the
/// escapes, and separated from [`UnOp`] because each has its own typing rule
/// rather than a type.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Rite {
    /// `seal e` — the cairn of `e`. `Cairn@0`, whatever `e` cost.
    Seal,
    /// `shade e` — `e`, opaque, at depth 0, remembering where it came from.
    Shade,
    /// `look s` — the value inside a shade. Legal only where you already hold
    /// the depth it came from. The Orpheus rule.
    Look,
    /// `opaque e` — `e`, never burned through.
    Opaque,
}

/// A step in a path to a field of a local aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Proj {
    /// `.name`, resolved to the field's position in declaration order.
    Field(u32),
    /// `[i]`
    Index(Box<Expr>),
}

/// The left-hand side of an assignment.
///
/// The root is always a local. There is no pointer type and no way to name a
/// location, so the only thing that can be written to is a local aggregate
/// that has not been read yet (`spec/05-types.md` §5.4) — and the IR says so
/// by construction rather than by check.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Place {
    /// The local at the root of the path.
    pub local: LocalId,
    /// The fields and indices walked from it. Empty is the local itself,
    /// which is only ever legal while the local is still being built.
    pub path: Vec<Proj>,
}

/// A sequence of statements and an optional tail value.
///
/// A block has no type or depth of its own: they are its tail's, or `U0@0`
/// when it has none. The enclosing [`Expr`] is where that is written down.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The statements, in order.
    pub stmts: Vec<Stmt>,
    /// The trailing expression with no semicolon, if there is one.
    pub tail: Option<Box<Expr>>,
    /// The whole block, braces included.
    pub span: Span,
}

/// A statement.
///
/// There are two, because everything else in `spec/04-grammar.md` §4.4 is an
/// expression once `if`, `while` and `for` have been folded into [`ExprKind`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// A binding. Immutable: there is no second one for the same local.
    Let {
        /// What is bound.
        local: LocalId,
        /// What it is bound to. Its depth is the binding's depth.
        value: Expr,
    },
    /// An expression evaluated for what it leaves behind.
    ///
    /// If its type is not `U0` it **deposits** that value into the trace,
    /// tagged with its span (`spec/04-grammar.md` §4.7). It is not printed and
    /// it is not discarded, and an implementation MUST NOT warn about it.
    Expr(Expr),
}

/// An expression, its type, its depth and where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// Which form it is.
    pub kind: ExprKind,
    /// `τ`.
    pub ty: Type,
    /// `d`. Never deeper than the ambient depth it was checked under, and
    /// never lowered by any evaluation step. `spec/01-strata.md` §1.2.
    pub depth: Depth,
    /// Where it was written.
    pub span: Span,
}

/// The forms an expression takes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// A literal. `[LIT]`, and therefore always depth 0.
    Literal(Literal),
    /// A local binding. `[VAR]`.
    Local(LocalId),
    /// A unit-level `let`.
    Global(GlobalId),
    /// A function declared in this unit, as a value.
    Func(FuncId),
    /// A prelude function, as a value.
    Prim(Prim),
    /// Application. `[APP]` joins three depths: the callee's latent depth, the
    /// depth of the callee *value*, and the arguments'. A function fetched
    /// over the network is deep before it is ever called.
    Call {
        /// What is applied. An expression, so its own depth counts.
        callee: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `-e`, `!e`, `~e`.
    Unary {
        /// Which one.
        op: UnOp,
        /// What it is applied to.
        operand: Box<Expr>,
    },
    /// An arithmetic, bitwise, shift or comparison operator. `[PRIM]`.
    Binary {
        /// Which one.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// `e.name`, resolved to the field's position in declaration order.
    Field {
        /// The aggregate.
        base: Box<Expr>,
        /// Which field.
        index: u32,
    },
    /// `e[i]`.
    Index {
        /// The array.
        base: Box<Expr>,
        /// Which element.
        index: Box<Expr>,
    },
    /// A write to a field of a local aggregate that has not been read yet.
    Assign {
        /// Where it goes.
        place: Place,
        /// What goes there.
        value: Box<Expr>,
    },
    /// `{ … }`.
    Block(Block),
    /// The one branching form: `if`, `else if`, `&&` and `||` are all this.
    Select {
        /// What is tested.
        cond: Box<Expr>,
        /// Taken when it holds.
        then: Box<Expr>,
        /// Taken when it does not. `U0` where the source wrote no `else`.
        otherwise: Box<Expr>,
    },
    /// The one looping form.
    ///
    /// `while (c) b` is this with no step; `for (i; c; s) b` is this with `s`
    /// as the step and `i` hoisted into the enclosing block. The step is a
    /// field rather than the last statement of the body because `continue`
    /// must still run it, and a `for` whose step a `continue` can skip is the
    /// oldest bug in this shape of desugaring.
    Loop {
        /// The body.
        body: Box<Expr>,
        /// Run at the end of every iteration, `continue` included.
        step: Option<Box<Expr>>,
    },
    /// `break`.
    Break,
    /// `continue`.
    Continue,
    /// `return` with or without a value.
    Return(Option<Box<Expr>>),
    /// `descend κ { … }`. The only rule that raises the ambient depth, and it
    /// raises it only inside its own body. `[DESCEND]`.
    Descend {
        /// `κ`.
        capability: Capability,
        /// Checked at `max(δ, s(κ))`. Only its value leaves.
        body: Box<Expr>,
    },
    /// `seal`, `shade`, `look` or `opaque`.
    Rite {
        /// Which one.
        rite: Rite,
        /// What it is applied to.
        operand: Box<Expr>,
    },
    /// `sizeof(T)`.
    SizeOf(Type),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pure(kind: ExprKind, ty: Type) -> Expr {
        Expr { kind, ty, depth: Depth::PURE, span: Span::default() }
    }

    #[test]
    fn a_place_can_only_be_rooted_at_a_local() {
        // Not a test of behaviour: a test that the shape holds, which is the
        // only way to say "there is no pointer type" in a data model.
        let p = Place { local: LocalId(0), path: vec![Proj::Field(1)] };
        assert_eq!(p.local, LocalId(0));
    }

    #[test]
    fn short_circuit_is_a_select_and_not_an_operator() {
        // `a && b` is `if (a) { b } else { false }`.
        let a = pure(ExprKind::Local(LocalId(0)), Type::Bool);
        let b = pure(ExprKind::Local(LocalId(1)), Type::Bool);
        let f = pure(ExprKind::Literal(Literal::Bool(false)), Type::Bool);
        let and = pure(
            ExprKind::Select { cond: Box::new(a), then: Box::new(b), otherwise: Box::new(f) },
            Type::Bool,
        );
        assert!(matches!(and.kind, ExprKind::Select { .. }));
    }

    #[test]
    fn a_loop_carries_the_step_a_for_needs() {
        let body = pure(ExprKind::Continue, Type::Unit);
        let step = pure(ExprKind::Literal(Literal::Unit), Type::Unit);
        let l =
            pure(ExprKind::Loop { body: Box::new(body), step: Some(Box::new(step)) }, Type::Unit);
        let ExprKind::Loop { step, .. } = l.kind else { panic!("not a loop") };
        assert!(step.is_some(), "a `continue` that skips the step is the bug this prevents");
    }
}
