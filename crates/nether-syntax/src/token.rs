//! What the lexer produces.
//!
//! The keyword and punctuation tables are `spec/03-lexical.md` §3.4 and §3.7
//! typed out, and the proof reads both back out of the specification and
//! compares. A table copied by hand is a table that drifts.

use core::fmt;

use nether_core::Span;

/// A token, and where in the source it is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Token {
    /// What it is.
    pub kind: TokenKind,
    /// Byte offsets into the source it was lexed from.
    pub span: Span,
}

/// What a token is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TokenKind {
    /// An identifier, after NFC normalisation. §3.3.
    Ident(String),
    /// One of the reserved words. §3.4.
    Keyword(Keyword),
    /// An integer literal. There is one integer type.
    Int(i64),
    /// A string literal, after escapes are resolved.
    Str(String),
    /// A `b"…"` literal.
    Bytes(Vec<u8>),
    /// `@` immediately followed by a digit: a depth annotation on a type,
    /// which §3.5 requires to be one token sequence with no space in it.
    ///
    /// The other form — whitespace between the `@` and the digit, which §3.5
    /// permits on a signature — arrives as [`Punct::At`] and an [`Int`].
    ///
    /// [`Int`]: TokenKind::Int
    Depth(u8),
    /// Punctuation. §3.7.
    Punct(Punct),
}

/// A reserved word. Never usable as an identifier.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Keyword {
    /// `descend`
    Descend,
    /// `seal`
    Seal,
    /// `shade`
    Shade,
    /// `look`
    Look,
    /// `opaque`
    Opaque,
    /// `demand`
    Demand,
    /// `if`
    If,
    /// `else`
    Else,
    /// `while`
    While,
    /// `for`
    For,
    /// `return`
    Return,
    /// `break`
    Break,
    /// `continue`
    Continue,
    /// `struct`
    Struct,
    /// `typedef`
    Typedef,
    /// `sizeof`
    Sizeof,
    /// `true`
    True,
    /// `false`
    False,
}

/// Every keyword, spelled as §3.4 spells it.
pub(crate) const KEYWORDS: [(&str, Keyword); 18] = [
    ("descend", Keyword::Descend),
    ("seal", Keyword::Seal),
    ("shade", Keyword::Shade),
    ("look", Keyword::Look),
    ("opaque", Keyword::Opaque),
    ("demand", Keyword::Demand),
    ("if", Keyword::If),
    ("else", Keyword::Else),
    ("while", Keyword::While),
    ("for", Keyword::For),
    ("return", Keyword::Return),
    ("break", Keyword::Break),
    ("continue", Keyword::Continue),
    ("struct", Keyword::Struct),
    ("typedef", Keyword::Typedef),
    ("sizeof", Keyword::Sizeof),
    ("true", Keyword::True),
    ("false", Keyword::False),
];

impl Keyword {
    /// Every keyword there is.
    #[must_use]
    pub fn all() -> Vec<Self> {
        KEYWORDS.iter().map(|(_, k)| *k).collect()
    }

    /// How it is written.
    #[must_use]
    pub fn spelling(self) -> &'static str {
        KEYWORDS.iter().find(|(_, k)| *k == self).map_or("", |(s, _)| *s)
    }

    /// The keyword that word is, if it is one.
    #[must_use]
    pub fn from_word(word: &str) -> Option<Self> {
        KEYWORDS.iter().find(|(s, _)| *s == word).map(|(_, k)| *k)
    }
}

impl fmt::Display for Keyword {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.spelling())
    }
}

/// A punctuation token.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Punct {
    /// `<<=`
    ShlEq,
    /// `>>=`
    ShrEq,
    /// `->`
    Arrow,
    /// `<<`
    Shl,
    /// `>>`
    Shr,
    /// `==`
    EqEq,
    /// `!=`
    BangEq,
    /// `<=`
    Le,
    /// `>=`
    Ge,
    /// `&&`
    AmpAmp,
    /// `||`
    PipePipe,
    /// `+=`
    PlusEq,
    /// `-=`
    MinusEq,
    /// `*=`
    StarEq,
    /// `/=`
    SlashEq,
    /// `%=`
    PercentEq,
    /// `&=`
    AmpEq,
    /// `|=`
    PipeEq,
    /// `^=`
    CaretEq,
    /// `(`
    LParen,
    /// `)`
    RParen,
    /// `{`
    LBrace,
    /// `}`
    RBrace,
    /// `[`
    LBracket,
    /// `]`
    RBracket,
    /// `;`
    Semi,
    /// `,`
    Comma,
    /// `.`
    Dot,
    /// `@`
    At,
    /// `+`
    Plus,
    /// `-`
    Minus,
    /// `*`
    Star,
    /// `/`
    Slash,
    /// `%`
    Percent,
    /// `!`
    Bang,
    /// `~`
    Tilde,
    /// `&`
    Amp,
    /// `|`
    Pipe,
    /// `^`
    Caret,
    /// `=`
    Eq,
    /// `<`
    Lt,
    /// `>`
    Gt,
}

/// Every punctuation token, longest first.
///
/// The order is the whole of the matching rule: `<<=` has to be tried before
/// `<<`, and `<<` before `<`, or the lexer produces a different program from
/// the one that was written.
pub(crate) const PUNCT: [(&str, Punct); 42] = [
    ("<<=", Punct::ShlEq),
    (">>=", Punct::ShrEq),
    ("->", Punct::Arrow),
    ("<<", Punct::Shl),
    (">>", Punct::Shr),
    ("==", Punct::EqEq),
    ("!=", Punct::BangEq),
    ("<=", Punct::Le),
    (">=", Punct::Ge),
    ("&&", Punct::AmpAmp),
    ("||", Punct::PipePipe),
    ("+=", Punct::PlusEq),
    ("-=", Punct::MinusEq),
    ("*=", Punct::StarEq),
    ("/=", Punct::SlashEq),
    ("%=", Punct::PercentEq),
    ("&=", Punct::AmpEq),
    ("|=", Punct::PipeEq),
    ("^=", Punct::CaretEq),
    ("(", Punct::LParen),
    (")", Punct::RParen),
    ("{", Punct::LBrace),
    ("}", Punct::RBrace),
    ("[", Punct::LBracket),
    ("]", Punct::RBracket),
    (";", Punct::Semi),
    (",", Punct::Comma),
    (".", Punct::Dot),
    ("@", Punct::At),
    ("+", Punct::Plus),
    ("-", Punct::Minus),
    ("*", Punct::Star),
    ("/", Punct::Slash),
    ("%", Punct::Percent),
    ("!", Punct::Bang),
    ("~", Punct::Tilde),
    ("&", Punct::Amp),
    ("|", Punct::Pipe),
    ("^", Punct::Caret),
    ("=", Punct::Eq),
    ("<", Punct::Lt),
    (">", Punct::Gt),
];

impl Punct {
    /// Every punctuation token there is, longest first.
    #[must_use]
    pub fn all() -> Vec<Self> {
        PUNCT.iter().map(|(_, p)| *p).collect()
    }

    /// How it is written.
    #[must_use]
    pub fn spelling(self) -> &'static str {
        PUNCT.iter().find(|(_, p)| *p == self).map_or("", |(s, _)| *s)
    }
}

impl fmt::Display for Punct {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.spelling())
    }
}
