//! The shape of a source file.
//!
//! `spec/04-grammar.md`, one type per production and nothing folded. The
//! folding happens in lowering, where there is a checker to say what the folds
//! mean; a parser that folds is a parser whose errors point at a program
//! nobody wrote.

use nether_core::{Rite, Span, UnOp};

/// An identifier, and where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Name {
    /// The identifier, NFC-normalised by the lexer.
    pub text: String,
    /// Where it was written.
    pub span: Span,
}

/// `unit := { item }`
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unit {
    /// The items, in source order.
    pub items: Vec<Item>,
}

/// `item := struct_decl | typedef_decl | func_decl | let_decl | demand_stmt`
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Item {
    /// `struct identifier { field… };`
    Struct {
        /// The type's name.
        name: Name,
        /// Its fields, in declaration order.
        fields: Vec<Field>,
        /// The whole declaration.
        span: Span,
    },
    /// `typedef type identifier;`
    Typedef {
        /// What it names.
        ty: Type,
        /// The name.
        name: Name,
        /// The whole declaration.
        span: Span,
    },
    /// `type identifier ( params ) [@d] block`
    Func(Func),
    /// `type identifier = expr;` at unit level.
    Let(Let),
    /// `demand expr;`
    Demand {
        /// What is demanded.
        value: Expr,
        /// The whole statement.
        span: Span,
    },
}

/// `field := type identifier ";"`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Field {
    /// Its type.
    pub ty: Type,
    /// Its name.
    pub name: Name,
    /// The whole field.
    pub span: Span,
}

/// `func_decl := type identifier "(" [ params ] ")" [ latent ] block`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Func {
    /// What it returns.
    pub ret: Type,
    /// Its name.
    pub name: Name,
    /// Its parameters, in order.
    pub params: Vec<Field>,
    /// The `@d` after the signature, if it was written.
    pub latent: Option<u8>,
    /// The body.
    pub body: Block,
    /// The whole declaration.
    pub span: Span,
}

/// `let_decl := type identifier "=" expr ";"`, at unit level or in a block.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Let {
    /// Its type.
    pub ty: Type,
    /// Its name.
    pub name: Name,
    /// What it is bound to.
    pub value: Expr,
    /// The whole declaration.
    pub span: Span,
}

/// `type := type_atom { "[" [ int_literal ] "]" } [ "@" digit ]`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Type {
    /// The name it starts with.
    pub name: Name,
    /// `<…>`, when it has them.
    pub args: Vec<Type>,
    /// One entry per `[…]`, outermost first. `None` is a slice.
    pub arrays: Vec<Option<i64>>,
    /// The `@d`, if it was written. Always a checked assertion, never a
    /// coercion. `spec/05-types.md` §5.6.
    pub depth: Option<u8>,
    /// The whole type.
    pub span: Span,
}

/// `block := "{" { stmt } [ expr ] "}"`
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Block {
    /// The statements, in order.
    pub stmts: Vec<Stmt>,
    /// The trailing expression with no semicolon, if there is one.
    pub tail: Option<Box<Expr>>,
    /// The whole block, braces included.
    pub span: Span,
}

/// `stmt`, as §4.4 lists them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Stmt {
    /// A binding.
    Let(Let),
    /// `expr ";"`. If its value is not `U0` it deposits. §4.7.
    Expr(Expr),
    /// `if (…) { … } [else …]`
    If {
        /// What is tested.
        cond: Expr,
        /// Taken when it holds.
        then: Block,
        /// `else`, which is a block or another `if`.
        otherwise: Option<Box<Else>>,
        /// The whole statement.
        span: Span,
    },
    /// `while (…) { … }`
    While {
        /// What is tested, before every iteration.
        cond: Expr,
        /// The body.
        body: Block,
        /// The whole statement.
        span: Span,
    },
    /// `for (init; cond; step) { … }`
    For {
        /// The binding or expression before the first semicolon.
        init: Option<Box<Stmt>>,
        /// What is tested, before every iteration.
        cond: Option<Expr>,
        /// What runs at the end of every iteration.
        step: Option<Expr>,
        /// The body.
        body: Block,
        /// The whole statement.
        span: Span,
    },
    /// `return [expr];`
    Return {
        /// What is returned, if anything.
        value: Option<Expr>,
        /// The whole statement.
        span: Span,
    },
    /// `break;`
    Break(Span),
    /// `continue;`
    Continue(Span),
    /// A nested block.
    Block(Block),
}

/// What follows an `else`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Else {
    /// `else { … }`
    Block(Block),
    /// `else if (…) …`. Boxed because an `if` is much larger than a block and
    /// every `else` would otherwise be the size of the larger one.
    If(Box<Stmt>),
}

/// An expression, and where it was written.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Expr {
    /// Which form it is.
    pub kind: ExprKind,
    /// Where it was written.
    pub span: Span,
}

/// The forms §4.5 gives an expression.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ExprKind {
    /// An integer literal.
    Int(i64),
    /// A string literal.
    Str(String),
    /// A `b"…"` literal.
    Bytes(Vec<u8>),
    /// `true` or `false`.
    Bool(bool),
    /// An identifier. What it names is lowering's question.
    Name(Name),
    /// `descend κ { … }`
    Descend {
        /// `κ`.
        capability: Name,
        /// The body.
        body: Block,
    },
    /// `{ … }` as an expression.
    Block(Block),
    /// `-e`, `!e`, `~e`.
    Unary {
        /// Which one.
        op: UnOp,
        /// What it applies to.
        operand: Box<Expr>,
    },
    /// `seal e`, `shade e`, `look e`, `opaque e`.
    Rite {
        /// Which one.
        rite: Rite,
        /// What it applies to.
        operand: Box<Expr>,
    },
    /// A binary operator, at one of §4.6's levels.
    Binary {
        /// Which one.
        op: BinOp,
        /// Left operand.
        lhs: Box<Expr>,
        /// Right operand.
        rhs: Box<Expr>,
    },
    /// `place = value`, or one of the compound forms.
    Assign {
        /// `Some` for a compound assignment: `x += e` carries `Add`.
        op: Option<BinOp>,
        /// What is written to.
        place: Box<Expr>,
        /// What is written.
        value: Box<Expr>,
    },
    /// `f(args)`
    Call {
        /// What is applied.
        callee: Box<Expr>,
        /// The arguments, in order.
        args: Vec<Expr>,
    },
    /// `e[i]`
    Index {
        /// The array.
        base: Box<Expr>,
        /// Which element.
        index: Box<Expr>,
    },
    /// `e.name`
    Field {
        /// The aggregate.
        base: Box<Expr>,
        /// Which field.
        name: Name,
    },
}

/// A binary operator, including the two that short-circuit.
///
/// The IR folds `&&` and `||` into a branch because they are control flow.
/// Here they are operators, because that is what §4.5 calls them and this is
/// the file that answers to §4.5.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum BinOp {
    /// `*`
    Mul,
    /// `/`
    Div,
    /// `%`
    Rem,
    /// `+`
    Add,
    /// `-`
    Sub,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `<`
    Lt,
    /// `<=`
    Le,
    /// `>`
    Gt,
    /// `>=`
    Ge,
    /// `==`
    Eq,
    /// `!=`
    Ne,
    /// `&`
    BitAnd,
    /// `^`
    BitXor,
    /// `|`
    BitOr,
    /// `&&`
    And,
    /// `||`
    Or,
}
