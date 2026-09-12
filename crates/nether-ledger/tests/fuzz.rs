//! Fuzzing the decoder.
//!
//! The decoder parses bytes from a store that may be shared between people who
//! do not trust each other. It is the one genuinely hostile surface in the
//! crate, and `SECURITY.md` says so.
//!
//! Two properties, held over every input:
//!
//! 1. **It does not panic.** Any byte string is an answer or an error, never a
//!    crash. A panicking decoder is a denial of service in a shared store.
//! 2. **It does not accept non-canonical input.** If `decode(b)` succeeds with
//!    `v`, then `encode(v)` is `b`. Accepting two spellings of one value is how
//!    a content-addressed store starts holding two names for one thing, and it
//!    would be found years later by somebody who cannot fix it.
//!
//! The generator is seeded and deterministic, so a failure here is reproducible
//! from the seed printed with it. `scripts/task fuzz` runs it for longer.

use nether_ledger::{Cairn, Call, Node, Span, Value, decode_stored};

/// xorshift64*. Five lines, deterministic, and nothing to install.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        if n == 0 {
            return 0;
        }
        // usize::try_from cannot fail here: the modulus bounds it by `n`.
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }

    fn byte(&mut self) -> u8 {
        (self.next() & 0xff) as u8
    }
}

/// The one law. Returns the complaint if it is broken.
fn check(bytes: &[u8]) -> Option<String> {
    let Ok(decoded) = decode_stored(bytes) else { return None };
    let re = decoded.encode();
    if re == bytes {
        return None;
    }
    Some(format!("accepted non-canonical input\n    in:  {}\n    out: {}", hex(bytes), hex(&re)))
}

fn hex(bytes: &[u8]) -> String {
    use core::fmt::Write as _;
    bytes.iter().take(64).fold(String::new(), |mut acc, b| {
        let _ = write!(acc, "{b:02x}");
        acc
    })
}

/// A corpus worth mutating: every tag and every node kind.
fn corpus() -> Vec<Vec<u8>> {
    let c = Cairn::of_encoded(b"seed");
    let span = Span { source: c, start: 0, end: 3 };
    let call = Call { function: "read".to_owned(), args: vec![c] };

    let values = vec![
        Value::Unit,
        Value::Bool(true),
        Value::Int(-9_000),
        Value::Bytes(vec![1, 2, 3, 4]),
        Value::Str("café".to_owned()),
        Value::Cairn(c),
        Value::Shade { origin: 4, value: c },
        Value::Struct { name: "Header".to_owned(), fields: vec![Value::Int(1), Value::Unit] },
        Value::Array(vec![Value::Int(1), Value::Array(vec![Value::Unit])]),
    ];
    let nodes = vec![
        Node::Literal(Value::Int(3)),
        Node::Apply { function: c, args: vec![c, c], result: c },
        Node::Hole { call: call.clone(), stratum: 3, span },
        Node::Deposit { value: c, span },
        Node::Witness { stratum: 5, call, answer: c, span },
        Node::Trace {
            residue: c,
            holes: vec![c],
            deposits: vec![c],
            source: c,
            fuel_spent: 12,
            depth: 3,
            unrecorded: true,
        },
    ];

    values.into_iter().map(|v| v.encode()).chain(nodes.into_iter().map(|n| n.encode())).collect()
}

fn mutate(rng: &mut Rng, seed: &[u8]) -> Vec<u8> {
    let mut out = seed.to_vec();
    match rng.below(6) {
        0 if !out.is_empty() => {
            let at = rng.below(out.len());
            out[at] ^= 1 << rng.below(8);
        }
        1 if !out.is_empty() => {
            let at = rng.below(out.len());
            out[at] = rng.byte();
        }
        2 if !out.is_empty() => out.truncate(rng.below(out.len())),
        3 => {
            let at = rng.below(out.len() + 1);
            out.insert(at, rng.byte());
        }
        4 if !out.is_empty() => {
            let at = rng.below(out.len());
            out.remove(at);
        }
        _ => out.extend((0..rng.below(8)).map(|_| rng.byte())),
    }
    out
}

fn run(seed: u64, rounds: usize) -> (usize, usize) {
    let mut rng = Rng(seed);
    let corpus = corpus();
    let mut accepted = 0usize;

    for i in 0..rounds {
        let bytes = if i % 3 == 0 {
            // Arbitrary bytes: mostly rejected, and the reason it must not panic.
            (0..rng.below(48)).map(|_| rng.byte()).collect::<Vec<u8>>()
        } else {
            let base = &corpus[rng.below(corpus.len())];
            let mut b = mutate(&mut rng, base);
            if rng.below(4) == 0 {
                b = mutate(&mut rng, &b);
            }
            b
        };

        if decode_stored(&bytes).is_ok() {
            accepted += 1;
        }
        assert!(check(&bytes).is_none(), "seed {seed}, round {i}: {}", check(&bytes).unwrap());
    }
    (rounds, accepted)
}

/// The other public entry point for untrusted input.
///
/// The decoder was fuzzed and `Cairn::from_str` was not, which is how a
/// boundary panic in it survived to be found by review instead. Anything that
/// takes bytes from a stranger belongs here.
#[test]
fn parsing_a_cairn_never_panics() {
    let mut rng = Rng(0x0BAD_CA17_0000_0001);
    let good = Cairn::of_encoded(b"seed").to_string();

    for i in 0..200_000 {
        let text: String = match i % 4 {
            // Arbitrary bytes that happen to be UTF-8.
            0 => (0..rng.below(80)).map(|_| char::from(rng.byte())).collect(),
            // Arbitrary characters, so multi-byte ones turn up on every offset.
            1 => (0..rng.below(40))
                .map(|_| char::from_u32((rng.next() % 0x1_0000) as u32).unwrap_or('?'))
                .collect(),
            // A real cairn with one character replaced by a multi-byte one.
            2 => {
                let mut chars: Vec<char> = good.chars().collect();
                let at = rng.below(chars.len());
                chars[at] = ['€', 'é', '💀', 'ß'][rng.below(4)];
                chars.into_iter().collect()
            }
            // A real cairn with one byte corrupted.
            _ => {
                let mut raw = good.clone().into_bytes();
                let at = rng.below(raw.len());
                raw[at] = rng.byte();
                String::from_utf8(raw).unwrap_or_else(|_| good.clone())
            }
        };

        // The only requirement: an answer or a refusal, never a crash.
        if let Ok(cairn) = text.parse::<Cairn>() {
            assert_eq!(cairn.to_string(), text, "accepted a spelling it does not produce");
        }
    }
    println!("\n  0045  200000 candidate cairns parsed, no panics\n");
}

/// Every input in the corpus survives being handed back unmutated.
#[test]
fn the_corpus_is_canonical() {
    for bytes in corpus() {
        assert!(check(&bytes).is_none(), "corpus entry is not canonical: {}", hex(&bytes));
        assert!(decode_stored(&bytes).is_ok(), "corpus entry does not decode: {}", hex(&bytes));
    }
}

/// Runs on every commit. Enough to catch a regression the same day it lands.
#[test]
fn no_panic_and_no_non_canonical_acceptance() {
    let mut total = 0;
    let mut accepted = 0;
    for seed in 1..=8u64 {
        let (n, a) = run(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 40_000);
        total += n;
        accepted += a;
    }
    println!(
        "\n  0045  {total} inputs across 8 seeds: {accepted} decoded, all canonical, no panics\n"
    );
    // If nothing is ever accepted the fuzzer is only testing the reject path.
    assert!(
        accepted > total / 100,
        "only {accepted} of {total} decoded; mutations are too destructive"
    );
}

/// The long soak. `scripts/task fuzz`.
#[test]
#[ignore = "a soak; run with scripts/task fuzz"]
fn soak() {
    let rounds: usize =
        std::env::var("NETHER_FUZZ_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(5_000_000);
    let (total, accepted) = run(0xDEAD_BEEF_CAFE_F00D, rounds);
    println!("\n  0045  soak: {total} inputs, {accepted} decoded, all canonical, no panics\n");
}
