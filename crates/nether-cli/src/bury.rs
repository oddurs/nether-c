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

use nether_bury::bury;
use nether_core::{Capability, Depth};
use nether_ledger::{Cairn, Node, Store, Stored, Stratum, Value};
use nether_syntax::print;

use crate::memo::Remembered;
use crate::{
    DEFAULT_FUEL, FAILED, bad_usage, code, json, parse_fuel, split_flag, take_val, usage_error,
};

pub(crate) const USAGE: &str = "usage: nether bury <file.nc> [--fuel <n>] [--json]";

/// `nether bury <file.nc> [--fuel <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut fuel: Option<u64> = None;
    let mut granted: Vec<Capability> = Vec::new();
    let mut path: Option<&str> = None;
    let mut end_opts = false;

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if end_opts {
            if path.is_some() {
                return bad_usage("one file at a time", USAGE);
            }
            path = Some(arg.as_str());
            continue;
        }
        let (flag, inline) = split_flag(arg);
        match (flag, inline) {
            ("--", None) => end_opts = true,
            ("--help" | "-h", None) => {
                println!("{USAGE}");
                return ExitCode::SUCCESS;
            }
            ("--json", None) => wants_json = true,
            ("--fuel", inline) => {
                if let Err(c) = parse_fuel(&mut fuel, take_val(inline, &mut rest), USAGE) {
                    return c;
                }
            }
            ("--grant", inline) => match take_val(inline, &mut rest).map(Capability::from_name) {
                Some(Some(cap)) => granted.push(cap),
                Some(None) => {
                    eprintln!("nether: no capability by that name");
                    eprintln!(
                        "        §9.1 lists them: store env disk disk! net net! entropy unrecorded"
                    );
                    return usage_error();
                }
                None => return bad_usage("`--grant` wants a capability", USAGE),
            },
            (other, _) if other.starts_with('-') && other != "-" => {
                return bad_usage(&format!("unknown option `{arg}`"), USAGE);
            }
            _ if path.is_some() => return bad_usage("one file at a time", USAGE),
            _ => path = Some(arg.as_str()),
        }
    }

    let Some(path) = path else {
        eprintln!("{USAGE}");
        return usage_error();
    };

    // A grant is refused rather than accepted and discarded. Honouring one is
    // `exhume`'s work, so a `--grant` here would validate its argument, change
    // nothing, and look like it had worked. §8.2, and the same reasoning as §8.0.
    if let Some(cap) = granted.first() {
        eprintln!(
            "nether: `bury` never grants capabilities; bury first, then `nether exhume --grant`."
        );
        eprintln!();
        eprintln!("  §8.2: burial holds no capabilities. Every world-touching expression");
        eprintln!("  becomes a hole, and answering one is what `exhume` is for.");
        eprintln!();
        eprintln!("    nether bury {path}");
        eprintln!("    nether exhume <cairn> --grant {cap}");
        return usage_error();
    }
    match inter(Path::new(path), fuel.unwrap_or(DEFAULT_FUEL), wants_json) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

fn inter(path: &Path, fuel: u64, wants_json: bool) -> Result<(), ExitCode> {
    let source = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("nether: {}: {e}", path.display());
            return Err(FAILED);
        }
    };

    // Source has to be text to be pointed at: every diagnostic underlines a
    // span, and a span is a byte range in something with lines in it.
    let Ok(text) = String::from_utf8(source.clone()) else {
        eprintln!("nether: {} is not UTF-8", path.display());
        return Err(ExitCode::from(code::MALFORMED));
    };
    let shown = path.display().to_string();
    let unit = crate::closing::checked_unit(&source, &shown)?;

    // Compute the source cairn without mutating the store so a remembered
    // burial (§8.2.1) or a halting burial creates nothing on disk.
    let source_stored = Stored::Value(Value::Bytes(source));
    let source_cairn = source_stored.cairn();

    // §8.2.1: a burial this implementation already did, within this budget,
    // is reported and not done again.
    let ro_store = crate::opened_readonly();
    let memo = crate::memo::Memo::of(source_cairn);
    let recalled = memo.as_ref().and_then(|m| m.recall(&ro_store, source_cairn, fuel));
    let remembered = recalled.is_some();
    let (store, done) = if let Some(r) = recalled {
        (ro_store, r)
    } else {
        let residue = bury(&unit, source_cairn, fuel)
            .map_err(|h| crate::closing::halt_exit(&h, &text, &shown))?;

        // Source, everything burial named, then the residue as source, then the trace.
        // Nothing was written (and no store directory created) until burial finished.
        let store = crate::opened()?;
        crate::kept(&store, &source_stored)?;
        let mut extra = Vec::new();
        for (c, stored) in &residue.named {
            crate::kept(&store, stored)?;
            if matches!(stored, Stored::Value(_)) {
                extra.push(*c);
            }
        }
        let printed = print(&residue.as_unit(&unit));
        let residue_cairn =
            crate::kept(&store, &Stored::Value(Value::Bytes(printed.into_bytes())))?;

        // Burial holds no capability, so nothing was answered and there are no
        // witnesses. §6.6: exhumation is what records one.
        let witnesses: Vec<Cairn> = Vec::new();
        let depth = crate::closing::depth(
            &store,
            crate::closing::stratum(residue.depth),
            &residue.holes,
            &witnesses,
        )?;
        let trace = Node::Trace {
            residue: residue_cairn,
            holes: residue.holes.clone(),
            unrecorded: crate::closing::unrecorded(&store, &witnesses)?,
            depth,
            witnesses,
            deposits: residue.deposits.clone(),
            source: source_cairn,
        };
        let trace = crate::kept(&store, &Stored::Node(trace))?;
        let done =
            Remembered { trace, steps: residue.fuel_spent, nodes: residue.named.len() + 2, extra };
        if let Some(memo) = &memo {
            memo.record(&done);
        }
        (store, done)
    };

    // Read back rather than carried, so a remembered burial and a fresh one
    // are reported by one path from one place: the trace.
    let Ok(Stored::Node(Node::Trace { residue, holes, depth, .. })) = store.get(done.trace) else {
        eprintln!("nether: trace {} is not readable back", done.trace.short());
        return Err(FAILED);
    };
    let told = Buried {
        path: path.display().to_string(),
        cairn: done.trace,
        residue,
        source: source_cairn,
        depth,
        holes,
        nodes: done.nodes,
        fuel_spent: done.steps,
        remembered,
    };
    if wants_json {
        println!("{}", told.json(&store));
    } else {
        print!("{}", told.text(&store));
    }
    Ok(())
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
    depth: Stratum,
    holes: Vec<Cairn>,
    nodes: usize,
    /// What this burial spent. §8.2: reported by the rite, and not recorded in
    /// the trace, because a trace holding it could not be replayed (§6.7).
    fuel_spent: u64,
    /// §8.2.1: whether the rite remembered this burial rather than doing it.
    /// The steps are the burial's either way.
    remembered: bool,
}

impl Buried {
    fn text(&self, store: &Store) -> String {
        let mut out = format!(
            "buried   {} → {}   depth {}   holes {}   {} nodes{}\n",
            self.path,
            self.cairn.short(),
            self.depth,
            self.holes.len(),
            self.nodes,
            if self.remembered { "   remembered" } else { "" }
        );
        for (n, hole) in self.holes.iter().enumerate() {
            let numeral = circled(n + 1);
            match store.get(*hole) {
                Ok(Stored::Node(Node::Hole { call, stratum, .. })) => {
                    let said = crate::lamp::said(store, call.function(), call.args());
                    let cap = Depth::new(stratum.as_u8())
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
                    json::string(call.function())
                ),
                _ => format!("{{\"cairn\":\"{h}\"}}"),
            })
            .collect();
        let named = format!(
            "\"buried\":{},\"cairn\":\"{}\",\"residue\":\"{}\",\"source\":\"{}\"",
            json::string(&self.path),
            self.cairn,
            self.residue,
            self.source
        );
        format!(
            "{{{named},\"depth\":{},\"nodes\":{},\"fuel_spent\":{},\"remembered\":{},\"holes\":[{}]}}",
            self.depth,
            self.nodes,
            self.fuel_spent,
            self.remembered,
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
