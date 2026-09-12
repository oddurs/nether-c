//! The scanner.
//!
//! One pass, no backtracking, a byte offset for everything. `spec/03-lexical.md`.

use core::fmt;

use nether_core::{Diagnostic, Span};
use unicode_normalization::UnicodeNormalization as _;

use crate::token::{Keyword, PUNCT, Token, TokenKind};

/// Something the source is not.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Where it is.
    pub span: Span,
    /// What it is.
    pub kind: FaultKind,
}

/// What kind of thing the source is not.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// Not well-formed UTF-8. §3.1 requires rejecting it rather than
    /// substituting replacement characters: a file that is not what it claims
    /// to be should say so once, here, and not once per mangled identifier.
    NotUtf8,
    /// A carriage return that is not immediately before a line feed. §3.1.
    StrayCarriageReturn,
    /// A `/*` with no `*/`. They do not nest, so there is no other reading.
    UnterminatedComment,
    /// A string literal that reaches the end of the line or the file.
    UnterminatedString,
    /// A `\` followed by something §3.6 does not list.
    UnknownEscape,
    /// A `\u{…}` that is not a scalar value.
    NotAScalar,
    /// A number with no digits after its prefix.
    EmptyNumber,
    /// A literal outside what an `I64` holds.
    NotAnI64,
    /// A `@` followed by a digit that is not a stratum.
    NotAStratum,
    /// A character that starts nothing.
    Stray,
    /// The parser wanted something the source does not have there.
    Expected(&'static str),
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self.kind {
            FaultKind::NotUtf8 => "this file is not well-formed UTF-8",
            FaultKind::StrayCarriageReturn => "a carriage return outside a line ending",
            FaultKind::UnterminatedComment => "this comment is never closed",
            FaultKind::UnterminatedString => "this string is never closed",
            FaultKind::UnknownEscape => "this is not an escape",
            FaultKind::NotAScalar => "this is not a Unicode scalar value",
            FaultKind::EmptyNumber => "this number has no digits",
            FaultKind::NotAnI64 => "this does not fit in an I64",
            FaultKind::NotAStratum => "a depth is 0 to 8",
            FaultKind::Stray => "this starts nothing",
            FaultKind::Expected(what) => return write!(f, "expected {what}"),
        })
    }
}

impl Fault {
    /// This fault, ready to print.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        let note = match self.kind {
            FaultKind::NotUtf8 => {
                Some("§3.1: a file that is not UTF-8 is rejected rather than repaired.")
            }
            FaultKind::StrayCarriageReturn => {
                Some("§3.1: a line ends with U+000A, and U+000D is only allowed before one.")
            }
            FaultKind::UnknownEscape => Some(r#"the escapes are \n \t \r \0 \\ \" and \u{…}."#),
            FaultKind::NotAnI64 => Some("there is one integer type, and this is outside it."),
            _ => None,
        };
        Diagnostic {
            span: self.span,
            headline: self.to_string(),
            label: None,
            note: note.map(ToString::to_string),
        }
    }
}

/// Lex a source file.
///
/// # Errors
///
/// The first thing the source is not. A broken token stream is not worth
/// continuing past: everything after the first fault is a guess.
pub fn lex(source: &[u8]) -> Result<Vec<Token>, Fault> {
    let text = core::str::from_utf8(source)
        .map_err(|e| Fault { span: at(e.valid_up_to()), kind: FaultKind::NotUtf8 })?;
    Scanner { text, at: 0, out: Vec::new() }.run()
}

fn at(offset: usize) -> Span {
    let n = u32::try_from(offset).unwrap_or(u32::MAX);
    Span { start: n, end: n }
}

struct Scanner<'a> {
    text: &'a str,
    at: usize,
    out: Vec<Token>,
}

impl Scanner<'_> {
    fn rest(&self) -> &str {
        &self.text[self.at..]
    }

    fn peek(&self) -> Option<char> {
        self.rest().chars().next()
    }

    fn span(&self, from: usize) -> Span {
        Span {
            start: u32::try_from(from).unwrap_or(u32::MAX),
            end: u32::try_from(self.at).unwrap_or(u32::MAX),
        }
    }

    fn fault(&self, from: usize, kind: FaultKind) -> Fault {
        Fault { span: self.span(from), kind }
    }

    fn push(&mut self, from: usize, kind: TokenKind) {
        let span = self.span(from);
        self.out.push(Token { kind, span });
    }

    fn run(mut self) -> Result<Vec<Token>, Fault> {
        while let Some(c) = self.peek() {
            let from = self.at;
            match c {
                ' ' | '\t' | '\n' => self.at += c.len_utf8(),
                // §3.1: a U+000D immediately preceding U+000A is discarded,
                // and one anywhere else is an error.
                '\r' => {
                    if self.rest().starts_with("\r\n") {
                        self.at += 1;
                    } else {
                        self.at += 1;
                        return Err(self.fault(from, FaultKind::StrayCarriageReturn));
                    }
                }
                '/' if self.rest().starts_with("//") => self.line_comment(),
                '/' if self.rest().starts_with("/*") => self.block_comment(from)?,
                '"' => self.string(from, false)?,
                'b' if self.rest().starts_with("b\"") => {
                    self.at += 1;
                    self.string(from, true)?;
                }
                '@' => self.depth(from)?,
                '0'..='9' => self.number(from)?,
                c if unicode_ident::is_xid_start(c) || c == '_' => self.word(from),
                _ => self.punctuation(from)?,
            }
        }
        Ok(self.out)
    }

    /// `// to the end of the line`. Comments are whitespace.
    fn line_comment(&mut self) {
        self.at += self.rest().find('\n').unwrap_or(self.rest().len());
    }

    /// `/* to the closing delimiter; these do not nest */`.
    fn block_comment(&mut self, from: usize) -> Result<(), Fault> {
        if let Some(i) = self.rest()[2..].find("*/") {
            self.at += 2 + i + 2;
            Ok(())
        } else {
            self.at = self.text.len();
            Err(self.fault(from, FaultKind::UnterminatedComment))
        }
    }

    /// An identifier or a keyword.
    ///
    /// §3.3: normalisation happens here and nowhere below. Two spellings of
    /// one name must not resolve to two bindings, and the ledger will not fix
    /// it later because the ledger stores the bytes it is handed.
    fn word(&mut self, from: usize) {
        while self.peek().is_some_and(|c| unicode_ident::is_xid_continue(c) || c == '_') {
            self.at += self.peek().map_or(0, char::len_utf8);
        }
        let raw = &self.text[from..self.at];
        let kind = match Keyword::from_word(raw) {
            // A keyword is ASCII, so there is nothing for NFC to do to one,
            // and one that needed normalising to become a keyword would be a
            // program spelling `descend` two ways.
            Some(k) => TokenKind::Keyword(k),
            None => TokenKind::Ident(raw.nfc().collect()),
        };
        self.push(from, kind);
    }

    /// `@` and a digit with nothing between them, which §3.5 requires of a
    /// depth on a type. With whitespace between, this is a `@` and a number,
    /// and only a signature may be written that way.
    fn depth(&mut self, from: usize) -> Result<(), Fault> {
        let next = self.rest()[1..].chars().next();
        let Some(d) = next.filter(char::is_ascii_digit).and_then(|c| c.to_digit(10)) else {
            return self.punctuation(from);
        };
        self.at += 2;
        let Ok(d) = u8::try_from(d) else { unreachable!("a digit is below ten") };
        if d > 8 {
            return Err(self.fault(from, FaultKind::NotAStratum));
        }
        self.push(from, TokenKind::Depth(d));
        Ok(())
    }

    /// A decimal, hexadecimal or binary integer. §3.6.
    ///
    /// Decimal spells a magnitude and so tops out at `2⁶³ − 1`; hexadecimal
    /// and binary spell a bit pattern and so run the whole width, which is the
    /// only way to write `−2⁶³` in a language whose `-` is an operator.
    fn number(&mut self, from: usize) -> Result<(), Fault> {
        let (radix, prefix) = match self.rest().get(..2) {
            Some("0x" | "0X") => (16, 2),
            Some("0b" | "0B") => (2, 2),
            _ => (10, 0),
        };
        self.at += prefix;
        let digits_from = self.at;
        while self.peek().is_some_and(|c| c == '_' || c.is_digit(radix)) {
            self.at += 1;
        }
        let digits: String =
            self.text[digits_from..self.at].chars().filter(|c| *c != '_').collect();
        if digits.is_empty() {
            return Err(self.fault(from, FaultKind::EmptyNumber));
        }
        let value = if radix == 10 {
            digits.parse::<i64>().map_err(|_| self.fault(from, FaultKind::NotAnI64))?
        } else {
            let bits = u64::from_str_radix(&digits, radix)
                .map_err(|_| self.fault(from, FaultKind::NotAnI64))?;
            i64::from_ne_bytes(bits.to_ne_bytes())
        };
        self.push(from, TokenKind::Int(value));
        Ok(())
    }

    /// A string or bytes literal. The opening quote is at `self.at`.
    fn string(&mut self, from: usize, raw: bool) -> Result<(), Fault> {
        self.at += 1;
        let mut out = String::new();
        loop {
            let Some(c) = self.peek() else {
                return Err(self.fault(from, FaultKind::UnterminatedString));
            };
            self.at += c.len_utf8();
            match c {
                '"' => break,
                // A string that runs off the end of its line is a missing
                // quote, and saying so here beats saying it four lines later.
                '\n' => return Err(self.fault(from, FaultKind::UnterminatedString)),
                '\\' => out.push(self.escape(from)?),
                _ => out.push(c),
            }
        }
        let kind = if raw { TokenKind::Bytes(out.into_bytes()) } else { TokenKind::Str(out) };
        self.push(from, kind);
        Ok(())
    }

    /// One escape, after the backslash. §3.6.
    fn escape(&mut self, from: usize) -> Result<char, Fault> {
        let here = self.at;
        let Some(c) = self.peek() else {
            return Err(self.fault(from, FaultKind::UnterminatedString));
        };
        self.at += c.len_utf8();
        Ok(match c {
            'n' => '\n',
            't' => '\t',
            'r' => '\r',
            '0' => '\0',
            '\\' => '\\',
            '"' => '"',
            'u' => return self.unicode_escape(here),
            _ => return Err(self.fault(here, FaultKind::UnknownEscape)),
        })
    }

    /// `\u{…}`, where the digits are hexadecimal and name a scalar value.
    fn unicode_escape(&mut self, from: usize) -> Result<char, Fault> {
        if !self.rest().starts_with('{') {
            return Err(self.fault(from, FaultKind::UnknownEscape));
        }
        self.at += 1;
        let digits_from = self.at;
        while self.peek().is_some_and(|c| c.is_ascii_hexdigit()) {
            self.at += 1;
        }
        let digits = &self.text[digits_from..self.at];
        if !self.rest().starts_with('}') {
            return Err(self.fault(from, FaultKind::UnknownEscape));
        }
        self.at += 1;
        if digits.is_empty() {
            return Err(self.fault(from, FaultKind::EmptyNumber));
        }
        u32::from_str_radix(digits, 16)
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| self.fault(from, FaultKind::NotAScalar))
    }

    /// The longest punctuation token that fits. §3.7.
    fn punctuation(&mut self, from: usize) -> Result<(), Fault> {
        for (spelling, punct) in PUNCT {
            if self.rest().starts_with(spelling) {
                self.at += spelling.len();
                self.push(from, TokenKind::Punct(punct));
                return Ok(());
            }
        }
        self.at += self.peek().map_or(1, char::len_utf8);
        Err(self.fault(from, FaultKind::Stray))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::token::KEYWORDS;

    fn kinds(src: &str) -> Vec<TokenKind> {
        lex(src.as_bytes()).expect("lexes").into_iter().map(|t| t.kind).collect()
    }

    #[test]
    fn every_keyword_is_a_keyword_and_not_an_identifier() {
        for (spelling, keyword) in KEYWORDS {
            assert_eq!(kinds(spelling), vec![TokenKind::Keyword(keyword)]);
        }
    }

    #[test]
    fn a_keyword_with_more_on_the_end_is_an_identifier() {
        assert_eq!(kinds("seals"), vec![TokenKind::Ident("seals".into())]);
        assert_eq!(kinds("_if"), vec![TokenKind::Ident("_if".into())]);
    }

    #[test]
    fn comments_are_whitespace() {
        assert_eq!(kinds("1 // two\n3"), vec![TokenKind::Int(1), TokenKind::Int(3)]);
        assert_eq!(kinds("1 /* two */ 3"), vec![TokenKind::Int(1), TokenKind::Int(3)]);
        // They do not nest, so the first `*/` closes.
        assert_eq!(kinds("/* /* */ 3"), vec![TokenKind::Int(3)]);
    }
}
