//! The intermediate representation.
//!
//! A typed term language with a depth on every expression. It is what a source
//! file becomes once names are resolved and spelling is folded away, and it is
//! what burial reduces.
//!
//! The IR is the **checked** form. Nothing here checks anything — the rules
//! are in `spec/02-calculus.md` and the checker that applies them is its own
//! crate — but the shape assumes they have been applied: every [`Expr`] states
//! a type and a depth, and something had to work them out.
//!
//! Three things are true of this crate on purpose:
//!
//! - It has no dependencies. Not even the ledger: an IR is a fact about a
//!   source file, and becomes a fact about the world only when something
//!   writes it down.
//! - It holds no depth twice. Where a depth could be read off two fields, only
//!   one of them exists, because a claim stated twice is eventually stated two
//!   different ways.
//! - It keeps only the forms with their own typing rule. The folds are listed
//!   in [`ir`].

mod check;
mod depth;
mod ir;
mod prim;
mod ty;
mod unit;

pub use check::{Fault, FaultKind, check};
pub use depth::{Capability, Depth};
pub use ir::{
    BinOp, Block, Expr, ExprKind, FuncId, GlobalId, Literal, LocalId, Place, Proj, Rite, Span,
    Stmt, UnOp,
};
pub use prim::Prim;
pub use ty::{Refusal, Type};
pub use unit::{Asserted, Demand, Field, FuncDef, GlobalDef, LocalDef, StructDef, Unit};
