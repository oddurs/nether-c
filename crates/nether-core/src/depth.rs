//! Depth, the lattice it lives in, and the capabilities that raise it.
//!
//! `spec/01-strata.md` §1.1.

use core::fmt;

/// How far into the world a value's history reaches: `0..=8`.
///
/// The nine strata are a total order and compose by maximum, so the lattice is
/// [`Ord`] and the join is [`Depth::join`]. Nothing in this crate can produce a
/// depth outside the range, which is what lets the ledger encode one in a byte.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Depth(u8);

impl Depth {
    /// Arithmetic, data, functions. Nothing is owed.
    pub const PURE: Self = Self(0);
    /// Reading the ledger by cairn.
    pub const STORE: Self = Self(1);
    /// The frozen environment: declared variables, a pinned clock, the target.
    pub const ENV: Self = Self(2);
    /// Reading files.
    pub const DISK: Self = Self(3);
    /// Creating and modifying files.
    pub const DISK_WRITE: Self = Self(4);
    /// Fetching.
    pub const NET: Self = Self(5);
    /// Sending.
    pub const NET_WRITE: Self = Self(6);
    /// True randomness.
    pub const ENTROPY: Self = Self(7);
    /// Foreign code, and the one stratum that cannot be recorded.
    pub const UNRECORDED: Self = Self(8);

    /// The deepest stratum there is.
    pub const MAX: Self = Self::UNRECORDED;

    /// A depth, if `n` names a stratum.
    #[must_use]
    pub const fn new(n: u8) -> Option<Self> {
        if n <= Self::MAX.0 { Some(Self(n)) } else { None }
    }

    /// The stratum number.
    #[must_use]
    pub const fn get(self) -> u8 {
        self.0
    }

    /// The deeper of two depths.
    ///
    /// This is the whole of composition: an expression built from parts is
    /// exactly as deep as its deepest part. `spec/02-calculus.md` [PRIM].
    #[must_use]
    pub const fn join(self, other: Self) -> Self {
        if self.0 >= other.0 { self } else { other }
    }

    /// Whether everything this depth reached can be written down and served
    /// back. Everything but stratum 8. `spec/01-strata.md` §1.7.
    #[must_use]
    pub const fn is_recordable(self) -> bool {
        self.0 < Self::UNRECORDED.0
    }
}

impl fmt::Display for Depth {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}

/// A name `descend` accepts.
///
/// The set is closed and fixed by `spec/09-prelude.md` §9.1. A capability that
/// is not in the prelude cannot be audited by a reader of the specification,
/// which is the entire reason for naming them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum Capability {
    /// `store` — reading the ledger by cairn.
    Store,
    /// `env` — the frozen environment.
    Env,
    /// `disk` — reading files.
    Disk,
    /// `disk!` — creating and modifying files.
    DiskWrite,
    /// `net` — fetching.
    Net,
    /// `net!` — sending.
    NetWrite,
    /// `entropy` — true randomness.
    Entropy,
    /// `unrecorded` — foreign code.
    Unrecorded,
}

impl Capability {
    /// Every capability, shallowest first.
    pub const ALL: [Self; 8] = [
        Self::Store,
        Self::Env,
        Self::Disk,
        Self::DiskWrite,
        Self::Net,
        Self::NetWrite,
        Self::Entropy,
        Self::Unrecorded,
    ];

    /// `s(κ)`: the stratum this capability grants.
    #[must_use]
    pub const fn stratum(self) -> Depth {
        match self {
            Self::Store => Depth::STORE,
            Self::Env => Depth::ENV,
            Self::Disk => Depth::DISK,
            Self::DiskWrite => Depth::DISK_WRITE,
            Self::Net => Depth::NET,
            Self::NetWrite => Depth::NET_WRITE,
            Self::Entropy => Depth::ENTROPY,
            Self::Unrecorded => Depth::UNRECORDED,
        }
    }

    /// The name it is written with.
    ///
    /// The `!` suffix marks the writing half of a pair, and reads as it does
    /// everywhere else in the language: this one does not take back.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Store => "store",
            Self::Env => "env",
            Self::Disk => "disk",
            Self::DiskWrite => "disk!",
            Self::Net => "net",
            Self::NetWrite => "net!",
            Self::Entropy => "entropy",
            Self::Unrecorded => "unrecorded",
        }
    }

    /// The capability of that name, if there is one.
    #[must_use]
    pub fn from_name(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|c| c.name() == name)
    }
}

impl fmt::Display for Capability {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.name())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_lattice_is_a_total_order_of_nine() {
        assert_eq!(Depth::new(9), None);
        assert_eq!(Depth::new(8), Some(Depth::UNRECORDED));
        assert!(Depth::PURE < Depth::STORE && Depth::ENTROPY < Depth::UNRECORDED);
    }

    #[test]
    fn join_is_the_deeper_of_the_two() {
        assert_eq!(Depth::DISK.join(Depth::PURE), Depth::DISK);
        assert_eq!(Depth::PURE.join(Depth::DISK), Depth::DISK);
        assert_eq!(Depth::NET.join(Depth::NET), Depth::NET);
    }

    #[test]
    fn only_stratum_eight_is_unrecordable() {
        let unrecordable: Vec<_> =
            (0..=8).filter(|n| !Depth::new(*n).unwrap().is_recordable()).collect();
        assert_eq!(unrecordable, vec![8]);
    }

    #[test]
    fn capability_names_round_trip() {
        for c in Capability::ALL {
            assert_eq!(Capability::from_name(c.name()), Some(c));
        }
        assert_eq!(Capability::from_name("disk!"), Some(Capability::DiskWrite));
        assert_eq!(Capability::from_name("sudo"), None);
    }

    #[test]
    fn capabilities_are_listed_shallowest_first() {
        let strata: Vec<u8> = Capability::ALL.iter().map(|c| c.stratum().get()).collect();
        assert_eq!(strata, vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }
}
