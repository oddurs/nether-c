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

/// The stratum a hole would reach, or an error if unreadable or the wrong node kind.
fn reached_hole(store: &Store, node: Cairn) -> Result<u8, StoreError> {
    match store.get(node)? {
        Stored::Node(Node::Hole { stratum, .. }) => Ok(stratum),
        _ => Err(StoreError::Corrupt { asked: node, found: None, why: None }),
    }
}

/// The stratum a witness reached, or an error if unreadable or the wrong node kind.
fn reached_witness(store: &Store, node: Cairn) -> Result<u8, StoreError> {
    match store.get(node)? {
        Stored::Node(Node::Witness { stratum, .. }) => Ok(stratum),
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
