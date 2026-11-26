//! What a trace says about itself, computed once.
//!
//! `spec/07-ledger.md` §7.3.2 gives a trace its derived facts, and three rites
//! write a trace. Working them out at each of those three is how each of them
//! came to be wrong at least once.

use std::process::ExitCode;

use nether_bury::{Halt, HaltKind};
use nether_core::{Unit, check, report};
use nether_ledger::{Cairn, Node, Store, StoreError, Stored, Stratum, Value};
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
    let fail = |diags: &mut dyn Iterator<Item = nether_core::Diagnostic>| -> ExitCode {
        for d in diags {
            eprint!("{}", report(&d, &text, shown));
        }
        ExitCode::from(code::MALFORMED)
    };
    let ast =
        parse(bytes).map_err(|f| fail(&mut f.iter().map(nether_syntax::Fault::diagnostic)))?;
    let unit =
        lower(&ast).map_err(|f| fail(&mut f.iter().map(nether_syntax::Fault::diagnostic)))?;
    let faults = check(&unit);
    if !faults.is_empty() {
        return Err(fail(&mut faults.iter().map(nether_core::Fault::diagnostic)));
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

/// Convert a validated `nether_core::Depth` (`0..=8`) to a ledger [`Stratum`].
pub(crate) fn stratum(d: nether_core::Depth) -> Stratum {
    Stratum::new(d.get()).unwrap_or(Stratum::MAX)
}

/// Whether `stratum` and argument count match the world-touching prelude
/// primitive named by `call`.
pub(crate) fn valid_stratum(call: &nether_ledger::Call, stratum: Stratum) -> bool {
    nether_core::Prim::from_name(call.function()).is_some_and(|p| {
        p.latent() > nether_core::Depth::PURE
            && p.latent().get() == stratum.as_u8()
            && call.args().len() == p.arity()
    })
}

/// The stratum a hole or witness reached, or `Corrupt` if wrong kind or forged stratum.
fn reached_node(store: &Store, node: Cairn, expect_witness: bool) -> Result<Stratum, StoreError> {
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
    residue: Stratum,
    holes: &[Cairn],
    witnesses: &[Cairn],
) -> Result<Stratum, ExitCode> {
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
        if reached_node(store, w, true).map_err(|e| store_err(&e))? == Stratum::MAX {
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

/// The unpacked fields of a `Node::Trace`.
pub(crate) struct TraceParts {
    pub residue: Cairn,
    pub holes: Vec<Cairn>,
    pub witnesses: Vec<Cairn>,
    pub deposits: Vec<Cairn>,
    pub source: Cairn,
    pub depth: Stratum,
    pub unrecorded: bool,
}

impl TraceParts {
    pub(crate) fn to_node(&self) -> Node {
        Node::Trace {
            residue: self.residue,
            holes: self.holes.clone(),
            witnesses: self.witnesses.clone(),
            deposits: self.deposits.clone(),
            source: self.source,
            depth: self.depth,
            unrecorded: self.unrecorded,
        }
    }
}

/// Read a `Node::Trace` from the store, or report what `cairn` holds instead.
pub(crate) fn load_trace(store: &Store, cairn: Cairn) -> Result<TraceParts, ExitCode> {
    let Ok(Stored::Node(Node::Trace {
        residue,
        holes,
        witnesses,
        deposits,
        source,
        depth,
        unrecorded,
    })) = store.get(cairn)
    else {
        return Err(crate::not_a_trace(store, cairn));
    };
    Ok(TraceParts { residue, holes, witnesses, deposits, source, depth, unrecorded })
}

/// Read and validate a single `Node::Hole` from `store`.
pub(crate) fn load_hole(
    store: &Store,
    hole: Cairn,
    on_absent: impl FnOnce(Cairn) -> ExitCode,
) -> Result<(nether_ledger::Call, Stratum, nether_ledger::Span), ExitCode> {
    match store.get(hole) {
        Ok(Stored::Node(Node::Hole { call, stratum, span })) if valid_stratum(&call, stratum) => {
            Ok((call, stratum, span))
        }
        Ok(Stored::Node(Node::Hole { call, stratum, .. })) => {
            eprintln!(
                "nether: hole {} claims stratum {stratum} for `{}`",
                hole.short(),
                call.function()
            );
            Err(ExitCode::from(code::MALFORMED))
        }
        Ok(_) => {
            eprintln!("nether: hole {} is not a hole node in this ledger", hole.short());
            Err(ExitCode::from(code::MALFORMED))
        }
        Err(StoreError::Absent(_)) => Err(on_absent(hole)),
        Err(e) => Err(store_err(&e)),
    }
}

/// Validate a trace's `source`, `deposits`, `holes`, `witnesses`, `depth`, and
/// `unrecorded` mark upfront before exhuming, replaying, or grafting.
///
/// Returns whether any prior witness reached stratum 8.
pub(crate) fn verify_trace(store: &Store, trace: &TraceParts) -> Result<bool, ExitCode> {
    if !matches!(
        store.get(trace.source).map_err(|e| store_err(&e))?,
        Stored::Value(Value::Bytes(_))
    ) {
        eprintln!("nether: source {} is not bytes", trace.source.short());
        return Err(ExitCode::from(code::MALFORMED));
    }
    for &d in &trace.deposits {
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
    let prior_unrecorded = unrecorded(store, &trace.witnesses)?;
    if !trace.unrecorded && prior_unrecorded {
        eprintln!("nether: this trace is not marked unrecorded, and a witness under it reached 8");
        return Err(ExitCode::from(code::MALFORMED));
    }
    if trace.unrecorded && trace.depth < Stratum::MAX {
        eprintln!("nether: this trace is marked unrecorded, and records depth {}", trace.depth);
        return Err(ExitCode::from(code::MALFORMED));
    }
    let min_depth = depth(store, Stratum::ZERO, &trace.holes, &trace.witnesses)?;
    if min_depth > trace.depth {
        eprintln!(
            "nether: this trace records depth {}, and a hole or witness under it reaches {min_depth}",
            trace.depth
        );
        return Err(ExitCode::from(code::MALFORMED));
    }
    Ok(prior_unrecorded)
}
