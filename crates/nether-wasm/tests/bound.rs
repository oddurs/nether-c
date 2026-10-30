//! A bound on every path that reads a stranger's bytes. 0219.
//!
//! Two decoders take bytes from people the reader does not trust: the front
//! end, which takes source (`bury`, `graft`, and `exhume` reading a residue
//! back), and the canonical codec, which takes objects (the store, the
//! browser). Everything else that crosses a boundary in `SECURITY.md` arrives
//! through one of them. Each is held to a line: what it allocates is at most
//! a stated multiple of what it was given, at its peak and in total, so no
//! count, nesting or repetition in the input buys more than its own length.
//!
//! The multiple is not small. One byte of source can be a token and a node,
//! and one byte of an object can be a 48-byte value, so no tree of either can
//! be built in ten times its input. What is ruled out is amplification: the
//! nested count that reserved five gigabytes from one megabyte is the case
//! this file was written against, and it fails here on the old decoder.
//!
//! The allocator that counts lives here because counting allocations is a
//! `GlobalAlloc`, which is `unsafe`, and this is one of the two crates that
//! may say so.

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicUsize, Ordering::Relaxed};
use std::sync::{Mutex, MutexGuard};

use nether_ledger::{Cairn, Store, Stored, Value, decode_stored};

struct Counting;

static LIVE: AtomicUsize = AtomicUsize::new(0);
static PEAK: AtomicUsize = AtomicUsize::new(0);
static TOTAL: AtomicUsize = AtomicUsize::new(0);

// SAFETY: every call is passed straight to `System` with the same arguments;
// the counters are bookkeeping beside it and touch no memory it hands out.
unsafe impl GlobalAlloc for Counting {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let live = LIVE.fetch_add(layout.size(), Relaxed) + layout.size();
        PEAK.fetch_max(live, Relaxed);
        TOTAL.fetch_add(layout.size(), Relaxed);
        // SAFETY: the caller's contract for `alloc` is `System`'s.
        unsafe { System.alloc(layout) }
    }

    unsafe fn dealloc(&self, at: *mut u8, layout: Layout) {
        LIVE.fetch_sub(layout.size(), Relaxed);
        // SAFETY: `at` came from `alloc` above, which is `System`'s.
        unsafe { System.dealloc(at, layout) }
    }
}

#[global_allocator]
static COUNTING: Counting = Counting;

/// The counters are process-wide, so one measurement at a time.
fn alone() -> MutexGuard<'static, ()> {
    static ONE: Mutex<()> = Mutex::new(());
    ONE.lock().unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// What a read cost: the most it held at once, and everything it asked for.
fn cost(f: impl FnOnce()) -> (usize, usize) {
    let base = LIVE.load(Relaxed);
    PEAK.store(base, Relaxed);
    let total = TOTAL.load(Relaxed);
    f();
    (PEAK.load(Relaxed).saturating_sub(base), TOTAL.load(Relaxed) - total)
}

/// Fixed costs that do not depend on the input: a store's directories, a
/// diagnostic's message, the first growth of every vector.
const SLACK: usize = 64 << 10;

/// The source front end: lex, parse, lower, check and print, which is what
/// `bury` does before it buries and `exhume` does to a residue it reads back.
/// Measured worst case 167x at the peak and 244x in total, both in the parser
/// on a megabyte of one-line functions.
const SOURCE_PEAK: usize = 256;
const SOURCE_TOTAL: usize = 384;

/// The codec: an array of one-byte units, each a 48-byte value, in vectors
/// that double. Measured worst case 96x at the peak and in total.
const OBJECT_PEAK: usize = 128;
const OBJECT_TOTAL: usize = 192;

fn within(what: &str, len: usize, (peak, total): (usize, usize), each: (usize, usize)) {
    let line = |per: usize| per * len + SLACK;
    println!("  {what:<28} {len:>8} bytes  peak {peak:>10}  total {total:>10}");
    assert!(peak <= line(each.0), "{what}: {len} bytes held {peak} at once, over {each:?}");
    assert!(total <= line(each.1), "{what}: {len} bytes asked for {total} in all, over {each:?}");
}

fn front_end(source: &[u8]) {
    let Ok(ast) = nether_syntax::parse(source) else { return };
    let Ok(unit) = nether_syntax::lower(&ast) else { return };
    let _ = nether_core::check(&unit);
    let _ = nether_syntax::print(&unit);
}

/// Source built to be expensive per byte: as many tokens and nodes as the
/// length allows, nesting at the limit, and literals that decode.
fn sources(n: usize) -> Vec<(&'static str, Vec<u8>)> {
    let fill = |s: &str| s.repeat(n / s.len()).into_bytes();
    vec![
        ("semicolons", fill(";")),
        ("declarations", fill("I64 a;")),
        ("one-line functions", fill("U0 f(){1;}")),
        ("sums", fill("I64 a = 1+1+1+1+1+1+1+1+1;\n")),
        ("calls", format!("U0 g() {{ {} }}", "g();".repeat(n / 4 - 4)).into_bytes()),
        ("parentheses", fill(&format!("I64 a = {}1{};", "(".repeat(30), ")".repeat(30)))),
        ("negations", format!("I64 a = {}1;", "-".repeat(n - 12)).into_bytes()),
        ("structs", fill("struct P { I64 x; I64 y; }\n")),
        ("a string", format!("Str s = \"{}\";", "a".repeat(n - 12)).into_bytes()),
        ("escapes", format!("Bytes b = b\"{}\";", "\\x41".repeat(n / 4 - 4)).into_bytes()),
        ("unicode identifiers", fill("I64 \u{e9}\u{e9}\u{e9};")),
        ("bytes", (0..n).map(|i| u8::try_from(i * 7919 % 251).unwrap_or(0)).collect()),
    ]
}

/// Objects built to be expensive per byte, and the counts that claim more
/// than they carry.
fn objects(n: usize) -> Vec<(&'static str, Vec<u8>)> {
    const ARRAY: u8 = 0x11;
    const STRUCT: u8 = 0x10;
    let count = |c: usize| u64::try_from(c).unwrap_or(u64::MAX).to_be_bytes();
    let mut out = Vec::new();

    let mut units = vec![ARRAY];
    units.extend(count(n - 9));
    units.resize(n, 0x00);
    out.push(("an array of units", units));

    let mut fields = vec![STRUCT];
    fields.extend(count(1));
    fields.push(b'S');
    fields.extend(count(n - 18));
    fields.resize(n, 0x00);
    out.push(("a struct of units", fields));

    // Every level claims what is left, which every level can.
    let mut nested = Vec::new();
    for level in 0..nether_ledger::MAX_DEPTH.min(n / 18) {
        nested.push(ARRAY);
        nested.extend(count(n - 9 * (level + 1) - 1));
    }
    nested.resize(n, 0x00);
    out.push(("nested claims", nested));

    let mut arrays = vec![ARRAY];
    arrays.extend(count(n / 9 - 1));
    while arrays.len() + 9 <= n {
        arrays.push(ARRAY);
        arrays.extend(count(0));
    }
    out.push(("an array of empty arrays", arrays));

    out.push(("an object of bytes", Stored::Value(Value::Bytes(vec![7; n - 9])).encode()));
    out
}

#[test]
fn the_front_end_is_linear_in_its_input() {
    let _one = alone();
    for n in [1 << 10, 1 << 18] {
        for (what, source) in sources(n) {
            let spent = cost(|| front_end(&source));
            within(what, source.len(), spent, (SOURCE_PEAK, SOURCE_TOTAL));
        }
    }
}

#[test]
fn the_codec_is_linear_in_its_input() {
    let _one = alone();
    for n in [1 << 10, 1 << 18] {
        for (what, bytes) in objects(n) {
            let spent = cost(|| {
                let _ = decode_stored(&bytes);
            });
            within(what, bytes.len(), spent, (OBJECT_PEAK, OBJECT_TOTAL));
        }
    }
}

/// A store's read is a file read, a hash and the codec, and its cache holds
/// at most what it read.
#[test]
fn a_store_read_is_linear_in_the_object() {
    let _one = alone();
    let root = std::env::temp_dir().join(format!("nether-bound-{}", std::process::id()));
    let store = Store::open(&root).expect("a scratch store");
    let n = 1 << 18;
    let units = Stored::Value(Value::Array(vec![Value::Unit; n]));
    let cairn = store.put(&units).expect("put");
    let len = units.encode().len();
    drop(units);
    let spent = cost(|| {
        let _ = store.get(cairn);
    });
    within("a store's array of units", len, spent, (OBJECT_PEAK + 2, OBJECT_TOTAL + 2));
    let spent = cost(|| {
        let _ = store.resolve(&cairn.to_string()[..8]);
        let _ = "0".repeat(64).parse::<Cairn>();
    });
    within("resolve and a cairn's name", 64, spent, (0, 0));
    let _ = std::fs::remove_dir_all(root);
}
