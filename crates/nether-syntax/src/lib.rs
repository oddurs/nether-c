//! The lexer.
//!
//! UTF-8 in, tokens with byte spans out. `spec/03-lexical.md`.
//!
//! Spans are load-bearing. Every diagnostic in the language points back at
//! source and so does every hole, so a token that does not know where it came
//! from is a token that makes the rest of the implementation lie.

pub mod ast;
mod lex;
mod parse;
mod token;

pub use lex::{Fault, FaultKind, lex};
pub use parse::{levels, parse};
pub use token::{Keyword, Punct, Token, TokenKind};
