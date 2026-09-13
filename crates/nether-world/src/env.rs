//! Stratum 2: the frozen environment.
//!
//! `spec/09-prelude.md` §9.4, and §8.3.1 for where the declaration comes from.
//! Three questions with pinned answers: a declared variable, the build time,
//! and the target triple.
//!
//! Nothing here reads the process environment, the wall clock or the host
//! triple. A rite whose answer depends on which machine ran it is the thing
//! §6.7 exists to prevent, and a provider that fills a gap in from the host is
//! the ambient authority §9.1 refuses in a place nobody would look for it.

use std::collections::BTreeMap;

use nether_core::{Capability, Depth};
use nether_ledger::{Call, Refusal, Span, Stored, Value};

use crate::provider::{Provider, Refuse, given, refused};
use crate::recorder::{Recorded, Recorder};

/// What §8.3.1's flags declared.
///
/// Sorted, because the declaration is read back in tests and in error
/// messages, and a map with an order nobody chose is an order that will
/// eventually be depended on.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
pub struct Declared {
    named: BTreeMap<String, String>,
}

/// A name declared twice. §8.3.1: an error, not a last-one-wins, because the
/// two invocations differ and only one of them can be what was meant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Twice(pub String);

impl core::fmt::Display for Twice {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(f, "`{}` is declared twice", self.0)
    }
}

impl core::error::Error for Twice {}

impl Declared {
    /// Nothing declared: an environment in which every name is undeclared.
    /// §8.3.1 makes this legal, and it is not the same as no `env` grant.
    #[must_use]
    pub fn none() -> Self {
        Self::default()
    }

    /// Declare one name. An empty value declares it and leaves it unset,
    /// which is the refusal §9.4 distinguishes from the collapse.
    ///
    /// # Errors
    ///
    /// [`Twice`] if that name was already declared.
    pub fn and(mut self, name: impl Into<String>, value: impl Into<String>) -> Result<Self, Twice> {
        let name = name.into();
        if self.named.contains_key(&name) {
            return Err(Twice(name));
        }
        self.named.insert(name, value.into());
        Ok(self)
    }

    /// What that name is, if it was declared at all.
    #[must_use]
    pub fn of(&self, name: &str) -> Option<&str> {
        self.named.get(name).map(String::as_str)
    }
}

/// The frozen environment, as one exhumation declared it.
pub struct Env {
    declared: Declared,
    /// Whole seconds since the Unix epoch. §8.3.1: given, never read.
    clock: i64,
    /// The target triple. Given, never the host's.
    target: String,
}

impl Env {
    /// An environment pinned to that declaration, clock and triple.
    #[must_use]
    pub fn pinned(declared: Declared, clock: i64, target: impl Into<String>) -> Self {
        Self { declared, clock, target: target.into() }
    }
}

/// The one `Str` argument `env` takes.
fn asked(into: &Recorder, call: &Call) -> Option<String> {
    match into.store().get(*call.args.first()?) {
        Ok(Stored::Value(Value::Str(s))) => Some(s),
        _ => None,
    }
}

impl Provider for Env {
    fn capability(&self) -> Capability {
        Capability::Env
    }

    fn answers(&self, function: &str) -> bool {
        matches!(function, "env" | "clock" | "target")
    }

    fn answer(&self, call: &Call, span: Span, into: &Recorder) -> Result<Recorded, Refuse> {
        let said = match call.function.as_str() {
            "clock" => given(&call.function, Value::Int(self.clock)),
            "target" => given(&call.function, Value::Str(self.target.clone())),
            // §9.4 distinguishes two mistakes, and this is the only place
            // that can tell them apart. Declared and unset is a refusal: the
            // world was asked and said no, and that is an answer. Never
            // declared is a bug in the program, and §9.9 makes it a collapse
            // — not an empty string, which is the class of bug this language
            // exists to make impossible.
            _ => {
                let name = asked(into, call).unwrap_or_default();
                match self.declared.of(&name) {
                    Some("") => refused(Refusal::Absent),
                    Some(value) => given(&call.function, Value::Str(value.to_owned())),
                    None => {
                        return Err(Refuse::Collapse(format!(
                            "`{name}` was never declared, so there is nothing for `env` to say"
                        )));
                    }
                }
            }
        };
        into.record(call, Depth::ENV, span, &said)
    }
}
