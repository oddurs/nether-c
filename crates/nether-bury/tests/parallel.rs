//! Serial equivalence, `spec/06-evaluation.md` §6.2.1. 0204.
//!
//! A burial on several threads must leave the trace one thread leaves, halt
//! where one thread halts, and report the steps one thread spends. Nothing
//! here measures speed; every assertion is that the count of threads cannot
//! be seen in what a burial produced.

use nether_bury::{Answers, Halt, Residue, bury_on};
use nether_core::check;
use nether_ledger::Cairn;
use nether_syntax::{lower, parse};

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

fn on(unit: &nether_core::Unit, fuel: u64, threads: usize) -> Result<Residue, Halt> {
    bury_on(unit, Cairn::of_encoded(b"parallel"), fuel, &Answers::none(), threads)
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
