//! Lexing, parsing, lowering, and printing for Nether C.
//!
//! UTF-8 source in, typed IR out, and back again (`spec/03-lexical.md`,
//! `spec/04-grammar.md`, `spec/06-evaluation.md` §6.5).
//!
//! Spans are load-bearing. Every diagnostic in the language points back at
//! source and so does every hole, so a token that does not know where it came
//! from is a token that makes the rest of the implementation lie.

pub mod ast;
mod lex;
mod lower;
mod parse;
mod print;
mod token;

pub use lex::{Fault, FaultKind, MAX_SOURCE, lex};
pub use lower::lower;
pub use parse::{MAX_NESTING, levels, parse};
pub use print::print;
pub use token::{Keyword, Punct, Token, TokenKind};
