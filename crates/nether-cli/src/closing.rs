//! What a trace says about itself, computed once.
//!
//! `spec/07-ledger.md` §7.3.2 gives a trace its derived facts, and three rites
//! write a trace. Working them out at each of those three is how each of them
//! came to be wrong at least once.

use std::process::ExitCode;

use nether_bury::{Halt, HaltKind};
use nether_core::{Unit, check, report};
use nether_ledger::{Cairn, Node, Store, StoreError, Stored, Value};
use nether_syntax::{lower, parse};

use crate::{FAILED, code};

/// Print a burial halt diagnostic and map it to §8.8's exit code.
pub(crate) fn halt_exit(halt: &Halt, text: &str, shown: &str) -> ExitCode {
    eprint!("{}", report(&halt.diagnostic(), text, shown));
    match halt.kind {
        HaltKind::OutOfFuel { .. } => ExitCode::from(code::FUEL),
        _ => FAILED,
    }
}

/// Print a store error and map it to §8.8's exit code.
pub(crate) fn store_err(e: &StoreError) -> ExitCode {
    eprintln!("nether: {e}");
    store_exit(e)
}

/// Parse, lower, and check source or residue bytes, reporting every fault.
pub fn checked_unit(bytes: &[u8], shown: &str) -> Result<Unit, ExitCode> {
    let text = String::from_utf8_lossy(bytes);
    let complain = |d: &nether_core::Diagnostic| eprint!("{}", report(d, &text, shown));
    let ast = match parse(bytes) {
        Ok(ast) => ast,
        Err(faults) => {
            for f in &faults {
                complain(&f.diagnostic());
            }
            return Err(ExitCode::from(code::MALFORMED));
        }
    };
    let unit = match lower(&ast) {
        Ok(unit) => unit,
        Err(faults) => {
            for f in &faults {
                complain(&f.diagnostic());
            }
            return Err(ExitCode::from(code::MALFORMED));
        }
    };
    let faults = check(&unit);
    if !faults.is_empty() {
        for f in &faults {
            complain(&f.diagnostic());
        }
        return Err(ExitCode::from(code::MALFORMED));
    }
    Ok(unit)
}

pub(crate) fn store_exit(e: &StoreError) -> ExitCode {
    match e {
        StoreError::Corrupt { .. } | StoreError::Ragged { .. } => ExitCode::from(code::MALFORMED),
        StoreError::Absent(_) => ExitCode::from(code::ABSENT),
        _ => FAILED,
    }
}

/// Read a trace's residue bytes and parse/lower/check them.
pub fn residue_unit(
    store: &Store,
    trace: Cairn,
    residue: Cairn,
) -> Result<(Vec<u8>, Unit), ExitCode> {
    let text = match store.get(residue) {
        Ok(Stored::Value(Value::Bytes(text))) => text,
        Ok(_) => {
            eprintln!("nether: the residue of {} is not bytes", trace.short());
            return Err(ExitCode::from(code::MALFORMED));
        }
        Err(StoreError::Absent(_)) => {
            eprintln!("nether: the residue of {} is not in this ledger", trace.short());
            return Err(ExitCode::from(code::ABSENT));
        }
        Err(e) => return Err(store_err(&e)),
    };
    let shown = format!("{}:residue", trace.short());
    let unit = checked_unit(&text, &shown)?;
    Ok((text, unit))
}

/// Whether `stratum` and argument count match the world-touching prelude
/// primitive named by `call`.
pub(crate) fn valid_stratum(call: &nether_ledger::Call, stratum: u8) -> bool {
    nether_core::Prim::from_name(&call.function).is_some_and(|p| {
        p.latent() > nether_core::Depth::PURE
            && p.latent().get() == stratum
            && call.args.len() == p.arity()
    })
}

/// The stratum a hole or witness reached, or `Corrupt` if wrong kind or forged stratum.
fn reached_node(store: &Store, node: Cairn, expect_witness: bool) -> Result<u8, StoreError> {
    match store.get(node)? {
        Stored::Node(Node::Hole { call, stratum, .. })
            if !expect_witness && valid_stratum(&call, stratum) =>
        {
            Ok(stratum)
        }
        Stored::Node(Node::Witness { call, stratum, .. })
            if expect_witness && valid_stratum(&call, stratum) =>
        {
            Ok(stratum)
        }
        _ => Err(StoreError::Corrupt { asked: node, found: None, why: None }),
    }
}

/// §7.3.2: the join of what the residue still reaches, what a hole would
/// reach, and what a witness reached.
pub fn depth(
    store: &Store,
    residue: u8,
    holes: &[Cairn],
    witnesses: &[Cairn],
) -> Result<u8, ExitCode> {
    let mut out = residue;
    for (nodes, expect_witness) in [(holes, false), (witnesses, true)] {
        for &node in nodes {
            out = out.max(reached_node(store, node, expect_witness).map_err(|e| store_err(&e))?);
        }
    }
    Ok(out)
}

/// §1.7: marked when something *reached* stratum 8, which a hole at 8 has not.
pub fn unrecorded(store: &Store, witnesses: &[Cairn]) -> Result<bool, ExitCode> {
    for &w in witnesses {
        if reached_node(store, w, true).map_err(|e| store_err(&e))? == nether_ledger::MAX_STRATUM {
            return Ok(true);
        }
    }
    Ok(false)
}

/// §8.4 / §7.3.1: carry prior deposits forward, then append every new deposit
/// produced by burying the residue in exact order (preserving repeats in `now`).
pub fn deposits(before: &[Cairn], now: &[Cairn]) -> Vec<Cairn> {
    let mut out = Vec::with_capacity(before.len() + now.len());
    out.extend_from_slice(before);
    out.extend_from_slice(now);
    out
}

/// Validate a trace's `source`, `deposits`, `holes`, `witnesses`, `depth`, and
/// `unrecorded` mark upfront before exhuming, replaying, or grafting.
///
/// Returns whether any prior witness reached stratum 8.
pub fn verify_trace_parts(
    store: &Store,
    source: Cairn,
    deposits: &[Cairn],
    holes: &[Cairn],
    witnesses: &[Cairn],
    orig_depth: u8,
    marked: bool,
) -> Result<bool, ExitCode> {
    if !matches!(store.get(source).map_err(|e| store_err(&e))?, Stored::Value(Value::Bytes(_))) {
        eprintln!("nether: source {} is not bytes", source.short());
        return Err(ExitCode::from(code::MALFORMED));
    }
    for &d in deposits {
        let Stored::Node(Node::Deposit { value, .. }) = store.get(d).map_err(|e| store_err(&e))?
        else {
            eprintln!("nether: deposit {} is not a deposit node in this ledger", d.short());
            return Err(ExitCode::from(code::MALFORMED));
        };
        if !matches!(store.get(value).map_err(|e| store_err(&e))?, Stored::Value(_)) {
            eprintln!("nether: deposit value {} is not a value", value.short());
            return Err(ExitCode::from(code::MALFORMED));
        }
    }
    let prior_unrecorded = unrecorded(store, witnesses)?;
    if !marked && prior_unrecorded {
        eprintln!("nether: this trace is not marked unrecorded, and a witness under it reached 8");
        return Err(ExitCode::from(code::MALFORMED));
    }
    if marked && orig_depth < nether_ledger::MAX_STRATUM {
        eprintln!("nether: this trace is marked unrecorded, and records depth {orig_depth}");
        return Err(ExitCode::from(code::MALFORMED));
    }
    let min_depth = depth(store, 0, holes, witnesses)?;
    if min_depth > orig_depth {
        eprintln!(
            "nether: this trace records depth {orig_depth}, and a hole or witness under it reaches {min_depth}"
        );
        return Err(ExitCode::from(code::MALFORMED));
    }
    Ok(prior_unrecorded)
}
