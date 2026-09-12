//! Units: what a source file lowers to.
//!
//! `spec/04-grammar.md` §4.1. There is no entry point. A unit is a set of
//! declarations plus the demands that give some of them a reason to be
//! evaluated, and nothing is evaluated except what a demand transitively
//! requires (`spec/06-evaluation.md` §6.2).

use crate::depth::Depth;
use crate::ir::{Block, Expr, FuncId, GlobalId, LocalId, Span};
use crate::ty::Type;

/// A compilation unit.
///
/// Declarations are held in the order they were written, and [`FuncId`],
/// [`GlobalId`] and [`LocalId`] are positions in these vectors.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unit {
    /// Struct declarations. Looked up by name: structs are nominal.
    pub structs: Vec<StructDef>,
    /// Function declarations.
    pub funcs: Vec<FuncDef>,
    /// Unit-level `let` declarations.
    pub globals: Vec<GlobalDef>,
    /// The demands, in source order, which is the order they are evaluated in.
    pub demands: Vec<Demand>,
}

impl Unit {
    /// The function that id names.
    #[must_use]
    pub fn func(&self, id: FuncId) -> Option<&FuncDef> {
        self.funcs.get(id.0 as usize)
    }

    /// The unit-level binding that id names.
    #[must_use]
    pub fn global(&self, id: GlobalId) -> Option<&GlobalDef> {
        self.globals.get(id.0 as usize)
    }

    /// The struct of that name.
    #[must_use]
    pub fn struct_def(&self, name: &str) -> Option<&StructDef> {
        self.structs.iter().find(|s| s.name == name)
    }
}

/// A depth the programmer wrote.
///
/// An assertion, checked against what inference produced, never a coercion:
/// an annotation that disagrees is an error naming both depths
/// (`spec/05-types.md` §5.6). `None` is the ordinary case.
pub type Asserted = Option<Depth>;

/// `struct Name { … };`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StructDef {
    /// The type's name. Part of the value: nominal typing means the name is
    /// encoded with it (`spec/07-ledger.md` §7.1 rule 3).
    pub name: String,
    /// Fields, in declaration order, which is also encoding order.
    pub fields: Vec<Field>,
    /// The declaration.
    pub span: Span,
}

/// A field of a struct.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Its name.
    pub name: String,
    /// Its type. Always written: inference does not cross a declaration.
    pub ty: Type,
    /// A depth the programmer wrote after the type.
    pub asserted: Asserted,
    /// The field.
    pub span: Span,
}

/// `T name(params) [@d] { … }`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FuncDef {
    /// Its name.
    pub name: String,
    /// Its parameters, as positions in [`FuncDef::locals`]. By convention they
    /// are the first ones.
    pub params: Vec<LocalId>,
    /// What it returns.
    pub ret: Type,
    /// `dƒ`: the deepest stratum applying it reaches. Inferred from the body,
    /// which is where [ABS] says latency comes from. Building a function that
    /// will touch the disk does not touch the disk.
    pub latent: Depth,
    /// The latent depth the programmer wrote after the signature.
    pub asserted: Asserted,
    /// Every binding in the body, parameters first.
    pub locals: Vec<LocalDef>,
    /// The body.
    pub body: Block,
    /// The declaration.
    pub span: Span,
}

impl FuncDef {
    /// The binding that id names.
    #[must_use]
    pub fn local(&self, id: LocalId) -> Option<&LocalDef> {
        self.locals.get(id.0 as usize)
    }
}

/// A binding inside a function body.
///
/// It has no depth of its own. A parameter's is whatever the call site
/// supplies and a `let`'s is its value's, and both are already written on the
/// [`Expr`] that provides them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LocalDef {
    /// Its name, kept for diagnostics. Resolution goes through [`LocalId`].
    pub name: String,
    /// Its type.
    pub ty: Type,
    /// A depth the programmer wrote after the type.
    pub asserted: Asserted,
    /// Where it was bound.
    pub span: Span,
}

/// `T name = expr;` at unit level.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GlobalDef {
    /// Its name.
    pub name: String,
    /// Its type. Always written at unit level.
    pub ty: Type,
    /// A depth the programmer wrote after the type.
    pub asserted: Asserted,
    /// What it is bound to. Evaluated only if a demand reaches it.
    pub value: Expr,
    /// The declaration.
    pub span: Span,
}

/// `demand expr;`
///
/// The only reason anything is evaluated. `[DEMAND]`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Demand {
    /// What is demanded.
    pub value: Expr,
    /// The statement.
    pub span: Span,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ir::{Expr, ExprKind, Literal};

    fn unit_expr() -> Expr {
        Expr {
            kind: ExprKind::Literal(Literal::Unit),
            ty: Type::Unit,
            depth: Depth::PURE,
            span: Span::default(),
        }
    }

    #[test]
    fn an_empty_unit_demands_nothing_and_is_legal() {
        let u = Unit::default();
        assert!(u.demands.is_empty());
        assert_eq!(u.func(FuncId(0)), None);
        assert_eq!(u.global(GlobalId(0)), None);
        assert_eq!(u.struct_def("Header"), None);
    }

    #[test]
    fn structs_are_found_by_name_because_they_are_nominal() {
        let u = Unit {
            structs: vec![
                StructDef { name: "Header".into(), fields: Vec::new(), span: Span::default() },
                StructDef { name: "Footer".into(), fields: Vec::new(), span: Span::default() },
            ],
            ..Unit::default()
        };
        assert_eq!(u.struct_def("Footer").map(|s| s.name.as_str()), Some("Footer"));
        assert_eq!(u.struct_def("header"), None);
    }

    #[test]
    fn demands_keep_the_order_they_were_written_in() {
        let u = Unit {
            demands: vec![
                Demand { value: unit_expr(), span: Span { start: 10, end: 20 } },
                Demand { value: unit_expr(), span: Span { start: 30, end: 40 } },
            ],
            ..Unit::default()
        };
        let starts: Vec<u32> = u.demands.iter().map(|d| d.span.start).collect();
        assert_eq!(starts, vec![10, 30]);
    }
}
