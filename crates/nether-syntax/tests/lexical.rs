//! The proof for *nether-syntax: the lexer*: every token in §03 is produced,
//! with correct spans.
//!
//! The keyword and punctuation tables are read back out of `spec/03-lexical.md`
//! and compared, so a token added to the specification fails this file until
//! the lexer has one, and one removed fails it until the lexer does not.

use std::collections::BTreeSet;

use nether_core::Span;
use nether_syntax::{Fault, FaultKind, Keyword, Punct, Token, TokenKind, lex};

fn spec() -> String {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../spec/03-lexical.md");
    std::fs::read_to_string(path).expect("cannot read spec/03-lexical.md")
}

/// The whitespace-separated words of the fenced block containing `marker`.
fn fenced_words(marker: &str) -> BTreeSet<String> {
    let text = spec();
    let mut block: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("```") {
            if inside && block.iter().any(|l| l.contains(marker)) {
                return block.join(" ").split_whitespace().map(ToString::to_string).collect();
            }
            inside = !inside;
            block.clear();
            continue;
        }
        if inside {
            block.push(line);
        }
    }
    panic!("§03 has no fenced block containing `{marker}`");
}

fn kinds(src: &str) -> Vec<TokenKind> {
    lex(src.as_bytes()).expect("lexes").into_iter().map(|t| t.kind).collect()
}

fn fault(src: &str) -> FaultKind {
    lex(src.as_bytes()).expect_err("this is not a source file").kind
}

// ── §03, read back ──────────────────────────────────────────────────────────

#[test]
fn the_keywords_are_the_ones_in_section_three_point_four() {
    let in_spec = fenced_words("descend");
    let in_code: BTreeSet<String> =
        Keyword::all().iter().map(|k| k.spelling().to_string()).collect();
    assert_eq!(in_code, in_spec, "the reserved words and §3.4 have parted");
}

#[test]
fn the_punctuation_is_what_section_three_point_seven_lists() {
    let in_spec = fenced_words("->");
    let in_code: BTreeSet<String> = Punct::all().iter().map(|p| p.spelling().to_string()).collect();
    assert_eq!(in_code, in_spec, "the punctuation and §3.7 have parted");
}

#[test]
fn every_keyword_and_every_punctuation_lexes_as_itself() {
    for k in Keyword::all() {
        assert_eq!(kinds(k.spelling()), vec![TokenKind::Keyword(k)], "{k}");
    }
    for p in Punct::all() {
        // `@` before a digit is a depth annotation, which is §3.5 and is
        // tested there; on its own it is punctuation.
        assert_eq!(kinds(p.spelling()), vec![TokenKind::Punct(p)], "{p}");
    }
}

#[test]
fn the_longest_punctuation_wins() {
    assert_eq!(kinds("<<="), vec![TokenKind::Punct(Punct::ShlEq)]);
    assert_eq!(kinds("<< ="), vec![TokenKind::Punct(Punct::Shl), TokenKind::Punct(Punct::Eq)]);
    assert_eq!(kinds("<<"), vec![TokenKind::Punct(Punct::Shl)]);
    assert_eq!(kinds("<"), vec![TokenKind::Punct(Punct::Lt)]);
}

#[test]
fn the_tokens_section_three_point_eight_says_are_absent_are_absent() {
    // No preprocessor. §3.6 spends the `#` on the cairn literal, so a reader
    // who types one finds out from the cairn's own diagnostic rather than
    // from "this starts nothing" — which is what §3.8 is for.
    let said = lex(b"#include <stdio.h>").unwrap_err().to_string();
    assert!(said.contains("no preprocessor"), "{said}");
    // And `#exe`, which is the one this language actually replaced.
    assert!(lex(b"#exe {}").unwrap_err().to_string().contains("no preprocessor"));
    // No increment: `++` is two `+`, which the grammar has nowhere to put.
    assert_eq!(kinds("++"), vec![TokenKind::Punct(Punct::Plus), TokenKind::Punct(Punct::Plus)]);
    // No `goto`: it is not reserved, so it is an ordinary identifier and the
    // grammar rejects it where a statement was expected.
    assert_eq!(kinds("goto"), vec![TokenKind::Ident("goto".into())]);
}

// ── §3.1 source encoding ────────────────────────────────────────────────────

#[test]
fn a_file_that_is_not_utf8_is_rejected_rather_than_repaired() {
    assert_eq!(lex(&[0x66, 0xff, 0x6f]).unwrap_err().kind, FaultKind::NotUtf8);
}

#[test]
fn a_carriage_return_belongs_before_a_line_feed_and_nowhere_else() {
    assert_eq!(kinds("1\r\n2"), vec![TokenKind::Int(1), TokenKind::Int(2)]);
    assert_eq!(fault("1\r2"), FaultKind::StrayCarriageReturn);
}

// ── §3.3 identifiers ────────────────────────────────────────────────────────

#[test]
fn one_name_spelled_two_ways_is_one_name() {
    // `é` composed, and `e` with a combining acute. §3.3: normalisation
    // happens in the lexer and nowhere below it, so that two spellings of one
    // name cannot resolve to two different bindings.
    let composed = kinds("caf\u{e9}");
    let decomposed = kinds("cafe\u{301}");
    assert_eq!(composed, decomposed);
    assert_eq!(composed, vec![TokenKind::Ident("caf\u{e9}".into())]);
}

#[test]
fn an_identifier_is_xid_and_underscore() {
    assert_eq!(kinds("_"), vec![TokenKind::Ident("_".into())]);
    assert_eq!(kinds("_x1"), vec![TokenKind::Ident("_x1".into())]);
    assert_eq!(kinds("Ω"), vec![TokenKind::Ident("Ω".into())]);
    // A digit does not start one.
    assert_eq!(kinds("1x"), vec![TokenKind::Int(1), TokenKind::Ident("x".into())]);
}

// ── §3.5 depth annotations ──────────────────────────────────────────────────

#[test]
fn a_depth_on_a_type_is_one_token_sequence() {
    assert_eq!(
        kinds("Bytes@3"),
        vec![TokenKind::Ident("Bytes".into()), TokenKind::Depth(3)],
        "§3.5: no whitespace is permitted in a type annotation"
    );
}

#[test]
fn a_depth_on_a_signature_may_have_a_space_in_it() {
    assert_eq!(kinds("@ 3"), vec![TokenKind::Punct(Punct::At), TokenKind::Int(3)]);
}

#[test]
fn a_depth_is_zero_to_eight() {
    for d in 0..=8u8 {
        assert_eq!(kinds(&format!("@{d}")), vec![TokenKind::Depth(d)]);
    }
    assert_eq!(fault("@9"), FaultKind::NotAStratum);
}

// ── §3.6 literals ───────────────────────────────────────────────────────────

#[test]
fn an_integer_is_decimal_hexadecimal_or_binary() {
    assert_eq!(kinds("1_000_000"), vec![TokenKind::Int(1_000_000)]);
    assert_eq!(kinds("0xdead_beef"), vec![TokenKind::Int(0xdead_beef)]);
    assert_eq!(kinds("0b1010_0101"), vec![TokenKind::Int(0b1010_0101)]);
    assert_eq!(kinds("0"), vec![TokenKind::Int(0)]);
}

#[test]
fn a_decimal_spells_a_magnitude_and_a_hexadecimal_spells_a_pattern() {
    // Decimal tops out at what an `I64` holds, because that is what it is
    // spelling. Hexadecimal and binary run the whole width, which is the only
    // way to write −2⁶³ in a language whose `-` is an operator.
    assert_eq!(kinds("9223372036854775807"), vec![TokenKind::Int(i64::MAX)]);
    assert_eq!(fault("9223372036854775808"), FaultKind::NotAnI64);
    assert_eq!(kinds("0x8000000000000000"), vec![TokenKind::Int(i64::MIN)]);
    assert_eq!(fault("0x1_0000_0000_0000_0000"), FaultKind::NotAnI64);
    assert_eq!(fault("0x"), FaultKind::EmptyNumber);
}

#[test]
fn a_string_resolves_its_escapes_and_bytes_keeps_the_b() {
    assert_eq!(kinds(r#""nc""#), vec![TokenKind::Str("nc".into())]);
    assert_eq!(kinds(r#"b"nc""#), vec![TokenKind::Bytes(b"nc".to_vec())]);
    assert_eq!(kinds(r#""\n\t\r\0\\\"""#), vec![TokenKind::Str("\n\t\r\0\\\"".into())]);
    assert_eq!(kinds(r#""\u{1F480}""#), vec![TokenKind::Str("💀".into())]);
}

#[test]
fn a_string_that_is_never_closed_says_so_where_it_opened() {
    assert_eq!(fault("\"nc"), FaultKind::UnterminatedString);
    assert_eq!(fault("\"nc\n\""), FaultKind::UnterminatedString);
    assert_eq!(fault(r#""\q""#), FaultKind::UnknownEscape);
    assert_eq!(fault(r#""\u{110000}""#), FaultKind::NotAScalar);
    assert_eq!(fault("/* never"), FaultKind::UnterminatedComment);
}

// ── spans ───────────────────────────────────────────────────────────────────

/// A source with one of everything in it.
const SAMPLE: &str = r#"// a build
struct Header {
  I64@3 len;   /* one */
  Bytes tag;
};

Answer<Bytes> read_it(Str p) @ 3
{
  if (p != "") { return descend disk { read(p) }; }
  I64 n = 0x1f + 0b11 - 1_000;
  n <<= 2;
  seal shade look opaque n;
  b"\u{e9}";
  demand n;
}
"#;

#[test]
fn every_token_knows_where_it_came_from() {
    let tokens = lex(SAMPLE.as_bytes()).expect("the sample lexes");
    assert!(tokens.len() > 60, "only {} tokens", tokens.len());

    for Token { kind, span } in &tokens {
        let Span { start, end } = *span;
        assert!(start < end, "a token with no width: {kind:?}");
        let text = &SAMPLE[start as usize..end as usize];
        match kind {
            TokenKind::Ident(name) => {
                // The span is the source, which is not always the token: an
                // identifier is normalised and the source is not.
                assert_eq!(name.chars().count(), text.chars().count(), "{name} vs {text}");
            }
            TokenKind::Keyword(k) => assert_eq!(text, k.spelling()),
            TokenKind::Punct(p) => assert_eq!(text, p.spelling()),
            TokenKind::Depth(d) => assert_eq!(text, format!("@{d}")),
            TokenKind::Int(_) => assert!(text.starts_with(|c: char| c.is_ascii_digit()), "{text}"),
            TokenKind::Str(_) => assert!(text.starts_with('"') && text.ends_with('"'), "{text}"),
            TokenKind::Bytes(_) => {
                assert!(text.starts_with("b\"") && text.ends_with('"'), "{text}");
            }
            TokenKind::Cairn(_) => {
                assert!(text.starts_with('#') && text.len() == 65, "{text}");
            }
        }
    }
}

#[test]
fn the_spans_are_in_order_and_do_not_overlap() {
    let tokens = lex(SAMPLE.as_bytes()).expect("the sample lexes");
    for pair in tokens.windows(2) {
        assert!(pair[0].span.end <= pair[1].span.start, "{:?} then {:?}", pair[0], pair[1]);
    }
}

#[test]
fn comments_leave_no_token_and_no_gap_in_the_offsets() {
    let tokens = lex(b"1 /* two */ 3").expect("lexes");
    assert_eq!(tokens.len(), 2);
    assert_eq!(tokens[0].span, Span { start: 0, end: 1 });
    assert_eq!(tokens[1].span, Span { start: 12, end: 13 });
}

#[test]
fn a_fault_points_at_what_is_wrong() {
    let f = lex(b"I64 n = 1;\n@9;\n").unwrap_err();
    assert_eq!(f.kind, FaultKind::NotAStratum);
    let rendered = nether_core::report(&f.diagnostic(), "I64 n = 1;\n@9;\n", "x.nc");
    assert!(rendered.contains("--> x.nc:2:1"), "{rendered}");
    assert!(rendered.contains("a depth is 0 to 8"), "{rendered}");
}

/// The limit that keeps every span exact.
///
/// A `Span` is a pair of `u32` offsets. A source longer than that has bytes no
/// diagnostic could point at, and the code used to clamp them all to the same
/// number instead — wrong spans, with no signal. §6.4: stated, and reported.
///
/// Four gigabytes is not a fixture, so what is checked is the pair that could
/// drift apart: the limit, and what a span can hold.
#[test]
fn a_source_a_span_cannot_address_is_refused_rather_than_clamped() {
    assert_eq!(nether_syntax::MAX_SOURCE, u32::MAX as usize, "a span is two u32s");
    let said = Fault { span: Span::default(), kind: FaultKind::TooBig }.to_string();
    assert!(said.contains(&nether_syntax::MAX_SOURCE.to_string()), "{said}");
}

// ── §3.6, the cairn literal ─────────────────────────────────────────────────

/// Sixty-four lowercase hex digits, as `nether lamp` prints them.
const NAME: &str = "f1353fd9d1aea164452daae9d822f156fb89e8f4d7734cf382d453f0afcfc60f";

#[test]
fn a_cairn_literal_is_thirty_two_bytes() {
    let source = format!("#{NAME}");
    let tokens = lex(source.as_bytes()).expect("lexes");
    assert_eq!(tokens.len(), 1);
    let TokenKind::Cairn(bytes) = &tokens[0].kind else { panic!("not a cairn: {:?}", tokens[0]) };
    assert_eq!(bytes[0], 0xf1, "the first digit pair is the first byte");
    assert_eq!(bytes[31], 0x0f, "and the last pair the last");
    assert_eq!(tokens[0].span, Span { start: 0, end: 65 }, "the `#` is part of it");
}

#[test]
fn every_near_miss_says_which_one_it_is() {
    // §3.6 gives a cairn one spelling. A cairn is sixty-five characters long,
    // so "this starts nothing" pointing at the sixty-fourth of them is a
    // message nobody can act on: each way to miss says which it was.
    let short = format!("#{}", &NAME[..8]);
    let long = format!("#{NAME}f");
    let upper = format!("#{}", NAME.to_uppercase());
    let split = format!("#{}_{}", &NAME[..4], &NAME[4..]);
    let past_f = format!("#{}z", &NAME[..63]);
    for (source, expected) in [
        (&short, "too short"),
        (&long, "too long"),
        (&upper, "a capital"),
        (&split, "a `_`"),
        (&past_f, "past `f`"),
    ] {
        let f = lex(source.as_bytes()).unwrap_err();
        let said = f.to_string();
        assert!(said.contains("sixty-four lowercase hex digits"), "{source}: {said}");
        assert!(said.contains(expected), "{source}: expected `{expected}` in: {said}");
    }
}

#[test]
fn a_cairn_is_not_an_identifier_and_an_identifier_is_not_a_cairn() {
    // The `#` is why the mark exists: `deadbeef` is a plausible cairn prefix
    // and a plausible variable name, and only one of them is a literal.
    let tokens = lex(NAME.as_bytes()).expect("a bare hex word lexes");
    assert!(matches!(tokens[0].kind, TokenKind::Ident(_)), "{:?}", tokens[0]);
}

/// §3.6: `\xNN` is one byte, exactly two digits, and only in a bytes literal.
#[test]
fn a_byte_escape_is_two_digits_and_only_in_bytes() {
    assert_eq!(kinds(r#"b"\xe2\x80\x94""#), vec![TokenKind::Bytes("\u{2014}".as_bytes().to_vec())]);
    // Exactly two: the third digit is the letter that follows the escape.
    assert_eq!(kinds(r#"b"\xeff""#), vec![TokenKind::Bytes(b"\xeff".to_vec())]);
    // A byte that is not text at all, which is the case `\u{…}` cannot spell.
    assert_eq!(kinds(r#"b"\x80""#), vec![TokenKind::Bytes(vec![0x80])]);
    for bad in [r#"b"\xg0""#, r#"b"\xe""#, r#"b"\x""#, r#""\xe2""#] {
        assert_eq!(fault(bad), FaultKind::UnknownEscape, "accepted {bad}");
    }
}
