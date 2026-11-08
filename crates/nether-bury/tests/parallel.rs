//! Serial equivalence, `spec/06-evaluation.md` §6.2.1. 0204, 0205.
//!
//! A burial on several threads must leave the trace one thread leaves, halt
//! where one thread halts, and report the steps one thread spends. Nothing
//! here measures speed; every assertion is that the count of threads cannot
//! be seen in what a burial produced.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicBool, Ordering::Relaxed};

use nether_bury::{Answers, Halt, Residue, bury_on};
use nether_core::{Unit, check};
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse, print};

const FIXTURES: &[&str] = &[
    include_str!("../../../tests/programs/build.nc"),
    include_str!("../../../tests/programs/errand.nc"),
    include_str!("../../../tests/programs/hello.nc"),
    include_str!("../../../tests/programs/order.nc"),
    include_str!("../../../tests/programs/owed.nc"),
    include_str!("../../../tests/programs/peek.nc"),
    include_str!("../../../tests/programs/post.nc"),
    include_str!("../../../tests/programs/tick.nc"),
    include_str!("../../../tests/programs/verdict.nc"),
];

/// Independent demands, demands that share a binding, deposits on either
/// side of a hole, a shared question asked from two demands, and a collapse
/// after all of them: every way one demand can see another, in one unit.
const MIXED: &str = r#"
I64 fib(I64 n) {
  if (n < 2) { return n; }
  return fib(n - 1) + fib(n - 2);
}
U0 say(I64 n) { "fib\n"; fib(n); }
Bytes@3 note = must(descend disk { read("note.txt") });
I64 shared = fib(9);

demand say(12);
demand fib(13);
demand shared + 1;
demand concat(note, b"!");
demand say(11);
demand descend disk { read("note.txt") };
demand shared + fib(10);
demand fib(12);
demand must(descend disk { read("absent.txt") });
"#;

fn unit(source: &str) -> nether_core::Unit {
    let unit = lower(&parse(source.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&unit).is_empty(), "checks:\n{source}");
    unit
}

fn on(unit: &Unit, fuel: u64, threads: usize) -> Result<Residue, Halt> {
    bury_on(unit, Cairn::of_encoded(b"parallel"), fuel, &Answers::none(), threads)
}

/// The trace cairn `nether bury` writes for a residue (`spec/07-ledger.md` §7.3).
fn trace_cairn(unit: &Unit, r: &Residue) -> Cairn {
    let source = Cairn::of_encoded(b"parallel");
    let printed = print(&r.as_unit(unit));
    let residue = Stored::Value(Value::Bytes(printed.into_bytes())).cairn();
    let depth = r.questions().into_iter().fold(r.depth.get(), |d, n| match n {
        Node::Hole { stratum, .. } => d.max(*stratum),
        _ => d,
    });
    Node::Trace {
        residue,
        holes: r.holes.clone(),
        unrecorded: false,
        depth,
        witnesses: Vec::new(),
        deposits: r.deposits.clone(),
        source,
    }
    .cairn()
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }

    fn under(&mut self, limit: u64) -> u64 {
        self.next() % limit
    }
}

/// A seeded program mixing independent demands, shared pure/starved globals,
/// duplicate read holes from different spans, unmerged act holes and deposits.
fn generated(seed: u64) -> String {
    let mut rng = Rng(seed ^ 0x9e37_79b9_7f4a_7c15);
    let mut out = String::from(
        "I64 fib(I64 n) {\n  if (n < 2) { return n; }\n  return fib(n - 1) + fib(n - 2);\n}\n\
         U0 say_a(I64 n) { \"alpha\\n\"; fib(n); }\n\
         U0 say_b(I64 n) { \"beta\\n\"; fib(n); }\n\
         Bytes@3 g_note = must(descend disk { read(\"note.txt\") });\n\
         Bytes@3 g_alt = must(descend disk { read(\"alt.txt\") });\n",
    );
    let _ = writeln!(out, "I64 g_pure = fib({});", 6 + rng.under(4));
    let count = 6 + rng.under(7);
    for idx in 0..count {
        let step = 7 + rng.under(5);
        match rng.under(7) {
            0 => {
                let _ = writeln!(out, "demand fib({step});");
            }
            1 => {
                let _ = writeln!(out, "demand say_a({});", 6 + rng.under(5));
            }
            2 => {
                let _ = writeln!(out, "demand say_b({});", 6 + rng.under(5));
            }
            3 => {
                let _ = writeln!(out, "demand g_pure + fib({});", 5 + rng.under(5));
            }
            4 => {
                let _ = writeln!(out, "demand concat(g_note, b\"#{idx}\");");
            }
            5 => {
                let file = if rng.under(2) == 0 { "note.txt" } else { "alt.txt" };
                let _ = writeln!(out, "demand descend disk {{ read(\"{file}\") }};");
            }
            _ => {
                let _ = writeln!(out, "demand descend disk! {{ write(\"out.txt\", b\"{idx}\") }};");
            }
        }
    }
    out
}

#[test]
fn every_thread_count_leaves_the_serial_trace() {
    for source in FIXTURES.iter().copied().chain([MIXED]) {
        let unit = unit(source);
        let serial = on(&unit, 50_000_000, 1);
        for threads in [2, 3, 4, 16] {
            assert_eq!(on(&unit, 50_000_000, threads), serial, "{threads} threads:\n{source}");
        }
    }
}

/// Where a budget runs out is where the serial order reached, at every
/// budget, so a demand finished early on another thread cannot move it.
#[test]
fn every_budget_runs_out_where_the_serial_order_would() {
    let unit = unit(MIXED);
    let spent = on(&unit, u64::MAX, 1).expect("buries").fuel_spent;
    let mut fuel = 0;
    while fuel <= spent + 1 {
        let serial = on(&unit, fuel, 1);
        assert_eq!(on(&unit, fuel, 4), serial, "fuel {fuel} of {spent}");
        fuel += 1 + fuel / 64;
    }
}

/// Seeded programs with shared bindings, duplicate holes, deposits and fuel
/// boundaries match the serial oracle on objects, first-hole spans and trace
/// cairn across worker counts and scheduling jitter. 0205.
#[test]
fn seeded_programs_match_serial_objects_spans_and_cairn() {
    let stop = AtomicBool::new(false);
    std::thread::scope(|s| {
        for jitter in 0..2usize {
            let stop = &stop;
            s.spawn(move || {
                let mut spins = jitter;
                while !stop.load(Relaxed) {
                    spins = spins.wrapping_add(1);
                    if spins % 8 == 0 {
                        std::thread::yield_now();
                    }
                }
            });
        }
        for seed in 1..=32 {
            let src = generated(seed);
            let u = unit(&src);
            let serial = on(&u, 50_000_000, 1).expect("serial buries");
            let want_cairn = trace_cairn(&u, &serial);
            let want_questions = serial.questions();
            for threads in [2, 3, 4, 8, 16] {
                if (seed + threads as u64) % 2 == 0 {
                    std::thread::yield_now();
                }
                let got = on(&u, 50_000_000, threads).expect("parallel buries");
                assert_eq!(
                    got.questions(),
                    want_questions,
                    "seed {seed}, {threads} threads:\n{src}"
                );
                assert_eq!(got.named, serial.named, "seed {seed}, {threads} threads:\n{src}");
                assert_eq!(got, serial, "seed {seed}, {threads} threads:\n{src}");
                assert_eq!(
                    trace_cairn(&u, &got),
                    want_cairn,
                    "seed {seed}, {threads} threads:\n{src}"
                );
            }
            let mid = serial.fuel_spent / 2;
            assert_eq!(on(&u, mid, 6), on(&u, mid, 1), "seed {seed} at mid-budget {mid}:\n{src}");
        }
        stop.store(true, Relaxed);
    });
}

/// Bounded CI companion to 0205's thousand-burial proof: repeated burials of
/// one program across varied worker counts and yield schedules produce one cairn.
#[test]
fn repeated_parallel_burials_produce_one_cairn() {
    let u = unit(MIXED);
    let serial = on(&u, 50_000_000, 1).expect("serial buries");
    let want = trace_cairn(&u, &serial);
    let mut cairns = HashSet::new();
    let counts = [2, 3, 4, 6, 8, 12, 16];
    for run in 0..64usize {
        for _ in 0..(run % 4) {
            std::thread::yield_now();
        }
        let got = on(&u, 50_000_000, counts[run % counts.len()]).expect("parallel buries");
        assert_eq!(got.questions(), serial.questions(), "run {run}");
        assert_eq!(got.named, serial.named, "run {run}");
        cairns.insert(trace_cairn(&u, &got));
    }
    assert_eq!(cairns.len(), 1);
    assert!(cairns.contains(&want));
}

/// A worker stops when the serial order is done with its demand. The first
/// demand collapses after a hundred thousand turns, long enough for a worker to take
/// the second, which is then a demand nobody will keep; it used to spin on with the whole fuel budget while the
/// halt waited for it. The bound is ten seconds against minutes, so a slow
/// machine does not make this flaky.
#[test]
fn a_worker_stops_when_the_serial_order_halts() {
    let src = "I64 spin(I64 n) { I64 i = 0; while (i < n) { i += 1; } i }\n\
               demand 1 / (spin(100000) - 100000);\n\
               demand spin(1000000000);\n";
    let unit = lower(&parse(src.as_bytes()).expect("parses")).expect("lowers");
    assert!(check(&unit).is_empty());
    let start = std::time::Instant::now();
    let out = bury_on(&unit, Cairn::of_encoded(src.as_bytes()), 4_000_000_000, &Answers::none(), 2);
    assert!(out.is_err(), "the first demand divides by zero");
    let took = start.elapsed();
    assert!(took < std::time::Duration::from_secs(10), "the halt waited {took:?} for a worker");
}
