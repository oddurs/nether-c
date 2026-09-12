//! Printing a thing that went wrong the way the specification prints one.
//!
//! The shape is fixed by the errors in `spec/01-strata.md` §1.6 and
//! `spec/06-evaluation.md` §6.4, and those two are fixtures rather than
//! illustrations: an error text a specification prints is an error text an
//! implementation owes.
//!
//! Everything that can go wrong turns into a [`Diagnostic`] first, so there is
//! one renderer and not one per kind of wrongness.

use core::fmt::Write as _;

use crate::ir::Span;

/// Something to say, and where to point while saying it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Diagnostic {
    /// What to underline.
    pub span: Span,
    /// The first line, after `error: `.
    pub headline: String,
    /// What goes at the end of the caret row, about the thing underlined.
    pub label: Option<String>,
    /// The line after the gap, which says what to do rather than what is
    /// wrong.
    pub note: Option<String>,
}

/// Render a diagnostic against the source it came from.
///
/// `path` is how the file should be named in the location line, which the IR
/// does not know and should not: a path is a fact about one machine at one
/// moment.
#[must_use]
pub fn report(d: &Diagnostic, source: &str, path: &str) -> String {
    let start = (d.span.start as usize).min(source.len());
    let before = &source[..start];
    let number = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[line_start..].find('\n').map_or(source.len(), |i| line_start + i);
    let text = source[line_start..line_end].trim_end_matches('\r');
    let column = source[line_start..start].chars().count() + 1;
    let end = (d.span.end as usize).clamp(start, line_end);
    let width = source[start..end].chars().count().max(1);

    // Everything lines up against the widest line number, which is this one.
    let gutter = number.to_string().len();
    let bar = format!("{:gutter$} |", "", gutter = gutter);

    let mut out = String::new();
    let headline = &d.headline;
    let _ = writeln!(out, "error: {headline}");
    let _ = writeln!(out, "{:gutter$}--> {path}:{number}:{column}", "", gutter = gutter);
    let _ = writeln!(out, "{bar}");
    let _ = writeln!(out, "{number} | {text}");
    let _ = write!(out, "{bar} {:column$}{:^<width$}", "", "", column = column - 1, width = width);
    if let Some(label) = &d.label {
        let _ = write!(out, " {label}");
    }
    let _ = writeln!(out);
    if let Some(note) = &d.note {
        let _ = writeln!(out, "{bar}");
        let _ = writeln!(out, "{:gutter$} = {note}", "", gutter = gutter);
    }
    out
}

/// A number with its digits grouped, the way §6.4 prints one.
///
/// Six lines, because a program that reports 262144 unrollings is reporting a
/// number nobody is going to read.
#[must_use]
pub fn grouped(n: u64) -> String {
    let digits = n.to_string();
    let mut out = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, c) in digits.char_indices() {
        if i > 0 && (digits.len() - i) % 3 == 0 {
            out.push(',');
        }
        out.push(c);
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plain(span: Span, headline: &str) -> Diagnostic {
        Diagnostic { span, headline: headline.into(), label: None, note: None }
    }

    #[test]
    fn the_gutter_widens_with_the_line_number() {
        let source = "x\n".repeat(200);
        let d = plain(Span { start: 200, end: 201 }, "something");
        let out = report(&d, &source, "wide.nc");
        assert!(out.contains("  --> wide.nc:101:1"), "{out}");
        assert!(out.contains("101 | x"), "{out}");
    }

    #[test]
    fn a_diagnostic_with_nothing_to_suggest_stops_after_the_carets() {
        let d = plain(Span { start: 0, end: 1 }, "this local is not bound here");
        let out = report(&d, "a\n", "x.nc");
        assert!(!out.contains(" = "), "{out}");
        assert_eq!(out.lines().count(), 5, "{out}");
    }

    #[test]
    fn a_zero_width_span_still_gets_one_caret() {
        let d = plain(Span::default(), "nowhere in particular");
        let out = report(&d, "abc\n", "x.nc");
        assert!(out.contains("| ^"), "{out}");
    }

    #[test]
    fn digits_are_grouped_the_way_the_specification_prints_them() {
        assert_eq!(grouped(0), "0");
        assert_eq!(grouped(999), "999");
        assert_eq!(grouped(1_000), "1,000");
        assert_eq!(grouped(262_144), "262,144");
        assert_eq!(grouped(1_000_000), "1,000,000");
    }
}
