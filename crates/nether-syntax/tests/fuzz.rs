//! Fuzzing the lexer, the parser and lowering.
//!
//! `nether bury` reads a stranger's source, and `exhume` reads a residue out of
//! a store that may be shared with people who do not trust each other. The
//! decoder and `Cairn::from_str` were fuzzed because a bug was found in each;
//! these take text from the same strangers and had never been handed ten
//! million malformed bytes. 0218.
//!
//! Two properties, held over every input:
//!
//! 1. **Nothing panics.** Any byte string lexes or is refused, parses or is
//!    refused, lowers or is refused, and what lowers checks and prints. A
//!    panicking front end is a denial of service in a shared store, and a
//!    crash is not one of §6.4's limits.
//! 2. **What checks prints as a program that lowers.** §6.5 makes a residue a
//!    program, and `print` is how one is written down; a unit whose printed
//!    form does not read back is a trace nobody can exhume. This harness found
//!    that false on its first run, and it is 0279. Only a unit that checks can
//!    become a trace, so that is the unit the property is about.
//!
//! The generator is seeded and deterministic, so a failure is reproducible
//! from the seed and round printed with it. `scripts/task fuzz` runs it longer.

use nether_core::check;
use nether_syntax::{MAX_NESTING, lex, lower, parse, print};

/// xorshift64*, as `nether-ledger`'s fuzzer has it.
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
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }

    fn byte(&mut self) -> u8 {
        (self.next() & 0xff) as u8
    }
}

/// Programs worth mutating: every fixture, and the corners §03 names.
fn corpus() -> Vec<Vec<u8>> {
    let dir = std::path::Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../../tests/programs"));
    let mut files: Vec<_> = std::fs::read_dir(dir)
        .expect("tests/programs")
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.extension().is_some_and(|e| e == "nc"))
        .collect();
    files.sort();
    let mut out: Vec<Vec<u8>> =
        files.iter().map(|p| std::fs::read(p).expect("a fixture")).collect();
    out.extend(
        [
            "Bytes b = b\"\\x00\\xff\\n\\t\\\\\\\"\";\n",
            "Str s = \"caf\\u{e9} \\u{1F480}\";\n",
            "Cairn c = #0000000000000000000000000000000000000000000000000000000000000000;\n",
            "I64 n = 0x7fffffffffffffff + -9223372036854775808 + 0b101 + 0o17;\n",
            "/* a comment */ // another\nU0 f() { for (I64 i = 0; i < 4; i += 1) { if (i == 2) { break; } } }\n",
            "struct P { I64 x; I64 y; }\ntypedef P Q;\nU0 g() { Q q; q.x = 1; q.y = q.x; }\n",
            "Shade<Bytes> s = descend net { shade must(get(\"https://example.invalid/\")) };\n",
            "U0 h() @{3,5} { opaque { 1; } }\n",
            "Bytes g = { Bytes s = b\"hi\"; concat(s, s) };\ndemand { I64 k = 2; k };\n",
        ]
        .map(|s| s.as_bytes().to_vec()),
    );
    out
}

fn mutate(rng: &mut Rng, seed: &[u8], corpus: &[Vec<u8>]) -> Vec<u8> {
    let mut out = seed.to_vec();
    let at = |rng: &mut Rng, out: &Vec<u8>| rng.below(out.len() + 1);
    match rng.below(9) {
        0 if !out.is_empty() => {
            let i = rng.below(out.len());
            out[i] ^= 1 << rng.below(8);
        }
        1 if !out.is_empty() => {
            let i = rng.below(out.len());
            out[i] = rng.byte();
        }
        2 => out.truncate(at(rng, &out)),
        3 => {
            let i = at(rng, &out);
            out.insert(i, rng.byte());
        }
        4 if !out.is_empty() => {
            let i = rng.below(out.len());
            out.remove(i);
        }
        // A slice of another program, spliced in: the mutation that produces
        // things that look like source and are not.
        5 => {
            let other = &corpus[rng.below(corpus.len())];
            let from = rng.below(other.len());
            let to = (from + rng.below(64)).min(other.len());
            let i = at(rng, &out);
            out.splice(i..i, other[from..to].iter().copied());
        }
        // Punctuation that opens and closes, which is where the ladder is.
        6 => {
            let i = at(rng, &out);
            let p = b"(){}[]<>\"'#@";
            out.insert(i, p[rng.below(p.len())]);
        }
        // A multi-byte character, so every offset meets one.
        7 => {
            let i = at(rng, &out);
            let c = ['é', '€', '💀', 'ß', '\u{200B}', '\u{FEFF}'][rng.below(6)];
            let mut buf = [0u8; 4];
            out.splice(i..i, c.encode_utf8(&mut buf).bytes());
        }
        _ => out.extend((0..rng.below(8)).map(|_| rng.byte())),
    }
    out
}

/// Input built to reach a limit rather than to look like a program.
fn extreme(rng: &mut Rng) -> Vec<u8> {
    let deep = usize::try_from(MAX_NESTING).unwrap_or(64) * 2;
    let n = rng.below(deep) + 1;
    match rng.below(6) {
        0 => format!("I64 a = {}1{};\n", "(".repeat(n), ")".repeat(n)).into_bytes(),
        1 => format!("U0 f() {}{}\n", "{ ".repeat(n), "}".repeat(n)).into_bytes(),
        2 => format!("Bytes b = b\"{}\";\n", "\\x41".repeat(n * 64)).into_bytes(),
        3 => format!("I64 a = {}1;\n", "-".repeat(n * 8)).into_bytes(),
        4 => format!("I64 a = {};\n", "9".repeat(n)).into_bytes(),
        _ => (0..rng.below(256)).map(|_| rng.byte()).collect(),
    }
}

/// What one input did.
#[derive(Default)]
struct Tally {
    lexed: usize,
    parsed: usize,
    lowered: usize,
}

fn one(bytes: &[u8], tally: &mut Tally) {
    if lex(bytes).is_ok() {
        tally.lexed += 1;
    }
    let Ok(ast) = parse(bytes) else { return };
    tally.parsed += 1;
    let Ok(unit) = lower(&ast) else { return };
    tally.lowered += 1;
    if !check(&unit).is_empty() {
        return;
    }
    let printed = print(&unit);
    let read = parse(printed.as_bytes()).map(|ast| lower(&ast));
    assert!(
        matches!(read, Ok(Ok(_))),
        "lowered, and printed as\n{printed}\nwhich does not: {read:?}"
    );
}

fn run(seed: u64, rounds: usize) -> Tally {
    let mut rng = Rng(seed);
    let corpus = corpus();
    let mut tally = Tally::default();
    for i in 0..rounds {
        let bytes = match i % 8 {
            0 => (0..rng.below(96)).map(|_| rng.byte()).collect(),
            1 => extreme(&mut rng),
            _ => {
                let base = &corpus[rng.below(corpus.len())];
                let mut b = mutate(&mut rng, base, &corpus);
                for _ in 0..rng.below(3) {
                    b = mutate(&mut rng, &b, &corpus);
                }
                b
            }
        };
        // A panic here is the failure, whichever property it was; the seed
        // and round reproduce it.
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            one(&bytes, &mut tally);
        }));
        assert!(
            caught.is_ok(),
            "seed {seed:#x}, round {i} failed on:\n{}",
            String::from_utf8_lossy(&bytes)
        );
    }
    tally
}

/// The corpus reaches lowering, or the fuzzer only tests refusals.
#[test]
fn the_corpus_lowers() {
    let mut tally = Tally::default();
    for bytes in corpus() {
        one(&bytes, &mut tally);
    }
    assert!(tally.lowered > 0, "nothing in the corpus lowers; it is testing only refusals");
}

/// Runs on every commit. Enough to catch a regression the same day it lands.
#[test]
fn no_input_panics_the_front_end() {
    let mut total = Tally::default();
    for seed in 1..=8u64 {
        let t = run(seed.wrapping_mul(0x9E37_79B9_7F4A_7C15), 4_000);
        total.lexed += t.lexed;
        total.parsed += t.parsed;
        total.lowered += t.lowered;
    }
    println!(
        "\n  0218  32000 inputs: {} lexed, {} parsed, {} lowered, no panics\n",
        total.lexed, total.parsed, total.lowered
    );
    // If nothing parses, the mutations only ever test the first refusal.
    assert!(total.parsed > 32_000 / 50, "only {} parsed; mutations too destructive", total.parsed);
}

/// The long soak. `scripts/task fuzz`.
#[test]
#[ignore = "a soak; run with scripts/task fuzz"]
fn soak() {
    let rounds: usize =
        std::env::var("NETHER_FUZZ_ROUNDS").ok().and_then(|v| v.parse().ok()).unwrap_or(2_000_000);
    let started = std::time::Instant::now();
    let t = run(0x5EED_0218_F00D_CAFE, rounds);
    println!(
        "\n  0218  soak: {rounds} inputs in {:.0?}: {} lexed, {} parsed, {} lowered, no panics\n",
        started.elapsed(),
        t.lexed,
        t.parsed,
        t.lowered
    );
}
