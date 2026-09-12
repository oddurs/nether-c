//! The prelude, as the IR names it.
//!
//! `spec/09-prelude.md`. The set is closed: these are the only functions a
//! program does not write itself, and the only ones a hole can ask the world
//! about (`spec/06-evaluation.md` §6.3).
//!
//! Only the name and the latent depth are here. A prelude signature is the
//! checker's business, and several of them are polymorphic in a way the IR's
//! [`Type`](crate::Type) deliberately is not.

use core::fmt;

use crate::depth::{Capability, Depth};

/// A prelude function.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Prim {
    /// `I64 min(I64, I64)`
    Min,
    /// `I64 max(I64, I64)`
    Max,
    /// `I64 abs(I64)`
    Abs,
    /// `I64 len(Bytes)`
    Len,
    /// `Bytes slice(Bytes, I64, I64)`
    Slice,
    /// `Bytes concat(Bytes, Bytes)`
    Concat,
    /// `Bool starts_with(Bytes, Bytes)`
    StartsWith,
    /// `Answer<Str> utf8(Bytes)`
    Utf8,
    /// `Bytes raw(Str)`
    Raw,
    /// `Str join(Str[], Str)`
    Join,
    /// `Str[] split(Str, Str)`
    Split,
    /// `Cairn cairn_of(Bytes)` — the same value `seal` would give.
    CairnOf,
    /// `Str hex(Cairn)`
    Hex,
    /// `Bool given(Answer<T>)` — did the world say yes?
    Given,
    /// `Refusal refusal(Answer<T>)` — which no was it? Starves on a yes.
    Refusal,
    /// `T must(Answer<T>)` — the value. Starves on a no.
    Must,
    /// `Answer<Bytes> fetch_node(Cairn)`
    FetchNode,
    /// `Bool has_node(Cairn)`
    HasNode,
    /// `Answer<Str> env(Str)`
    Env,
    /// `I64 clock()` — the pinned build time.
    Clock,
    /// `Str target()` — the target triple.
    Target,
    /// `Answer<Bytes> read(Str)`
    Read,
    /// `Answer<Str[]> list(Str)`
    List,
    /// `Bool exists(Str)`
    Exists,
    /// `Answer<U0> write(Str, Bytes)` — the first genuinely irreversible thing.
    Write,
    /// `Answer<U0> remove(Str)`
    Remove,
    /// `Answer<Bytes> get(Str)`
    Get,
    /// `Answer<Bytes> post(Str, Bytes)`
    Post,
    /// `Bytes draw(I64)` — the only source of nondeterminism in the language.
    Draw,
    /// `Answer<Bytes> call_foreign(Str, Bytes)` — marks the trace, for good.
    CallForeign,
}

impl Prim {
    /// Every prelude function, in the order `spec/09-prelude.md` lists them.
    pub const ALL: [Self; 30] = [
        Self::Min,
        Self::Max,
        Self::Abs,
        Self::Len,
        Self::Slice,
        Self::Concat,
        Self::StartsWith,
        Self::Utf8,
        Self::Raw,
        Self::Join,
        Self::Split,
        Self::CairnOf,
        Self::Hex,
        Self::Given,
        Self::Refusal,
        Self::Must,
        Self::FetchNode,
        Self::HasNode,
        Self::Env,
        Self::Clock,
        Self::Target,
        Self::Read,
        Self::List,
        Self::Exists,
        Self::Write,
        Self::Remove,
        Self::Get,
        Self::Post,
        Self::Draw,
        Self::CallForeign,
    ];

    /// The name it is written with, and the deepest stratum applying it
    /// reaches.
    #[must_use]
    pub const fn signature(self) -> (&'static str, Depth) {
        match self {
            Self::Min => ("min", Depth::PURE),
            Self::Max => ("max", Depth::PURE),
            Self::Abs => ("abs", Depth::PURE),
            Self::Len => ("len", Depth::PURE),
            Self::Slice => ("slice", Depth::PURE),
            Self::Concat => ("concat", Depth::PURE),
            Self::StartsWith => ("starts_with", Depth::PURE),
            Self::Utf8 => ("utf8", Depth::PURE),
            Self::Raw => ("raw", Depth::PURE),
            Self::Join => ("join", Depth::PURE),
            Self::Split => ("split", Depth::PURE),
            Self::CairnOf => ("cairn_of", Depth::PURE),
            Self::Hex => ("hex", Depth::PURE),
            Self::Given => ("given", Depth::PURE),
            Self::Refusal => ("refusal", Depth::PURE),
            Self::Must => ("must", Depth::PURE),
            Self::FetchNode => ("fetch_node", Depth::STORE),
            Self::HasNode => ("has_node", Depth::STORE),
            Self::Env => ("env", Depth::ENV),
            Self::Clock => ("clock", Depth::ENV),
            Self::Target => ("target", Depth::ENV),
            Self::Read => ("read", Depth::DISK),
            Self::List => ("list", Depth::DISK),
            Self::Exists => ("exists", Depth::DISK),
            Self::Write => ("write", Depth::DISK_WRITE),
            Self::Remove => ("remove", Depth::DISK_WRITE),
            Self::Get => ("get", Depth::NET),
            Self::Post => ("post", Depth::NET_WRITE),
            Self::Draw => ("draw", Depth::ENTROPY),
            Self::CallForeign => ("call_foreign", Depth::UNRECORDED),
        }
    }

    /// The name it is written with.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.signature().0
    }

    /// The deepest stratum applying it reaches: `dƒ` in [APP].
    #[must_use]
    pub const fn latent(self) -> Depth {
        self.signature().1
    }

    /// The capability a call to this must be inside, if any.
    ///
    /// Derived rather than stored: the capabilities are exactly the strata
    /// below 0, so a second table could only ever disagree with the first.
    #[must_use]
    pub fn capability(self) -> Option<Capability> {
        Capability::ALL.into_iter().find(|c| c.stratum() == self.latent())
    }

    /// The prelude function of that name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|p| p.name() == name)
    }
}

impl fmt::Display for Prim {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names_round_trip_and_are_distinct() {
        for p in Prim::ALL {
            assert_eq!(Prim::from_name(p.name()), Some(p));
        }
        let mut names: Vec<_> = Prim::ALL.iter().map(|p| p.name()).collect();
        names.sort_unstable();
        let before = names.len();
        names.dedup();
        assert_eq!(names.len(), before, "two prelude functions share a name");
    }

    #[test]
    fn compile_is_not_in_the_prelude() {
        // Nether C does not know how to compile Nether C. Until then it is an
        // ordinary function a program supplies for itself.
        assert_eq!(Prim::from_name("compile"), None);
    }

    #[test]
    fn a_pure_prim_needs_no_capability() {
        assert_eq!(Prim::Concat.capability(), None);
        assert_eq!(Prim::Read.capability(), Some(Capability::Disk));
        assert_eq!(Prim::Write.capability(), Some(Capability::DiskWrite));
        assert_eq!(Prim::CallForeign.capability(), Some(Capability::Unrecorded));
    }

    #[test]
    fn every_capability_is_reachable_by_some_prim() {
        for c in Capability::ALL {
            assert!(
                Prim::ALL.iter().any(|p| p.capability() == Some(c)),
                "nothing in the prelude reaches `{c}`, so descending into it is pointless"
            );
        }
    }
}
