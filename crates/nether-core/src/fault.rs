//! Checker faults, blame, and diagnostic rendering.

use core::fmt;

use crate::depth::{Capability, Depth, Held};
use crate::ir::Span;
use crate::report::{Cause, Diagnostic};

/// Something the calculus does not derive.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Fault {
    /// Where it was written.
    pub span: Span,
    /// What is wrong.
    pub kind: FaultKind,
    /// What made the value as deep as it is, when that is a different place
    /// from the fault and worth pointing at.
    pub blame: Option<Blame>,
}

/// The call that took a value to the stratum it is at.
///
/// §2.4 promises this is findable by construction: a value at depth 5 means
/// some `descend net` is responsible. This is the call inside it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Blame {
    /// Where the call is.
    pub span: Span,
    /// What it called.
    pub what: String,
}

/// What kind of thing is wrong.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FaultKind {
    /// [APP]: an application reaches deeper than what is held here.
    Ungranted {
        /// `dƒ`, the latent depth of the arrow: what applying it reaches.
        needed: Depth,
        /// `δ`.
        held: Held,
    },
    /// [LOOK]: the Orpheus rule. You may carry a shade up out of any stratum;
    /// you may only look at it by going back down.
    Orpheus {
        /// The stratum the shade came from.
        origin: Depth,
        /// `δ`, which does not hold it.
        held: Held,
    },
    /// The IR states a latent set the eleven rules do not produce. [ABS]'s
    /// half of `Stated`, and not a mistake a program can make.
    StatedLatent {
        /// What the rules give.
        derived: Held,
        /// What the node claims.
        stated: Held,
    },
    /// A latent set written after a signature that inference disagrees with.
    /// [ABS]'s half of `Asserted`. §5.6: a written depth is a checked
    /// assertion, not a coercion.
    AssertedLatent {
        /// What the rules give.
        derived: Held,
        /// What the programmer wrote.
        asserted: Held,
    },
    /// The IR states a depth the eleven rules do not produce. Not a mistake a
    /// program can make: a mistake whatever built the IR made.
    Stated {
        /// What the rules give.
        derived: Depth,
        /// What the node claims.
        stated: Depth,
    },
    /// A written `@d` disagrees with inference. An assertion, never a
    /// coercion. `spec/05-types.md` §5.6.
    Asserted {
        /// What the rules give.
        derived: Depth,
        /// What was written.
        asserted: Depth,
    },
    /// The IR is not well-formed enough to have a judgement at all.
    Malformed(&'static str),
    /// A local was assigned after its value had been named.
    ///
    /// Once a value has been used as a value — sealed, shaded, deposited,
    /// returned, or passed as an argument — its cairn exists, and nothing can
    /// change what a cairn names. `spec/05-types.md` §5.4.
    Frozen {
        /// Where it was named. The interesting place is almost never the
        /// assignment; it is the line that gave the value a name.
        named: Span,
    },
}

/// Deepest stratum in `derived` absent from `asserted`, or `derived`'s deepest.
pub(crate) fn extra_stratum(derived: Held, asserted: Held) -> Depth {
    let extra = derived.without(asserted);
    if extra.is_none() { derived.deepest() } else { extra.deepest() }
}

impl Fault {
    /// This fault, ready to print.
    #[must_use]
    pub fn diagnostic(&self) -> Diagnostic {
        Diagnostic {
            span: self.span,
            headline: self.to_string(),
            label: self.label(),
            cause: self.cause(),
            note: self.note(),
        }
    }

    /// What goes at the end of the caret row, about the thing underlined.
    fn label(&self) -> Option<String> {
        let blame = self.blame.as_ref()?;
        match self.kind {
            FaultKind::Orpheus { origin, .. } => {
                Some(format!("this shade came from `{}` at stratum {origin}", blame.what))
            }
            FaultKind::Ungranted { needed, .. } => {
                Some(format!("`{}` reaches stratum {needed}", blame.what))
            }
            _ => None,
        }
    }

    /// The other end of the error: the line the depth came from.
    ///
    /// Only an assertion gets one. The other two that carry blame say it on
    /// the caret row, because for them the cause and the symptom are the same
    /// expression; an annotation is about a number that came from somewhere
    /// else, which is the whole reason it is hard to phrase.
    fn cause(&self) -> Option<Cause> {
        if let FaultKind::Frozen { named } = self.kind {
            return Some(Cause { span: named, label: "named here".to_string() });
        }
        let blame = self.blame.as_ref()?;
        let stratum = match self.kind {
            FaultKind::Asserted { derived, .. } => derived,
            FaultKind::AssertedLatent { derived, asserted } => extra_stratum(derived, asserted),
            _ => return None,
        };
        Some(Cause {
            span: blame.span,
            label: format!("`{}` reaches stratum {stratum}", blame.what),
        })
    }

    /// The line after the gap, which says what to do rather than what is wrong.
    fn note(&self) -> Option<String> {
        match self.kind {
            FaultKind::Orpheus { origin, .. } => Capability::at(origin).map(|c| {
                format!(
                    "the value is here, but you are not. Wrap the look in `descend {c} {{ … }}`."
                )
            }),
            FaultKind::Ungranted { needed, .. } => {
                Capability::at(needed).map(|c| format!("wrap it in `descend {c} {{ … }}`."))
            }
            FaultKind::Asserted { derived, .. } => {
                Some(format!("inference is not a coercion. Write `@{derived}`, or write nothing."))
            }
            FaultKind::AssertedLatent { derived, .. } => {
                Some(format!("inference is not a coercion. Write `@{derived}`, or write nothing."))
            }
            FaultKind::Frozen { .. } => Some(
                "a value that has been named cannot change; make another one instead.".to_string(),
            ),
            FaultKind::Stated { .. } | FaultKind::StatedLatent { .. } | FaultKind::Malformed(_) => {
                None
            }
        }
    }
}

impl fmt::Display for Fault {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self.kind {
            FaultKind::Ungranted { needed, held } if held.is_none() => {
                write!(f, "this reaches stratum {needed}, and depth 0 is held here")
            }
            FaultKind::Ungranted { needed, held } => match Capability::at(needed) {
                Some(c) => write!(f, "this reaches stratum {needed}; `{c}` is not in {held}"),
                None => write!(f, "this reaches stratum {needed}, which is not in {held}"),
            },
            // §1.6: where something is held and it is the wrong one, say so
            // rather than report a number. A shade from stratum 3 looked at
            // under `descend net` is not a shallower scope, and "at depth 5"
            // would tell the reader their scope is deep enough.
            FaultKind::Orpheus { origin, held } if held.is_none() => {
                write!(f, "cannot look at a shade from stratum {origin} at depth 0")
            }
            FaultKind::Orpheus { origin, held } => {
                write!(f, "cannot look at a shade from stratum {origin}; {held} is held here")
            }
            FaultKind::Stated { derived, stated } => {
                write!(f, "the rules give depth {derived}; this node says {stated}")
            }
            FaultKind::StatedLatent { derived, stated } => {
                write!(f, "the rules give @{derived}; this node says @{stated}")
            }
            FaultKind::Asserted { derived, asserted } => {
                write!(f, "annotated @{asserted}; inference gives {derived}")
            }
            FaultKind::AssertedLatent { derived, asserted } => {
                write!(f, "annotated @{asserted}; inference gives @{derived}")
            }
            FaultKind::Frozen { .. } => {
                f.write_str("this was named, and a name cannot change what it names")
            }
            FaultKind::Malformed(why) => f.write_str(why),
        }
    }
}
