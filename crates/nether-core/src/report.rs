//! Printing a fault the way the specification prints one.
//!
//! The shape is fixed by the errors in `spec/01-strata.md` §1.6 and
//! `spec/06-evaluation.md` §6.4, and those two are fixtures rather than
//! illustrations: an error text a specification prints is an error text an
//! implementation owes.

use core::fmt::Write as _;

use crate::check::{Fault, FaultKind};
use crate::depth::Capability;

/// Render a fault against the source it came from.
///
/// `path` is how the file should be named in the location line, which the IR
/// does not know and should not: a path is a fact about one machine at one
/// moment.
#[must_use]
pub fn report(fault: &Fault, source: &str, path: &str) -> String {
    let start = (fault.span.start as usize).min(source.len());
    let before = &source[..start];
    let number = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let line_end = source[line_start..].find('\n').map_or(source.len(), |i| line_start + i);
    let text = source[line_start..line_end].trim_end_matches('\r');
    let column = source[line_start..start].chars().count() + 1;
    let end = (fault.span.end as usize).clamp(start, line_end);
    let width = source[start..end].chars().count().max(1);

    // Everything lines up against the widest line number, which is this one.
    let gutter = number.to_string().len();
    let bar = format!("{:gutter$} |", "", gutter = gutter);

    let mut out = String::new();
    let _ = writeln!(out, "error: {fault}");
    let _ = writeln!(out, "{:gutter$}--> {path}:{number}:{column}", "", gutter = gutter);
    let _ = writeln!(out, "{bar}");
    let _ = writeln!(out, "{number} | {text}");
    let _ = write!(out, "{bar} {:column$}{:^<width$}", "", "", column = column - 1, width = width);
    if let Some(label) = label(fault) {
        let _ = write!(out, " {label}");
    }
    let _ = writeln!(out);
    if let Some(note) = note(fault) {
        let _ = writeln!(out, "{bar}");
        let _ = writeln!(out, "{:gutter$} = {note}", "", gutter = gutter);
    }
    out
}

/// What goes at the end of the caret row, pointing at the thing underlined.
fn label(fault: &Fault) -> Option<String> {
    let blame = fault.blame.as_ref()?;
    match fault.kind {
        FaultKind::Orpheus { origin, .. } => {
            Some(format!("this shade came from `{}` at stratum {origin}", blame.what))
        }
        FaultKind::Ungranted { needed, .. } => {
            Some(format!("`{}` reaches stratum {needed}", blame.what))
        }
        _ => None,
    }
}

/// The line after the gap, which says what to do rather than what is wrong.
fn note(fault: &Fault) -> Option<String> {
    match fault.kind {
        FaultKind::Orpheus { origin, .. } => Capability::at(origin).map(|c| {
            format!("the value is here, but you are not. Wrap the look in `descend {c} {{ … }}`.")
        }),
        FaultKind::Ungranted { needed, .. } => {
            Capability::at(needed).map(|c| format!("wrap it in `descend {c} {{ … }}`."))
        }
        FaultKind::Asserted { derived, .. } => {
            Some(format!("inference is not a coercion. Write `@{derived}`, or write nothing."))
        }
        FaultKind::Stated { .. } | FaultKind::Malformed(_) => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::check::Blame;
    use crate::depth::Depth;
    use crate::ir::Span;

    #[test]
    fn the_gutter_widens_with_the_line_number() {
        let source = "x\n".repeat(200);
        let fault = Fault {
            span: Span { start: 200, end: 201 },
            kind: FaultKind::Stated { derived: Depth::PURE, stated: Depth::DISK },
            blame: None,
        };
        let out = report(&fault, &source, "wide.nc");
        assert!(out.contains("  --> wide.nc:101:1"), "{out}");
        assert!(out.contains("101 | x"), "{out}");
    }

    #[test]
    fn a_fault_with_nothing_to_suggest_stops_after_the_carets() {
        let fault = Fault {
            span: Span { start: 0, end: 1 },
            kind: FaultKind::Malformed("this local is not bound here"),
            blame: None,
        };
        let out = report(&fault, "a\n", "x.nc");
        assert!(!out.contains(" = "), "{out}");
        assert_eq!(out.lines().count(), 5, "{out}");
    }

    #[test]
    fn a_zero_width_span_still_gets_one_caret() {
        let fault = Fault {
            span: Span::default(),
            kind: FaultKind::Malformed("nowhere in particular"),
            blame: None,
        };
        let out = report(&fault, "abc\n", "x.nc");
        assert!(out.contains("| ^"), "{out}");
    }

    #[test]
    fn blame_that_the_kind_has_no_use_for_is_not_printed() {
        let fault = Fault {
            span: Span { start: 0, end: 1 },
            kind: FaultKind::Stated { derived: Depth::PURE, stated: Depth::DISK },
            blame: Some(Blame { span: Span::default(), what: "get".into() }),
        };
        assert!(!report(&fault, "a\n", "x.nc").contains("get"));
    }
}
