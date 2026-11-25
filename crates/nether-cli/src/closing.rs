//! What a trace says about itself, computed once.
//!
//! `spec/07-ledger.md` §7.3.2 gives a trace its derived facts, and three rites
//! write a trace. Working them out at each of those three is how each of them
//! came to be wrong at least once.

use std::process::ExitCode;

use nether_core::{Unit, check, report};
use nether_ledger::{Cairn, Node, Store, StoreError, Stored, Value};
use nether_syntax::{lower, parse};

use crate::{FAILED, code};

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
        Err(e) => {
            eprintln!("nether: {e}");
            return Err(store_exit(&e));
        }
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

/// The stratum a hole would reach, or an error if unreadable, wrong node kind,
/// or claiming a stratum that disagrees with its prelude primitive.
fn reached_hole(store: &Store, node: Cairn) -> Result<u8, StoreError> {
    match store.get(node)? {
        Stored::Node(Node::Hole { call, stratum, .. }) if valid_stratum(&call, stratum) => {
            Ok(stratum)
        }
        _ => Err(StoreError::Corrupt { asked: node, found: None, why: None }),
    }
}

/// The stratum a witness reached, or an error if unreadable, wrong node kind,
/// or claiming a stratum that disagrees with its prelude primitive.
fn reached_witness(store: &Store, node: Cairn) -> Result<u8, StoreError> {
    match store.get(node)? {
        Stored::Node(Node::Witness { call, stratum, .. }) if valid_stratum(&call, stratum) => {
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
    for node in holes {
        match reached_hole(store, *node) {
            Ok(s) => out = out.max(s),
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(store_exit(&e));
            }
        }
    }
    for node in witnesses {
        match reached_witness(store, *node) {
            Ok(s) => out = out.max(s),
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(store_exit(&e));
            }
        }
    }
    Ok(out)
}

/// §1.7: marked when something *reached* stratum 8, which a hole at 8 has not.
pub fn unrecorded(store: &Store, witnesses: &[Cairn]) -> Result<bool, ExitCode> {
    for w in witnesses {
        match reached_witness(store, *w) {
            Ok(s) if s == nether_ledger::MAX_STRATUM => return Ok(true),
            Ok(_) => {}
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(store_exit(&e));
            }
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
    match store.get(source) {
        Ok(Stored::Value(Value::Bytes(_))) => {}
        Ok(_) => {
            eprintln!("nether: source {} is not bytes", source.short());
            return Err(ExitCode::from(code::MALFORMED));
        }
        Err(e) => {
            eprintln!("nether: {e}");
            return Err(store_exit(&e));
        }
    }
    for &d in deposits {
        match store.get(d) {
            Ok(Stored::Node(Node::Deposit { value, .. })) => match store.get(value) {
                Ok(Stored::Value(_)) => {}
                Ok(_) => {
                    eprintln!("nether: deposit value {} is not a value", value.short());
                    return Err(ExitCode::from(code::MALFORMED));
                }
                Err(e) => {
                    eprintln!("nether: {e}");
                    return Err(store_exit(&e));
                }
            },
            Ok(_) => {
                eprintln!("nether: deposit {} is not a deposit node in this ledger", d.short());
                return Err(ExitCode::from(code::MALFORMED));
            }
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(store_exit(&e));
            }
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
