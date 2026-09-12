//! `nether bury` — evaluate as far as the world allows, and write a trace.
//!
//! `spec/08-rites.md` §8.2.
//!
//! The primary verb. Everything a burial learns goes into the ledger; what
//! reaches the terminal is a summary of what was written, never the thing
//! itself. A program cannot print, and neither can burying one.

use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use nether_bury::{HaltKind, bury};
use nether_core::{Capability, Depth, Diagnostic, check, report};
use nether_ledger::{Cairn, Node, Store, Stored, Value};
use nether_syntax::{lower, parse, print};

use crate::{FAILED, code, json, ledger, usage_error};

/// The budget a burial runs under when nobody says otherwise.
///
/// §8.2 requires this to be finite and to be recorded, because a trace buried
/// under a different budget is a different trace.
const DEFAULT_FUEL: u64 = 1_000_000;

/// `nether bury <file.nc> [--grant <cap>]... [--fuel <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut fuel = DEFAULT_FUEL;
    let mut granted: Vec<Capability> = Vec::new();
    let mut path: Option<&str> = None;

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => wants_json = true,
            "--fuel" => {
                if let Some(Ok(n)) = rest.next().map(|n| n.parse::<u64>()) {
                    fuel = n;
                } else {
                    eprintln!("nether: --fuel wants a number of steps");
                    return usage_error();
                }
            }
            "--grant" => match rest.next().map(|c| Capability::from_name(c)) {
                Some(Some(cap)) => granted.push(cap),
                Some(None) => {
                    eprintln!("nether: no capability by that name");
                    eprintln!(
                        "        §9.1 lists them: store env disk disk! net net! entropy unrecorded"
                    );
                    return usage_error();
                }
                None => {
                    eprintln!("nether: --grant wants a capability");
                    return usage_error();
                }
            },
            other if other.starts_with("--") => {
                eprintln!("nether: unknown option {other}");
                return usage_error();
            }
            other => path = Some(other),
        }
    }

    let Some(path) = path else {
        eprintln!("usage: nether bury <file.nc> [--grant <cap>]... [--fuel <n>] [--json]");
        return usage_error();
    };

    // A grant is refused rather than accepted and discarded. Honouring one is
    // `exhume`'s work and that rite is not built, so a `--grant` here would
    // validate its argument, change nothing, and look like it had worked.
    // §8.2, and the same reasoning as §8.0.
    if let Some(cap) = granted.first() {
        eprintln!("nether: `bury` cannot honour a grant yet, so it will not take one.");
        eprintln!();
        eprintln!("  Burial holds no capabilities: every world-touching expression");
        eprintln!("  becomes a hole, and answering one is what `exhume` is for.");
        eprintln!();
        eprintln!("    nether bury {path}");
        eprintln!("    nether exhume <cairn> --grant {cap}");
        return usage_error();
    }
    inter(Path::new(path), fuel, wants_json)
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass, in the order it happens; splitting it would hide the order"
)]
fn inter(path: &Path, fuel: u64, wants_json: bool) -> ExitCode {
    let source = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("nether: {}: {e}", path.display());
            return FAILED;
        }
    };

    // Source has to be text to be pointed at: every diagnostic underlines a
    // span, and a span is a byte range in something with lines in it.
    let Ok(text) = String::from_utf8(source.clone()) else {
        eprintln!("nether: {} is not UTF-8", path.display());
        return ExitCode::from(code::MALFORMED);
    };
    let shown = path.display().to_string();
    let complain = |d: &Diagnostic| eprint!("{}", report(d, &text, &shown));

    let ast = match parse(&source) {
        Ok(ast) => ast,
        Err(faults) => {
            for fault in &faults {
                complain(&fault.diagnostic());
            }
            return ExitCode::from(code::MALFORMED);
        }
    };
    let unit = match lower(&ast) {
        Ok(unit) => unit,
        Err(faults) => {
            for fault in &faults {
                complain(&fault.diagnostic());
            }
            return ExitCode::from(code::MALFORMED);
        }
    };
    // The depth checker runs before burial, not after. A program that does not
    // check is a program with no depths, and burying one would mean deciding
    // what it meant.
    let faults = check(&unit);
    if !faults.is_empty() {
        for fault in &faults {
            complain(&fault.diagnostic());
        }
        return ExitCode::from(code::MALFORMED);
    }

    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    // The source is named before anything is buried. A span points at a cairn
    // rather than a path (§7.3.1), so the name has to exist first.
    let source_cairn = match store.put(&Stored::Value(Value::Bytes(source.clone()))) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    let residue = match bury(&unit, source_cairn, fuel) {
        Ok(residue) => residue,
        Err(halt) => {
            complain(&halt.diagnostic());
            return match halt.kind {
                // §8.8 gives fuel its own code: it is the one failure a larger
                // budget might fix.
                HaltKind::OutOfFuel { .. } => ExitCode::from(code::FUEL),
                _ => FAILED,
            };
        }
    };

    // Everything burial named, then the residue as source, then the trace.
    // Nothing was written until now: burial holds no capability at all, and
    // writing to a store is stratum 1.
    let mut written = 0usize;
    for (_, stored) in &residue.named {
        match store.put(stored) {
            Ok(_) => written += 1,
            Err(e) => {
                eprintln!("nether: {e}");
                return FAILED;
            }
        }
    }

    let printed = print(&residue.as_unit(&unit));
    let residue_cairn = match store.put(&Stored::Value(Value::Bytes(printed.into_bytes()))) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    // Burial holds no capability, so nothing was answered and there are no
    // witnesses. §6.6: exhumation is what records one.
    let witnesses: Vec<Cairn> = Vec::new();

    let trace = Node::Trace {
        residue: residue_cairn,
        holes: residue.holes.clone(),
        // §1.7: marked when something *reached* stratum 8, which a hole at 8
        // has not. Derived rather than written down, so that it stays true the
        // day `exhume` starts answering one.
        unrecorded: witnesses.iter().any(|w| reached_the_bottom(&store, *w)),
        witnesses,
        deposits: residue.deposits.clone(),
        source: source_cairn,
        fuel_spent: residue.fuel_spent,
        // §7.3.2: the join of what the residue still reaches and what a
        // witness reached. Nothing has been answered, so it is the first.
        depth: residue.depth.get(),
    };
    let trace_cairn = match store.put(&Stored::Node(trace)) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    let told = Buried {
        path: path.display().to_string(),
        cairn: trace_cairn,
        residue: residue_cairn,
        source: source_cairn,
        depth: residue.depth.get(),
        holes: residue.holes.clone(),
        nodes: written + 2,
        fuel_spent: residue.fuel_spent,
    };
    if wants_json {
        println!("{}", told.json(&store));
    } else {
        print!("{}", told.text(&store));
    }
    ExitCode::SUCCESS
}

/// Whether that witness is one §1.7 marks a trace for.
fn reached_the_bottom(store: &Store, witness: Cairn) -> bool {
    matches!(store.get(witness), Ok(Stored::Node(Node::Witness { stratum: 8, .. })))
}

/// The summary §8.2 requires: what was written, never what the program said.
struct Buried {
    path: String,
    cairn: Cairn,
    /// The residue, as source. §6.5 makes it a MUST that this can be printed
    /// and lowered again, and a person who cannot get at its name cannot
    /// check that.
    residue: Cairn,
    source: Cairn,
    depth: u8,
    holes: Vec<Cairn>,
    nodes: usize,
    fuel_spent: u64,
}

impl Buried {
    fn text(&self, store: &Store) -> String {
        let mut out = format!(
            "buried   {} → {}   depth {}   holes {}   {} nodes\n",
            self.path,
            self.cairn.short(),
            self.depth,
            self.holes.len(),
            self.nodes
        );
        for (n, hole) in self.holes.iter().enumerate() {
            let numeral = circled(n + 1);
            match store.get(*hole) {
                Ok(Stored::Node(Node::Hole { call, stratum, .. })) => {
                    let said = crate::lamp::said(store, &call.function, &call.args);
                    let cap = Depth::new(stratum)
                        .and_then(Capability::at)
                        .map_or_else(String::new, |c| format!("  {}", c.name()));
                    let _ = writeln!(out, "  hole {numeral}  {said:<30} stratum {stratum}{cap}");
                }
                _ => {
                    let _ = writeln!(out, "  hole {numeral}  {}", hole.short());
                }
            }
        }
        out
    }

    fn json(&self, store: &Store) -> String {
        let holes: Vec<String> = self
            .holes
            .iter()
            .map(|h| match store.get(*h) {
                Ok(Stored::Node(Node::Hole { call, stratum, .. })) => format!(
                    "{{\"cairn\":\"{h}\",\"call\":{},\"stratum\":{stratum}}}",
                    json::string(&call.function)
                ),
                _ => format!("{{\"cairn\":\"{h}\"}}"),
            })
            .collect();
        format!(
            "{{\"buried\":{},\"cairn\":\"{}\",\"residue\":\"{}\",\"source\":\"{}\",             \"depth\":{},\"nodes\":{},\"fuel_spent\":{},\"holes\":[{}]}}",
            json::string(&self.path),
            self.cairn,
            self.residue,
            self.source,
            self.depth,
            self.nodes,
            self.fuel_spent,
            holes.join(",")
        )
    }
}

/// ①..⑳, and a plain number after that.
pub fn circled(n: usize) -> String {
    if (1..=20).contains(&n) {
        char::from_u32(0x245F + u32::try_from(n).unwrap_or(0))
            .map_or_else(|| n.to_string(), |c| c.to_string())
    } else {
        n.to_string()
    }
}

/// How big an answer is, the way §6.6 writes it.
///
/// Bytes get counted, because "11,204 bytes" is what a reader wants from a
/// file; anything else is rendered, because it is small enough to read.
pub fn measure(v: &Value) -> String {
    match v {
        Value::Answer(a) => match a.as_ref() {
            nether_ledger::AnswerOf::Given(v) => measure(v),
            nether_ledger::AnswerOf::Refused(_) => crate::lamp::value(v),
        },
        Value::Bytes(b) => format!("{} bytes", nether_core::grouped(b.len() as u64)),
        other => crate::lamp::value(other),
    }
}
