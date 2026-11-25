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
use nether_ledger::{Cairn, Node, Stored, Value};
use nether_syntax::{lower, parse, print};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// Browser graph caps mirrored inside WASM before hex+JSON assembly so a small
/// program depositing many large values returns a clean diagnostic instead of
/// exhausting linear memory.
const MAX_WASM_OBJECTS: usize = 20_000;
const MAX_WASM_ENCODED_BYTES: usize = 16 * 1024 * 1024;
const MAX_WASM_JSON_BYTES: usize = 34 * 1024 * 1024;

/// Hand back `len` bytes the caller may write into.
///
/// # Safety
///
/// The pointer is valid for `len` bytes until it is passed to
/// [`nether_free`] with the same length.
#[unsafe(no_mangle)]
pub extern "C" fn nether_alloc(len: usize) -> *mut u8 {
    if len == 0 {
        return core::ptr::NonNull::<u8>::dangling().as_ptr();
    }
    let Ok(layout) = std::alloc::Layout::from_size_align(len, 1) else {
        return core::ptr::null_mut();
    };
    // SAFETY: `len > 0` and alignment is 1.
    unsafe { std::alloc::alloc(layout) }
}

/// Give back what [`nether_alloc`] handed out, or what a burial returned.
///
/// # Safety
///
/// `at` came from this module and `len` is the length it was made with.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn nether_free(at: *mut u8, len: usize) {
    if at.is_null() || len == 0 {
        return;
    }
    if let Ok(layout) = std::alloc::Layout::from_size_align(len, 1) {
        // SAFETY: the caller's half of the bargain in the module documentation.
        unsafe { std::alloc::dealloc(at, layout) };
    }
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
    if source.is_null() || len > isize::MAX as usize {
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
    let Ok(len) = u32::try_from(said.len()) else {
        // Four gigabytes of JSON is not a thing this can hand back, and
        // truncating it silently would be worse than saying so.
        return block("{\"error\":\"the answer is too large to return\"}");
    };
    let Some(total) = said.len().checked_add(4) else {
        return block("{\"error\":\"the answer is too large to return\"}");
    };
    let at = nether_alloc(total);
    if at.is_null() {
        return at;
    }
    // SAFETY: `nether_alloc(total)` allocated `said.len() + 4` contiguous bytes.
    unsafe {
        core::ptr::copy_nonoverlapping(len.to_le_bytes().as_ptr(), at, 4);
        core::ptr::copy_nonoverlapping(said.as_ptr(), at.add(4), said.len());
    }
    at
}

fn parse_and_check(source: &[u8]) -> Result<(&str, nether_core::Unit), String> {
    let text = match core::str::from_utf8(source) {
        Ok(s) => s,
        Err(e) => {
            let valid = &source[..e.valid_up_to()];
            let prefix = core::str::from_utf8(valid).unwrap_or("");
            let at = u32::try_from(e.valid_up_to()).unwrap_or(u32::MAX);
            let fault = nether_syntax::Fault {
                span: nether_core::Span { start: at, end: at },
                kind: nether_syntax::FaultKind::NotUtf8,
            };
            return Err(trouble(&report(&fault.diagnostic(), prefix, "source"), ""));
        }
    };
    let format_all = |diags: Vec<nether_core::Diagnostic>| -> String {
        let mut out = String::new();
        for d in &diags {
            out.push_str(&report(d, text, "source"));
        }
        if out.is_empty() { "burial failed with no diagnostic".to_owned() } else { out }
    };
    let ast = parse(source).map_err(|faults| {
        trouble(&format_all(faults.iter().map(nether_syntax::Fault::diagnostic).collect()), "")
    })?;
    let unit = lower(&ast).map_err(|faults| {
        trouble(&format_all(faults.iter().map(nether_syntax::Fault::diagnostic).collect()), "")
    })?;
    let faults = check(&unit);
    if !faults.is_empty() {
        return Err(trouble(
            &format_all(faults.iter().map(nether_core::Fault::diagnostic).collect()),
            "",
        ));
    }
    Ok((text, unit))
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
    let (text, unit) = match parse_and_check(source) {
        Ok(pair) => pair,
        Err(err_json) => return err_json,
    };

    // §7.3.1: a span names its source by cairn, so the name exists first.
    let named = Stored::Value(Value::Bytes(source.to_vec()));
    let source_cairn = named.cairn();

    let residue = match nether_bury::bury(&unit, source_cairn, fuel) {
        Ok(residue) => residue,
        Err(halt) => return trouble(&report(&halt.diagnostic(), text, "source"), ""),
    };

    let printed = print(&residue.as_unit(&unit));
    if residue.holes.len() > MAX_WASM_OBJECTS || residue.deposits.len() > MAX_WASM_OBJECTS {
        return trouble(
            &format!(
                "burial produced too many trace entries for the browser limit of {MAX_WASM_OBJECTS}"
            ),
            "reduce the trace size or run `nether bury` natively.",
        );
    }
    if printed.len() > MAX_WASM_ENCODED_BYTES {
        return trouble(
            "burial residue exceeds the browser's 16 MiB transport limit",
            "reduce the output size or run `nether bury` natively.",
        );
    }
    let printed_value = Stored::Value(Value::Bytes(printed.as_bytes().to_vec()));
    let residue_cairn = printed_value.cairn();
    // Index `residue.named` once so hole lookups are O(log N) rather than O(H*N).
    let mut named_index: Vec<(Cairn, &Stored)> =
        residue.named.iter().map(|(id, object)| (*id, object)).collect();
    named_index.sort_unstable_by_key(|(id, _)| *id);
    named_index.dedup_by_key(|(id, _)| *id);
    let lookup = |c: Cairn| -> Option<&Stored> {
        named_index.binary_search_by_key(&c, |(id, _)| *id).ok().map(|idx| named_index[idx].1)
    };
    // §7.3.2: a trace's depth is the deepest of its residue and every hole it
    // owes, matching `nether bury` (e.g. a pure `seal` of a deep call).
    let mut deepest = residue.depth.get();
    for h in &residue.holes {
        if let Some(Stored::Node(Node::Hole { stratum, .. })) = lookup(*h) {
            deepest = deepest.max(*stratum);
        }
    }
    let trace = Stored::Node(Node::Trace {
        residue: residue_cairn,
        holes: residue.holes.clone(),
        witnesses: Vec::new(),
        deposits: residue.deposits.clone(),
        unrecorded: false,
        source: source_cairn,
        depth: deepest,
    });
    // Match `nether bury`: count `residue.named` plus residue and trace (not source).
    let node_count = residue.named.len() + 2;

    // Transport the frozen ledger encoding, not a second value format.
    // Include source, residue and trace explicitly: `named` holds evaluation's
    // objects, not the envelope. Sorting also deduplicates shared values.
    let mut objects = named_index;
    for object in [&named, &printed_value, &trace] {
        objects.push((object.cairn(), object));
    }
    objects.sort_unstable_by_key(|(id, _)| *id);
    objects.dedup_by_key(|(id, _)| *id);
    if objects.len() > MAX_WASM_OBJECTS {
        return trouble(
            &format!(
                "burial produced {} objects, past the browser limit of {MAX_WASM_OBJECTS}",
                objects.len()
            ),
            "",
        );
    }

    let mut out = String::with_capacity(1024);
    let _ = write!(
        out,
        "{{\"cairn\":\"{}\",\"residue\":\"{}\",\"source\":\"{}\",\"depth\":{},\"nodes\":{},\"fuel_spent\":{},\"holes\":[",
        trace.cairn(),
        residue_cairn,
        source_cairn,
        deepest,
        node_count,
        residue.fuel_spent,
    );
    if let Err(err_json) = append_holes(&mut out, &residue.holes, &objects) {
        return err_json;
    }
    out.push_str("],\"printed\":");
    push_quoted(&mut out, &printed);
    if out.len() > MAX_WASM_JSON_BYTES {
        return trouble(
            "burial output exceeds the browser's transport limit",
            "reduce the output size or run `nether bury` natively.",
        );
    }
    out.push_str(",\"objects\":[");
    if let Err(err_json) = append_objects(&mut out, &objects) {
        return err_json;
    }
    out.push_str("]}");
    out
}

fn append_holes(
    out: &mut String,
    holes: &[Cairn],
    objects: &[(Cairn, &Stored)],
) -> Result<(), String> {
    let mut first = true;
    for h in holes {
        let Some(Stored::Node(Node::Hole { call, stratum, .. })) =
            objects.binary_search_by_key(h, |(id, _)| *id).ok().map(|idx| objects[idx].1)
        else {
            continue;
        };
        if !first {
            out.push(',');
        }
        first = false;
        let _ = write!(out, "{{\"cairn\":\"{h}\",\"call\":");
        push_quoted(out, &call.function);
        let _ = write!(out, ",\"stratum\":{stratum},\"capability\":");
        match Depth::new(*stratum).and_then(nether_core::Capability::at) {
            Some(cap) => push_quoted(out, cap.name()),
            None => out.push_str("null"),
        }
        out.push('}');
        if out.len() > MAX_WASM_JSON_BYTES {
            return Err(trouble(
                "burial output exceeds the browser's transport limit",
                "reduce the output size or run `nether bury` natively.",
            ));
        }
    }
    Ok(())
}

fn append_objects(out: &mut String, objects: &[(Cairn, &Stored)]) -> Result<(), String> {
    let mut total_encoded: usize = 0;
    for (idx, (id, object)) in objects.iter().enumerate() {
        let raw = object.encode();
        total_encoded = total_encoded.saturating_add(raw.len());
        if total_encoded > MAX_WASM_ENCODED_BYTES {
            return Err(trouble(
                "burial objects exceed the browser's 16 MiB transport limit",
                "reduce the output size or run `nether bury` natively.",
            ));
        }
        if idx > 0 {
            out.push(',');
        }
        let _ = write!(out, "[\"{id}\",\"");
        push_hex(out, &raw);
        out.push_str("\"]");
        if out.len() > MAX_WASM_JSON_BYTES {
            return Err(trouble(
                "burial output exceeds the browser's transport limit",
                "reduce the output size or run `nether bury` natively.",
            ));
        }
    }
    Ok(())
}

fn push_hex(out: &mut String, bytes: &[u8]) {
    const DIGITS: &[u8] = b"0123456789abcdef";
    out.reserve(bytes.len() * 2);
    for &b in bytes {
        out.push(char::from(DIGITS[(b >> 4) as usize]));
        out.push(char::from(DIGITS[(b & 15) as usize]));
    }
}

/// What went wrong, in the shape the page already reads.
fn trouble(said: &str, note: &str) -> String {
    let mut out = String::with_capacity(said.len() + note.len() + 24);
    out.push_str("{\"error\":");
    push_quoted(&mut out, said);
    out.push_str(",\"note\":");
    push_quoted(&mut out, note);
    out.push('}');
    out
}

/// A JSON string. Enough of §7 of RFC 8259 to be correct for what is written
/// here, which is diagnostics and source.
fn push_quoted(out: &mut String, s: &str) {
    out.reserve(s.len() + 2);
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
}
