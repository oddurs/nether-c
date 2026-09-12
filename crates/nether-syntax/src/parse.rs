//! Recursive descent, written by hand from the EBNF.
//!
//! The technique is the thesis. A generated parser is a parser nobody reads,
//! and §04 is short enough that reading it beside this file is the point.
//!
//! One mistake produces one message. On an error the parser records it and
//! skips to the next place a new thing can start — a `;` at the level it is
//! at, or the `}` that closes the block — so a missing semicolon on line four
//! does not produce a complaint about every line after it.

use nether_core::{Rite, Span, UnOp};

use crate::ast::{
    BinOp, Block, Else, Expr, ExprKind, Field, Func, Item, Let, Name, Stmt, Type, Unit,
};
use crate::lex::{Fault, FaultKind, lex};
use crate::token::{Keyword, Punct, Token, TokenKind};

/// §4.6's table, which is the whole of precedence and associativity.
///
/// Level 1 is postfix and level 2 is unary, both handled in their own
/// functions; 13 is assignment, which is right-associative and handled after
/// the climb. What is left is levels 3 to 12, every one of them
/// left-associative.
pub(crate) const LEVELS: [(Punct, u8, BinOp); 18] = [
    (Punct::Star, 3, BinOp::Mul),
    (Punct::Slash, 3, BinOp::Div),
    (Punct::Percent, 3, BinOp::Rem),
    (Punct::Plus, 4, BinOp::Add),
    (Punct::Minus, 4, BinOp::Sub),
    (Punct::Shl, 5, BinOp::Shl),
    (Punct::Shr, 5, BinOp::Shr),
    (Punct::Lt, 6, BinOp::Lt),
    (Punct::Le, 6, BinOp::Le),
    (Punct::Gt, 6, BinOp::Gt),
    (Punct::Ge, 6, BinOp::Ge),
    (Punct::EqEq, 7, BinOp::Eq),
    (Punct::BangEq, 7, BinOp::Ne),
    (Punct::Amp, 8, BinOp::BitAnd),
    (Punct::Caret, 9, BinOp::BitXor),
    (Punct::Pipe, 10, BinOp::BitOr),
    (Punct::AmpAmp, 11, BinOp::And),
    (Punct::PipePipe, 12, BinOp::Or),
];

/// The compound assignments, and what each one is an assignment of.
const COMPOUND: [(Punct, BinOp); 10] = [
    (Punct::PlusEq, BinOp::Add),
    (Punct::MinusEq, BinOp::Sub),
    (Punct::StarEq, BinOp::Mul),
    (Punct::SlashEq, BinOp::Div),
    (Punct::PercentEq, BinOp::Rem),
    (Punct::AmpEq, BinOp::BitAnd),
    (Punct::PipeEq, BinOp::BitOr),
    (Punct::CaretEq, BinOp::BitXor),
    (Punct::ShlEq, BinOp::Shl),
    (Punct::ShrEq, BinOp::Shr),
];

const LOWEST: u8 = 12;

/// §4.6's binary operators, each with the level it is at.
///
/// Handed out so the proof can compare them against the table in §04 rather
/// than against a copy of it.
#[must_use]
pub fn levels() -> Vec<(Punct, u8, BinOp)> {
    LEVELS.to_vec()
}

/// Lex and parse a source file.
///
/// # Errors
///
/// Every mistake it found, in source order. A lexical fault stops it, because
/// a token stream that is a guess produces syntax errors that are also
/// guesses.
pub fn parse(source: &[u8]) -> Result<Unit, Vec<Fault>> {
    let tokens = lex(source).map_err(|f| vec![f])?;
    let mut p = Parser { tokens: &tokens, at: 0, faults: Vec::new(), end: end_of(source), deep: 0 };
    let unit = p.unit();
    if p.faults.is_empty() { Ok(unit) } else { Err(p.faults) }
}

fn end_of(source: &[u8]) -> Span {
    // Exact: `lex` has already refused anything a span could not address.
    let n = u32::try_from(source.len()).unwrap_or(u32::MAX);
    Span { start: n, end: n }
}

/// How deeply an expression or a type may nest.
///
/// The grammar is recursive and this parser is not. `spec/06-evaluation.md`
/// §6.4 requires an implementation to state a limit like this one and to
/// report reaching it rather than crash into it, and before this one existed a
/// hundred parentheses aborted the process.
///
/// Sixty-four is what a level of nesting costs here, not what a program could
/// reasonably want. §4.6's ladder is ten frames deep, so every parenthesis is
/// about twenty kilobytes of host stack and sixty-four of them is most of a
/// small thread's. Raising this means making a level cheaper first — walking
/// the ladder by precedence climbing rather than by recursion would cost one
/// frame per level instead of ten.
///
/// Everything downstream inherits it. `lower` and `check` walk what this
/// produced, so neither states a bound of its own.
pub const MAX_NESTING: u32 = 64;

struct Parser<'a> {
    tokens: &'a [Token],
    at: usize,
    faults: Vec<Fault>,
    /// Where to point when the source simply stopped.
    end: Span,
    /// How far in the current expression or type is. See [`MAX_NESTING`].
    deep: u32,
}

/// The parser gave up on this construct. It has already recorded why.
struct Given;

type Parsed<T> = Result<T, Given>;

impl Parser<'_> {
    // ── the token stream ────────────────────────────────────────────────────

    fn peek(&self) -> Option<&TokenKind> {
        self.tokens.get(self.at).map(|t| &t.kind)
    }

    fn span(&self) -> Span {
        self.tokens.get(self.at).map_or(self.end, |t| t.span)
    }

    fn since(&self, from: Span) -> Span {
        let end = self.tokens[..self.at].last().map_or(from.end, |t| t.span.end);
        Span { start: from.start, end }
    }

    fn bump(&mut self) -> Option<&TokenKind> {
        self.at += 1;
        self.tokens.get(self.at - 1).map(|t| &t.kind)
    }

    fn at_punct(&self, p: Punct) -> bool {
        matches!(self.peek(), Some(TokenKind::Punct(q)) if *q == p)
    }

    fn at_keyword(&self, k: Keyword) -> bool {
        matches!(self.peek(), Some(TokenKind::Keyword(j)) if *j == k)
    }

    fn eat(&mut self, p: Punct) -> bool {
        let here = self.at_punct(p);
        if here {
            self.at += 1;
        }
        here
    }

    fn eat_keyword(&mut self, k: Keyword) -> bool {
        let here = self.at_keyword(k);
        if here {
            self.at += 1;
        }
        here
    }

    fn expect(&mut self, p: Punct, what: &'static str) -> Parsed<()> {
        if self.eat(p) { Ok(()) } else { Err(self.complain(what)) }
    }

    /// A terminator that is missing is reported and then assumed.
    ///
    /// Giving up here would throw away an item that is otherwise complete and
    /// resynchronise past the next one, so a single missing semicolon would
    /// cost two things: a message, and the declaration after it.
    fn expect_semi(&mut self, what: &'static str) {
        if !self.eat(Punct::Semi) {
            self.complain(what);
        }
    }

    fn complain(&mut self, what: &'static str) -> Given {
        let span = self.span();
        self.faults.push(Fault { span, kind: FaultKind::Expected(what) });
        Given
    }

    fn name(&mut self, what: &'static str) -> Parsed<Name> {
        let span = self.span();
        match self.peek() {
            Some(TokenKind::Ident(text)) => {
                let text = text.clone();
                self.at += 1;
                Ok(Name { text, span })
            }
            _ => Err(self.complain(what)),
        }
    }

    // ── recovery ────────────────────────────────────────────────────────────

    /// Skip to where a new item could start: past the next `;` at brace depth
    /// zero, or to the next keyword that opens one.
    fn resync_item(&mut self) {
        let mut braces = 0i32;
        while let Some(kind) = self.peek() {
            match kind {
                TokenKind::Punct(Punct::LBrace) => braces += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    braces -= 1;
                    if braces <= 0 {
                        self.at += 1;
                        return;
                    }
                }
                TokenKind::Punct(Punct::Semi) if braces == 0 => {
                    self.at += 1;
                    return;
                }
                TokenKind::Keyword(Keyword::Struct | Keyword::Typedef | Keyword::Demand)
                    if braces == 0 =>
                {
                    return;
                }
                _ => {}
            }
            self.at += 1;
        }
    }

    /// Skip to the end of the statement: past the next `;`, or up to the `}`
    /// that closes the block this statement is in.
    fn resync_stmt(&mut self) {
        let mut braces = 0i32;
        while let Some(kind) = self.peek() {
            match kind {
                TokenKind::Punct(Punct::LBrace) => braces += 1,
                TokenKind::Punct(Punct::RBrace) => {
                    if braces == 0 {
                        return;
                    }
                    braces -= 1;
                }
                TokenKind::Punct(Punct::Semi) if braces == 0 => {
                    self.at += 1;
                    return;
                }
                _ => {}
            }
            self.at += 1;
        }
    }

    // ── §4.1 and §4.2 ───────────────────────────────────────────────────────

    fn unit(&mut self) -> Unit {
        let mut items = Vec::new();
        while self.peek().is_some() {
            let before = self.at;
            match self.item() {
                Ok(item) => items.push(item),
                Err(Given) => self.resync_item(),
            }
            // Recovery that does not move is a loop, and a parser that hangs
            // on a malformed file is worse than one that says the wrong thing.
            if self.at == before {
                self.at += 1;
            }
        }
        Unit { items }
    }

    fn item(&mut self) -> Parsed<Item> {
        let from = self.span();
        if self.eat_keyword(Keyword::Struct) {
            return self.struct_decl(from);
        }
        if self.eat_keyword(Keyword::Typedef) {
            let ty = self.ty()?;
            let name = self.name("a name for the type")?;
            self.expect_semi("`;` after a typedef");
            return Ok(Item::Typedef { ty, name, span: self.since(from) });
        }
        if self.eat_keyword(Keyword::Demand) {
            let value = self.expr()?;
            self.expect_semi("`;` after a demand");
            return Ok(Item::Demand { value, span: self.since(from) });
        }

        // What is left starts `type identifier`, and the token after the name
        // says which it is.
        let ty = self.ty()?;
        let name = self.name("a name")?;
        if self.at_punct(Punct::LParen) {
            return self.func(ty, name, from);
        }
        self.expect(Punct::Eq, "`(` for a function, or `=` for a binding")?;
        let value = self.expr()?;
        self.expect_semi("`;` after a binding");
        Ok(Item::Let(Let { ty, name, value, span: self.since(from) }))
    }

    fn struct_decl(&mut self, from: Span) -> Parsed<Item> {
        let name = self.name("a name for the struct")?;
        self.expect(Punct::LBrace, "`{` to open the fields")?;
        let mut fields = Vec::new();
        while !self.at_punct(Punct::RBrace) && self.peek().is_some() {
            let at = self.span();
            let ty = self.ty()?;
            let field = self.name("a name for the field")?;
            self.expect_semi("`;` after a field");
            fields.push(Field { ty, name: field, span: self.since(at) });
        }
        self.expect(Punct::RBrace, "`}` to close the fields")?;
        self.expect_semi("`;` after a struct");
        Ok(Item::Struct { name, fields, span: self.since(from) })
    }

    fn func(&mut self, ret: Type, name: Name, from: Span) -> Parsed<Item> {
        self.expect(Punct::LParen, "`(`")?;
        let mut params = Vec::new();
        while !self.at_punct(Punct::RParen) {
            let at = self.span();
            let ty = self.ty()?;
            let pname = self.name("a name for the parameter")?;
            params.push(Field { ty, name: pname, span: self.since(at) });
            if !self.eat(Punct::Comma) {
                break;
            }
        }
        self.expect(Punct::RParen, "`)` to close the parameters")?;
        // §3.5: a latent depth may be written with a space after the `@`, so
        // it arrives either as one token or as two.
        let latent = match self.peek() {
            Some(TokenKind::Depth(d)) => {
                let d = *d;
                self.at += 1;
                Some(d)
            }
            Some(TokenKind::Punct(Punct::At)) => {
                self.at += 1;
                Some(self.stratum()?)
            }
            _ => None,
        };
        let body = self.block()?;
        Ok(Item::Func(Func { ret, name, params, latent, body, span: self.since(from) }))
    }

    fn stratum(&mut self) -> Parsed<u8> {
        let span = self.span();
        let Some(TokenKind::Int(n)) = self.peek() else {
            return Err(self.complain("a depth, 0 to 8"));
        };
        let n = *n;
        self.at += 1;
        u8::try_from(n).ok().filter(|d| *d <= 8).ok_or_else(|| {
            self.faults.push(Fault { span, kind: FaultKind::NotAStratum });
            Given
        })
    }

    // ── §4.3 ────────────────────────────────────────────────────────────────

    fn ty(&mut self) -> Parsed<Type> {
        self.deeper()?;
        let out = self.ty_inner();
        self.deep -= 1;
        out
    }

    fn ty_inner(&mut self) -> Parsed<Type> {
        let from = self.span();
        let name = self.name("a type")?;
        let mut args = Vec::new();
        if self.eat(Punct::Lt) {
            loop {
                args.push(self.ty()?);
                if !self.eat(Punct::Comma) {
                    break;
                }
            }
            self.expect(Punct::Gt, "`>` to close the type arguments")?;
        }
        let mut arrays = Vec::new();
        while self.eat(Punct::LBracket) {
            let len = match self.peek() {
                Some(TokenKind::Int(n)) => {
                    let n = *n;
                    self.at += 1;
                    Some(n)
                }
                _ => None,
            };
            self.expect(Punct::RBracket, "`]` to close the length")?;
            arrays.push(len);
        }
        // §3.5: on a type there is no space, so this is one token or nothing.
        let depth = match self.peek() {
            Some(TokenKind::Depth(d)) => {
                let d = *d;
                self.at += 1;
                Some(d)
            }
            _ => None,
        };
        Ok(Type { name, args, arrays, depth, span: self.since(from) })
    }

    /// A type followed by a name, or nothing. Used where §04 has two
    /// productions that start the same way and the token after decides.
    fn try_binding(&mut self) -> Option<(Type, Name)> {
        let save = (self.at, self.faults.len());
        let out = self.ty().ok().zip(self.name("a name").ok());
        // Nothing that was tried speculatively gets to complain about it.
        self.faults.truncate(save.1);
        if out.is_none() {
            self.at = save.0;
        }
        out
    }

    // ── §4.4 ────────────────────────────────────────────────────────────────

    fn block(&mut self) -> Parsed<Block> {
        let from = self.span();
        self.expect(Punct::LBrace, "`{`")?;
        let mut stmts = Vec::new();
        let mut tail = None;
        while !self.at_punct(Punct::RBrace) && self.peek().is_some() {
            let before = self.at;
            match self.stmt_or_tail() {
                Ok(Line::Stmt(s)) => stmts.push(s),
                Ok(Line::Tail(e)) => {
                    tail = Some(Box::new(e));
                    break;
                }
                Err(Given) => self.resync_stmt(),
            }
            if self.at == before {
                self.at += 1;
            }
        }
        self.expect(Punct::RBrace, "`}` to close the block")?;
        Ok(Block { stmts, tail, span: self.since(from) })
    }

    fn stmt_or_tail(&mut self) -> Parsed<Line> {
        let from = self.span();
        if self.at_punct(Punct::LBrace) {
            return Ok(Line::Stmt(Stmt::Block(self.block()?)));
        }
        if self.at_keyword(Keyword::If) {
            return Ok(Line::Stmt(self.if_stmt()?));
        }
        if self.eat_keyword(Keyword::While) {
            self.expect(Punct::LParen, "`(` after `while`")?;
            let cond = self.expr()?;
            self.expect(Punct::RParen, "`)` after the condition")?;
            let body = self.block()?;
            return Ok(Line::Stmt(Stmt::While { cond, body, span: self.since(from) }));
        }
        if self.at_keyword(Keyword::For) {
            return Ok(Line::Stmt(self.for_stmt(from)?));
        }
        if self.eat_keyword(Keyword::Return) {
            let value = if self.at_punct(Punct::Semi) { None } else { Some(self.expr()?) };
            self.expect_semi("`;` after a return");
            return Ok(Line::Stmt(Stmt::Return { value, span: self.since(from) }));
        }
        if self.eat_keyword(Keyword::Break) {
            self.expect_semi("`;` after `break`");
            return Ok(Line::Stmt(Stmt::Break(self.since(from))));
        }
        if self.eat_keyword(Keyword::Continue) {
            self.expect_semi("`;` after `continue`");
            return Ok(Line::Stmt(Stmt::Continue(self.since(from))));
        }
        if let Some(binding) = self.let_stmt(from)? {
            return Ok(Line::Stmt(binding));
        }

        // An expression. With a `;` it is a statement; without one, and with
        // the block about to close, it is the block's tail value.
        let value = self.expr()?;
        if self.eat(Punct::Semi) {
            Ok(Line::Stmt(Stmt::Expr(value)))
        } else {
            Ok(Line::Tail(value))
        }
    }

    fn let_stmt(&mut self, from: Span) -> Parsed<Option<Stmt>> {
        let Some((ty, name)) = self.try_binding() else { return Ok(None) };
        if !self.eat(Punct::Eq) {
            // `Header h;` is not a production §04 has. Saying so where the
            // `=` should be beats saying `expected an expression` at `h`.
            return Err(self.complain("`=` after a binding"));
        }
        let value = self.expr()?;
        self.expect_semi("`;` after a binding");
        Ok(Some(Stmt::Let(Let { ty, name, value, span: self.since(from) })))
    }

    fn if_stmt(&mut self) -> Parsed<Stmt> {
        let from = self.span();
        self.at += 1;
        self.expect(Punct::LParen, "`(` after `if`")?;
        let cond = self.expr()?;
        self.expect(Punct::RParen, "`)` after the condition")?;
        let then = self.block()?;
        let otherwise = if self.eat_keyword(Keyword::Else) {
            Some(Box::new(if self.at_keyword(Keyword::If) {
                Else::If(Box::new(self.if_stmt()?))
            } else {
                Else::Block(self.block()?)
            }))
        } else {
            None
        };
        Ok(Stmt::If { cond, then, otherwise, span: self.since(from) })
    }

    fn for_stmt(&mut self, from: Span) -> Parsed<Stmt> {
        self.at += 1;
        self.expect(Punct::LParen, "`(` after `for`")?;
        let init = if self.eat(Punct::Semi) {
            None
        } else if let Some(binding) = self.let_stmt(self.span())? {
            Some(Box::new(binding))
        } else {
            let e = self.expr()?;
            self.expect_semi("`;` after the first clause");
            Some(Box::new(Stmt::Expr(e)))
        };
        let cond = if self.at_punct(Punct::Semi) { None } else { Some(self.expr()?) };
        self.expect_semi("`;` after the condition");
        let step = if self.at_punct(Punct::RParen) { None } else { Some(self.expr()?) };
        self.expect(Punct::RParen, "`)` to close the clauses")?;
        let body = self.block()?;
        Ok(Stmt::For { init, cond, step, body, span: self.since(from) })
    }

    // ── §4.5 and §4.6 ───────────────────────────────────────────────────────

    fn expr(&mut self) -> Parsed<Expr> {
        let from = self.span();
        let lhs = self.binary(LOWEST)?;
        if self.eat(Punct::Eq) {
            let value = self.expr()?;
            let kind = ExprKind::Assign { op: None, place: Box::new(lhs), value: Box::new(value) };
            return Ok(Expr { kind, span: self.since(from) });
        }
        for (punct, op) in COMPOUND {
            if self.eat(punct) {
                let value = self.expr()?;
                let kind =
                    ExprKind::Assign { op: Some(op), place: Box::new(lhs), value: Box::new(value) };
                return Ok(Expr { kind, span: self.since(from) });
            }
        }
        Ok(lhs)
    }

    /// Levels 3 to 12 of §4.6, all left-associative.
    fn binary(&mut self, level: u8) -> Parsed<Expr> {
        if level < 3 {
            return self.unary();
        }
        let from = self.span();
        let mut lhs = self.binary(level - 1)?;
        while let Some(op) = self.operator_at(level) {
            let rhs = self.binary(level - 1)?;
            let kind = ExprKind::Binary { op, lhs: Box::new(lhs), rhs: Box::new(rhs) };
            lhs = Expr { kind, span: self.since(from) };
        }
        Ok(lhs)
    }

    fn operator_at(&mut self, level: u8) -> Option<BinOp> {
        let TokenKind::Punct(p) = self.peek()? else { return None };
        let (_, _, op) = LEVELS.iter().find(|(q, l, _)| q == p && *l == level)?;
        self.at += 1;
        Some(*op)
    }

    /// Level 2 of §4.6, and the one place every level of nesting passes
    /// through: the ladder above, a parenthesis, a block, an index, an
    /// argument and a prefix operator all arrive here. So this is where the
    /// depth is counted.
    fn unary(&mut self) -> Parsed<Expr> {
        self.deeper()?;
        let out = self.prefix();
        self.deep -= 1;
        out
    }

    /// One level further in, or the fault that says this is far enough.
    fn deeper(&mut self) -> Parsed<()> {
        if self.deep >= MAX_NESTING {
            let span = self.span();
            self.faults.push(Fault { span, kind: FaultKind::TooDeep });
            return Err(Given);
        }
        self.deep += 1;
        Ok(())
    }

    fn prefix(&mut self) -> Parsed<Expr> {
        let from = self.span();
        let un = match self.peek() {
            Some(TokenKind::Punct(Punct::Minus)) => Some(UnOp::Neg),
            Some(TokenKind::Punct(Punct::Bang)) => Some(UnOp::Not),
            Some(TokenKind::Punct(Punct::Tilde)) => Some(UnOp::BitNot),
            _ => None,
        };
        if let Some(op) = un {
            self.at += 1;
            let operand = Box::new(self.unary()?);
            return Ok(Expr { kind: ExprKind::Unary { op, operand }, span: self.since(from) });
        }
        let rite = match self.peek() {
            Some(TokenKind::Keyword(Keyword::Seal)) => Some(Rite::Seal),
            Some(TokenKind::Keyword(Keyword::Shade)) => Some(Rite::Shade),
            Some(TokenKind::Keyword(Keyword::Look)) => Some(Rite::Look),
            Some(TokenKind::Keyword(Keyword::Opaque)) => Some(Rite::Opaque),
            _ => None,
        };
        if let Some(rite) = rite {
            self.at += 1;
            let operand = Box::new(self.unary()?);
            return Ok(Expr { kind: ExprKind::Rite { rite, operand }, span: self.since(from) });
        }
        self.postfix()
    }

    fn postfix(&mut self) -> Parsed<Expr> {
        let from = self.span();
        let mut base = self.primary()?;
        loop {
            let kind = if self.eat(Punct::LParen) {
                let mut args = Vec::new();
                while !self.at_punct(Punct::RParen) {
                    args.push(self.expr()?);
                    if !self.eat(Punct::Comma) {
                        break;
                    }
                }
                self.expect(Punct::RParen, "`)` to close the arguments")?;
                ExprKind::Call { callee: Box::new(base), args }
            } else if self.eat(Punct::LBracket) {
                let index = Box::new(self.expr()?);
                self.expect(Punct::RBracket, "`]` to close the index")?;
                ExprKind::Index { base: Box::new(base), index }
            } else if self.eat(Punct::Dot) {
                let name = self.name("a field name")?;
                ExprKind::Field { base: Box::new(base), name }
            } else {
                return Ok(base);
            };
            base = Expr { kind, span: self.since(from) };
        }
    }

    fn primary(&mut self) -> Parsed<Expr> {
        let from = self.span();
        if self.at_punct(Punct::LBrace) {
            let block = self.block()?;
            return Ok(Expr { kind: ExprKind::Block(block), span: self.since(from) });
        }
        if self.eat(Punct::LParen) {
            let inner = self.expr()?;
            self.expect(Punct::RParen, "`)` to close the group")?;
            return Ok(inner);
        }
        if self.at_keyword(Keyword::Sizeof) {
            let span = self.span();
            self.faults.push(Fault { span, kind: FaultKind::Reserved("sizeof") });
            return Err(Given);
        }
        if self.eat_keyword(Keyword::Descend) {
            let capability = self.capability()?;
            let body = self.block()?;
            let kind = ExprKind::Descend { capability, body };
            return Ok(Expr { kind, span: self.since(from) });
        }
        let kind = match self.bump() {
            Some(TokenKind::Int(n)) => ExprKind::Int(*n),
            Some(TokenKind::Str(s)) => ExprKind::Str(s.clone()),
            Some(TokenKind::Bytes(b)) => ExprKind::Bytes(b.clone()),
            Some(TokenKind::Keyword(Keyword::True)) => ExprKind::Bool(true),
            Some(TokenKind::Keyword(Keyword::False)) => ExprKind::Bool(false),
            Some(TokenKind::Ident(text)) => ExprKind::Name(Name { text: text.clone(), span: from }),
            _ => {
                self.at -= usize::from(self.at > 0);
                return Err(self.complain("an expression"));
            }
        };
        Ok(Expr { kind, span: self.since(from) })
    }

    /// `descend κ { … }`. The `!` half of a pair is two tokens, because §3.7
    /// has no `disk!` and a lexer that invented one would be guessing.
    fn capability(&mut self) -> Parsed<Name> {
        let mut name = self.name("a capability")?;
        if self.eat(Punct::Bang) {
            name.text.push('!');
            name.span = self.since(name.span);
        }
        Ok(name)
    }
}

/// What a line of a block turned out to be.
enum Line {
    Stmt(Stmt),
    Tail(Expr),
}

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(src: &str) -> Unit {
        parse(src.as_bytes()).unwrap_or_else(|f| panic!("{f:?}"))
    }

    #[test]
    fn an_empty_file_is_a_unit_with_nothing_in_it() {
        assert_eq!(unit(""), Unit::default());
        assert_eq!(unit("// nothing\n"), Unit::default());
    }

    #[test]
    fn a_demand_is_an_item() {
        let u = unit("demand 1;");
        assert!(matches!(u.items[..], [Item::Demand { .. }]));
    }

    #[test]
    fn a_capability_keeps_its_bang() {
        let u = unit("U0 f() { descend disk! { 1 }; }");
        let Item::Func(f) = &u.items[0] else { panic!() };
        let Stmt::Expr(Expr { kind: ExprKind::Descend { capability, .. }, .. }) = &f.body.stmts[0]
        else {
            panic!("{:?}", f.body.stmts[0])
        };
        assert_eq!(capability.text, "disk!");
    }
}
