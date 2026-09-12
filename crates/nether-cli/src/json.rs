//! Just enough JSON to answer `--json`.
//!
//! §8.1 requires every rite to accept it and write one object. What that takes
//! is a string escaper, so that is what is here — a serialisation library for
//! six fields would be a dependency for something smaller than the dependency.

/// A string, quoted and escaped, as JSON spells one.
#[must_use]
pub fn string(s: &str) -> String {
    let mut out = String::with_capacity(s.len() + 2);
    out.push('"');
    for c in s.chars() {
        match c {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\r' => out.push_str("\\r"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                use core::fmt::Write as _;
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::string;

    #[test]
    fn what_has_to_be_escaped_is() {
        assert_eq!(string("plain"), "\"plain\"");
        assert_eq!(string("a\"b"), "\"a\\\"b\"");
        assert_eq!(string("a\\b"), "\"a\\\\b\"");
        assert_eq!(string("a\nb"), "\"a\\nb\"");
        assert_eq!(string("a\u{1}b"), "\"a\\u0001b\"");
        // Anything above the control range is itself: the output is UTF-8.
        assert_eq!(string("café 💀"), "\"café 💀\"");
    }
}
