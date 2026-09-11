//! Cairns: the name a value has by virtue of being that value.

use core::fmt;
use core::str::FromStr;

/// Domain separator, versioned with the encoding it names.
///
/// Changing the encoding changes this, which changes every cairn that has ever
/// existed. That is the correct and honest consequence: values encoded under
/// different rules are not the same values. `spec/07-ledger.md` §7.2.
const DOMAIN: &[u8] = b"netherc/cairn/v1\x00";

/// The number of hexadecimal characters in the recommended short form.
pub const SHORT_LEN: usize = 8;

/// The content address of a value: `blake3(DOMAIN || encode(value))`.
///
/// A cairn is a name, and a name is pure however deep the thing it names lies.
/// That is why `seal` yields depth 0 in the language itself.
#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct Cairn([u8; 32]);

impl Cairn {
    /// Names the already-encoded bytes of a value.
    ///
    /// Takes the encoding rather than the value so that nothing can be named
    /// twice by two different routes.
    #[must_use]
    pub fn of_encoded(encoded: &[u8]) -> Self {
        let mut hasher = blake3::Hasher::new();
        hasher.update(DOMAIN);
        hasher.update(encoded);
        Self(*hasher.finalize().as_bytes())
    }

    /// The raw digest.
    #[must_use]
    pub const fn as_bytes(&self) -> &[u8; 32] {
        &self.0
    }

    /// Reconstructs a cairn from a digest that is already known to be one.
    #[must_use]
    pub const fn from_bytes(bytes: [u8; 32]) -> Self {
        Self(bytes)
    }

    /// The first eight hexadecimal characters, for human output.
    ///
    /// A tool that accepts a short form must reject an ambiguous one rather
    /// than picking a match. `spec/07-ledger.md` §7.2.
    #[must_use]
    pub fn short(&self) -> String {
        self.to_string()[..SHORT_LEN].to_owned()
    }
}

impl fmt::Display for Cairn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        for byte in self.0 {
            write!(f, "{byte:02x}")?;
        }
        Ok(())
    }
}

impl fmt::Debug for Cairn {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Cairn({})", self.short())
    }
}

/// Why a string was not a cairn.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseCairnError {
    /// A cairn is exactly sixty-four hexadecimal characters.
    WrongLength(usize),
    /// Cairns are lowercase hexadecimal, and only that.
    NotLowercaseHex,
}

impl fmt::Display for ParseCairnError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::WrongLength(n) => write!(f, "a cairn is 64 hex characters, this is {n}"),
            Self::NotLowercaseHex => f.write_str("a cairn is lowercase hexadecimal"),
        }
    }
}

impl core::error::Error for ParseCairnError {}

/// One lowercase hexadecimal digit, or a refusal.
fn nibble(c: u8) -> Result<u8, ParseCairnError> {
    match c {
        b'0'..=b'9' => Ok(c - b'0'),
        b'a'..=b'f' => Ok(c - b'a' + 10),
        _ => Err(ParseCairnError::NotLowercaseHex),
    }
}

impl FromStr for Cairn {
    type Err = ParseCairnError;

    /// Strict: sixty-four lowercase hexadecimal characters, nothing else.
    ///
    /// Uppercase is rejected rather than folded. Two spellings of one name is
    /// the beginning of two names.
    ///
    /// # Errors
    ///
    /// [`ParseCairnError`] when the length is wrong or a character is not
    /// lowercase hexadecimal.
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Bytes throughout. `str::len` is a byte count, so slicing by `i * 2`
        // after a length check on it is only sound while every character is
        // one byte, and a 64-byte string containing a multi-byte character
        // used to panic on a boundary instead of being rejected.
        let raw = s.as_bytes();
        if raw.len() != 64 {
            return Err(ParseCairnError::WrongLength(raw.len()));
        }
        let mut out = [0u8; 32];
        for (i, byte) in out.iter_mut().enumerate() {
            *byte = (nibble(raw[i * 2])? << 4) | nibble(raw[i * 2 + 1])?;
        }
        Ok(Self(out))
    }
}

#[cfg(test)]
mod tests {
    use super::{Cairn, ParseCairnError, SHORT_LEN};

    #[test]
    fn text_form_round_trips() {
        for seed in 0u8..=255 {
            let cairn = Cairn::of_encoded(&[seed, seed.wrapping_mul(7)]);
            let text = cairn.to_string();
            assert_eq!(text.len(), 64);
            assert_eq!(text.parse::<Cairn>().unwrap(), cairn);
        }
    }

    #[test]
    fn the_domain_separator_is_applied() {
        // Naming the empty encoding must not be naming nothing.
        let bare = blake3::hash(b"");
        assert_ne!(Cairn::of_encoded(b"").as_bytes(), bare.as_bytes());
    }

    #[test]
    fn naming_is_a_function_of_the_bytes_alone() {
        assert_eq!(Cairn::of_encoded(b"abc"), Cairn::of_encoded(b"abc"));
        assert_ne!(Cairn::of_encoded(b"abc"), Cairn::of_encoded(b"abd"));
    }

    #[test]
    fn uppercase_is_rejected_rather_than_folded() {
        let text = Cairn::of_encoded(b"x").to_string().to_uppercase();
        assert_eq!(text.parse::<Cairn>(), Err(ParseCairnError::NotLowercaseHex));
    }

    #[test]
    fn short_form_is_a_prefix_of_the_long_one() {
        let cairn = Cairn::of_encoded(b"anything");
        assert_eq!(cairn.short().len(), SHORT_LEN);
        assert!(cairn.to_string().starts_with(&cairn.short()));
    }

    /// Found by review. `str::len` is a byte count and the loop sliced by byte
    /// index, so a 64-byte string with a multi-byte character panicked on a
    /// boundary rather than being refused. `FromStr` is how every rite takes a
    /// cairn from a user, and SECURITY.md puts panics on untrusted input in
    /// scope.
    #[test]
    fn a_64_byte_string_that_is_not_64_characters_is_refused() {
        for s in ["\u{20ac}".to_owned() + &"a".repeat(61), "é".repeat(32), "\u{1f480}".repeat(16)]
        {
            assert_eq!(s.len(), 64, "the probe itself must be 64 bytes");
            assert_eq!(s.parse::<Cairn>(), Err(ParseCairnError::NotLowercaseHex));
        }
    }

    #[test]
    fn every_byte_outside_lowercase_hex_is_refused() {
        let good = Cairn::of_encoded(b"x").to_string();
        for bad in [b'g', b'G', b'A', b'/', b':', b'@', 0x00, 0x7f] {
            let mut raw = good.clone().into_bytes();
            raw[7] = bad;
            let s = String::from_utf8(raw).unwrap();
            assert_eq!(
                s.parse::<Cairn>(),
                Err(ParseCairnError::NotLowercaseHex),
                "accepted {bad:#04x}"
            );
        }
    }

    #[test]
    fn wrong_lengths_are_rejected() {
        assert_eq!("".parse::<Cairn>(), Err(ParseCairnError::WrongLength(0)));
        assert_eq!("ab".parse::<Cairn>(), Err(ParseCairnError::WrongLength(2)));
        let long = "a".repeat(65);
        assert_eq!(long.parse::<Cairn>(), Err(ParseCairnError::WrongLength(65)));
    }
}
