//! Burial in a browser.
//!
//! The whole of `bury` with nothing that touches a machine: lex, parse, lower,
//! check, bury, print. No store, because a browser has no ledger to write to
//! — a burial names everything it makes and hands the names back, and what
//! the page does with them is the page's business.
//!
//! No `nether-world` either. A browser has no world to grant, so every
//! world-touching expression becomes a hole, which is what
//! `spec/08-rites.md` §8.2 does by default anyway.
//!
//! ## The seam
//!
//! Hand-written, over linear memory, because a bindings generator is a
//! dependency and this is four functions:
//!
//! ```js
//! const p = m.nether_alloc(bytes.length);
//! new Uint8Array(m.memory.buffer).set(bytes, p);
//! const r = m.nether_bury(p, bytes.length, 1000000);
//! const len = new DataView(m.memory.buffer).getUint32(r, true);
//! const json = new TextDecoder().decode(
//!     new Uint8Array(m.memory.buffer, r + 4, len));
//! m.nether_free(r, len + 4);
//! ```
//!
//! Every returned block is a little-endian `u32` length and then that many
//! bytes of UTF-8. The caller owns it and frees it, which is the same bargain
//! `spec/09-prelude.md` §9.8.1 strikes at the other boundary and for the same
//! reason: two sides sharing an allocator they do not have is a corrupted heap
//! an hour later.

use core::fmt::Write as _;

use nether_core::{Depth, check, report};
use nether_ledger::{Node, Stored, Value};
use nether_syntax::{lower, parse, print};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// Hand back `len` bytes the caller may write into.
///
/// # Safety
///
/// The pointer is valid for `len` bytes until it is passed to
/// [`nether_free`] with the same length.
#[unsafe(no_mangle)]
pub extern "C" fn nether_alloc(len: usize) -> *mut u8 {
    let mut bytes = Vec::<u8>::with_capacity(len);
    let at = bytes.as_mut_ptr();
    core::mem::forget(bytes);
    at
}

/// Give back what [`nether_alloc`] handed out, or what a burial returned.
///
/// # Safety
///
/// `at` came from this module and `len` is the length it was made with.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nether_free(at: *mut u8, len: usize) {
    if at.is_null() {
        return;
    }
    // SAFETY: the caller's half of the bargain in the module documentation.
    drop(unsafe { Vec::from_raw_parts(at, 0, len) });
}

/// Bury `len` bytes of source, and hand back a length-prefixed JSON block.
///
/// `fuel` of zero means §8.2's default. See [`burying`].
///
/// # Safety
///
/// `source` is readable for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nether_bury(source: *const u8, len: usize, fuel: u64) -> *mut u8 {
    if source.is_null() {
        return block(&trouble("nothing was handed in", ""));
    }
    // SAFETY: the caller's half of the bargain in the module documentation.
    let bytes = unsafe { core::slice::from_raw_parts(source, len) }.to_vec();
    block(&burying(&bytes, fuel))
}

/// A length-prefixed block the caller owns: a little-endian `u32`, then that
/// many bytes.
fn block(text: &str) -> *mut u8 {
    let said = text.as_bytes();
    let mut out = Vec::with_capacity(said.len() + 4);
    let Ok(len) = u32::try_from(said.len()) else {
        // Four gigabytes of JSON is not a thing this can hand back, and
        // truncating it silently would be worse than saying so.
        return block("{\"error\":\"the answer is too large to return\"}");
    };
    out.extend_from_slice(&len.to_le_bytes());
    out.extend_from_slice(said);
    let at = out.as_mut_ptr();
    core::mem::forget(out);
    at
}

/// The burial itself, in the order §8.2 puts it.
///
/// Public and pointer-free, because the claim worth proving is that this
/// agrees with `nether bury` on the same source, and a proof that has to go
/// through linear memory to make it is proving the plumbing instead.
#[must_use]
pub fn burying(source: &[u8], fuel: u64) -> String {
    // Zero means §8.2's default rather than a burial that cannot take a step,
    // because a budget of nothing is never what anyone meant.
    let fuel = if fuel == 0 { DEFAULT_FUEL } else { fuel };
    let text = String::from_utf8_lossy(source).into_owned();
    let ast = match parse(source) {
        Ok(ast) => ast,
        Err(faults) => return trouble(&report(&faults[0].diagnostic(), &text, "source"), ""),
    };
    let unit = match lower(&ast) {
        Ok(unit) => unit,
        Err(faults) => return trouble(&report(&faults[0].diagnostic(), &text, "source"), ""),
    };
    if let Some(fault) = check(&unit).first() {
        return trouble(&report(&fault.diagnostic(), &text, "source"), "");
    }

    // §7.3.1: a span names its source by cairn, so the name exists first.
    let named = Stored::Value(Value::Bytes(source.to_vec()));
    let source_cairn = named.cairn();

    let residue = match nether_bury::bury(&unit, source_cairn, fuel) {
        Ok(residue) => residue,
        Err(halt) => return trouble(&report(&halt.diagnostic(), &text, "source"), ""),
    };

    let printed = print(&residue.as_unit(&unit));
    let printed_value = Stored::Value(Value::Bytes(printed.clone().into_bytes()));
    let residue_cairn = printed_value.cairn();
    // Burial holds no capability, so there are no witnesses and §7.3.2's join
    // is the residue's depth alone.
    let trace = Stored::Node(Node::Trace {
        residue: residue_cairn,
        holes: residue.holes.clone(),
        witnesses: Vec::new(),
        deposits: residue.deposits.clone(),
        unrecorded: false,
        source: source_cairn,
        depth: residue.depth.get(),
    });

    let holes: Vec<_> = residue
        .holes
        .iter()
        .filter_map(|h| {
            let Some(Stored::Node(Node::Hole { call, stratum, .. })) = residue.get(*h) else {
                return None;
            };
            let cap = Depth::new(*stratum)
                .and_then(nether_core::Capability::at)
                .map(nether_core::Capability::name)
                .map_or_else(|| "null".to_owned(), quoted);
            Some(format!(
                "{{\"cairn\":\"{h}\",\"call\":{},\"stratum\":{stratum},\"capability\":{cap}}}",
                quoted(&format!("{}({} argument(s))", call.function, call.args.len()))
            ))
        })
        .collect();
    // Transport the frozen ledger encoding, not a second value format.
    // Include source, residue and trace explicitly: `named` holds evaluation's
    // objects, not the envelope. Sorting also deduplicates shared values.
    let mut objects: Vec<_> = residue.named.iter().map(|(id, object)| (*id, object)).collect();
    for object in [&named, &printed_value, &trace] {
        objects.push((object.cairn(), object));
    }
    objects.sort_unstable_by_key(|(id, _)| *id);
    objects.dedup_by_key(|(id, _)| *id);
    let objects = objects
        .iter()
        .map(|(id, object)| format!("[\"{id}\",\"{}\"]", hex(&object.encode())))
        .collect::<Vec<_>>()
        .join(",");
    format!(
        "{{\"cairn\":\"{}\",\"residue\":\"{}\",\"source\":\"{}\",\"depth\":{},\"nodes\":{},\
         \"fuel_spent\":{},\"holes\":[{}],\"printed\":{},\"objects\":[{objects}]}}",
        trace.cairn(),
        residue_cairn,
        source_cairn,
        residue.depth.get(),
        residue.named.len() + 2,
        residue.fuel_spent,
        holes.join(","),
        quoted(&printed)
    )
}

fn hex(bytes: &[u8]) -> String {
    const DIGITS: &[u8] = b"0123456789abcdef";
    bytes
        .iter()
        .flat_map(|b| {
            [char::from(DIGITS[(b >> 4) as usize]), char::from(DIGITS[(b & 15) as usize])]
        })
        .collect()
}

/// What went wrong, in the shape the page already reads.
fn trouble(said: &str, note: &str) -> String {
    format!("{{\"error\":{},\"note\":{}}}", quoted(said), quoted(note))
}

/// A JSON string. Enough of §7 of RFC 8259 to be correct for what is written
/// here, which is diagnostics and source.
fn quoted(s: &str) -> String {
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
                let _ = write!(out, "\\u{:04x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}
