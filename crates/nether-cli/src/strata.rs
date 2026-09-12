//! `nether strata` — blame for depth.
//!
//! `spec/08-rites.md` §8.6. "Why is this value at depth 5" has to be a
//! command, not an investigation, so the whole answer is one screen: the
//! stratum the trace reached, and every place it went to the world.

use std::cmp::Reverse;
use std::collections::HashSet;
use std::process::ExitCode;

use nether_core::{Capability, Depth};
use nether_ledger::{Cairn, Node, Span, Store, Stored, Value};

use crate::{FAILED, code, json, lamp, ledger, usage_error};

/// `nether strata <cairn> [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let wants_json = args.iter().any(|a| a == "--json");
    let rest: Vec<&str> = args.iter().filter(|a| *a != "--json").map(String::as_str).collect();
    let [name] = rest.as_slice() else {
        return complain();
    };
    if name.starts_with("--") {
        return complain();
    }

    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let cairn = match store.resolve(name) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return ExitCode::from(code::ABSENT);
        }
    };
    let stored = match store.get(cairn) {
        Ok(stored) => stored,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let Stored::Node(Node::Trace { roots, depth, unrecorded, .. }) = &stored else {
        eprintln!("nether: {} is {}, not a trace", cairn.short(), kind(&stored));
        eprintln!("        `nether lamp {}` shows what is there", cairn.short());
        return FAILED;
    };

    let found = survey(&store, roots);
    let told = Reading { cairn, depth: *depth, unrecorded: *unrecorded, found };
    if wants_json {
        println!("{}", told.json(&store));
    } else {
        print!("{}", told.text(&store));
    }
    // A trace that under-reports its own depth is a trace lying about the one
    // thing it is for. §8.8 calls that malformed.
    if told.deepest_answered() > told.depth {
        eprintln!(
            "nether: this trace records depth {}, and a witness under it reached {}",
            told.depth,
            told.deepest_answered()
        );
        return ExitCode::from(code::MALFORMED);
    }
    ExitCode::SUCCESS
}

fn complain() -> ExitCode {
    eprintln!("usage: nether strata <cairn> [--json]");
    usage_error()
}

/// What a cairn turned out to name, for the sentence that says it is not a trace.
fn kind(stored: &Stored) -> &'static str {
    match stored {
        Stored::Value(_) => "a value",
        Stored::Node(Node::Literal(_)) => "a literal",
        Stored::Node(Node::Apply { .. }) => "an application",
        Stored::Node(Node::Hole { .. }) => "a hole",
        Stored::Node(Node::Deposit { .. }) => "a deposit",
        Stored::Node(Node::Witness { .. }) => "a witness",
        Stored::Node(Node::Trace { .. }) => "a trace",
    }
}

// ── what is under a trace ───────────────────────────────────────────────────

/// One question put to the world, and how deep it went.
struct Reach {
    stratum: u8,
    /// Whether the world has answered.
    ///
    /// §7.3 makes the difference: a witness holds "the stratum that was
    /// reached", a hole "the stratum the call would reach". One is history and
    /// the other is a bill.
    answered: bool,
    call: String,
    at: Span,
}

/// Everything under a trace that touches the world.
struct Survey {
    reaches: Vec<Reach>,
    /// Nodes the graph names that the ledger does not hold.
    ///
    /// Nodes, not cairns. A source blob that was never sealed is the ordinary
    /// state of a ledger and bounds nothing about which strata were reached;
    /// only a node that is not here can be hiding one.
    missing: usize,
}

/// Deepest, then where, then what: the order the report is read in.
fn order(r: &Reach) -> (Reverse<u8>, [u8; 32], u64, bool, &str) {
    (Reverse(r.stratum), *r.at.source.as_bytes(), r.at.start, r.answered, &r.call)
}

/// Walk the graph from a trace's roots and collect every hole and witness.
///
/// Along `nodes` and not `references`: an argument, an answer and a span's
/// source are values, and reading them would say nothing about depth while
/// making every unsealed source look like a gap in the record. §7.3 makes the
/// graph acyclic by construction, so `seen` is an economy and not a safety
/// measure.
fn survey(store: &Store, roots: &[Cairn]) -> Survey {
    let mut raw: Vec<Reach> = Vec::new();
    let mut missing = 0;
    let mut seen = HashSet::new();
    let mut stack: Vec<Cairn> = roots.to_vec();
    while let Some(at) = stack.pop() {
        if !seen.insert(at) {
            continue;
        }
        let Ok(stored) = store.get(at) else {
            missing += 1;
            continue;
        };
        let Stored::Node(n) = &stored else { continue };
        match n {
            Node::Hole { call, stratum, span, .. } => raw.push(Reach {
                stratum: *stratum,
                answered: false,
                call: lamp::said(store, &call.function, &call.args),
                at: *span,
            }),
            Node::Witness { call, stratum, span, .. } => raw.push(Reach {
                stratum: *stratum,
                answered: true,
                call: lamp::said(store, &call.function, &call.args),
                at: *span,
            }),
            _ => {}
        }
        stack.extend(n.nodes());
    }

    // Deepest first, because that is the line the reader came for; then in
    // source order, so the rest reads like the program.
    //
    // Nothing is collapsed. A node is named by its content and `seen` is keyed
    // on that, so two entries here are two nodes — and two witnesses of one
    // call at one place are two different answers, which is the thing a blame
    // tool exists to show rather than tidy away.
    let mut reaches = raw;
    reaches.sort_by(|a, b| order(a).cmp(&order(b)));
    Survey { reaches, missing }
}

// ── saying it ───────────────────────────────────────────────────────────────

/// A trace, read for depth.
struct Reading {
    cairn: Cairn,
    depth: u8,
    unrecorded: bool,
    found: Survey,
}

impl Reading {
    /// The deepest stratum the world actually answered at.
    fn deepest_answered(&self) -> u8 {
        self.found.reaches.iter().filter(|r| r.answered).map(|r| r.stratum).max().unwrap_or(0)
    }

    /// The deepest stratum a hole still owes.
    fn deepest_pending(&self) -> u8 {
        self.found.reaches.iter().filter(|r| !r.answered).map(|r| r.stratum).max().unwrap_or(0)
    }

    fn text(&self, store: &Store) -> String {
        use core::fmt::Write as _;

        let mut out = String::new();
        let _ = writeln!(out, "depth {}   {}", self.depth, stratum_name(self.depth));
        let _ = writeln!(out);

        if self.found.reaches.is_empty() {
            let _ = writeln!(out, "  nothing reached the world");
        } else {
            let places: Vec<String> =
                self.found.reaches.iter().map(|r| place(store, r.at)).collect();
            // Wide enough for the widest call, and never so narrow that the
            // positions crowd the calls.
            let width = self
                .found
                .reaches
                .iter()
                .map(|r| r.call.chars().count())
                .max()
                .unwrap_or(0)
                .max(24);
            for (r, at) in self.found.reaches.iter().zip(&places) {
                let _ = write!(out, "  {}  {:width$}  {at}", r.stratum, r.call);
                if !r.answered {
                    let _ = write!(out, "   pending");
                }
                let _ = writeln!(out);
            }
            let _ = writeln!(out, "  0  everything else");
        }
        let _ = writeln!(out);

        let _ = writeln!(out, "  replayable: {}", if self.unrecorded { "no" } else { "yes" });
        for note in self.notes() {
            let _ = writeln!(out, "  {note}");
        }
        out
    }

    /// What the reader has to be told without asking.
    fn notes(&self) -> Vec<String> {
        let mut notes = Vec::new();
        if self.unrecorded {
            // §9.8: naming the symbol is the minimum, and the symbol is in the
            // call this already printed.
            notes.push(
                "stratum 8 was reached. Nothing downstream of it is recorded, ever.".to_string(),
            );
        }
        let pending = self.deepest_pending();
        if pending > self.depth {
            notes.push(format!(
                "holes reach {pending} ({}); exhuming this trace will take it there.",
                stratum_name(pending)
            ));
        }
        if self.found.missing > 0 {
            notes.push(format!(
                "{} node(s) under this trace are not in this ledger, so this is a lower bound.",
                self.found.missing
            ));
        }
        notes
    }

    fn json(&self, store: &Store) -> String {
        let reaches: Vec<String> = self
            .found
            .reaches
            .iter()
            .map(|r| {
                format!(
                    "{{\"stratum\":{},\"call\":{},\"at\":{},\"answered\":{}}}",
                    r.stratum,
                    json::string(&r.call),
                    json::string(&place(store, r.at)),
                    r.answered
                )
            })
            .collect();
        let notes: Vec<String> = self.notes().iter().map(|n| json::string(n)).collect();
        format!(
            "{{\"cairn\":\"{}\",\"depth\":{},\"stratum\":{},\"replayable\":{},\"unrecorded\":{},\
             \"missing\":{},\"reaches\":[{}],\"notes\":[{}]}}",
            self.cairn,
            self.depth,
            json::string(stratum_name(self.depth)),
            !self.unrecorded,
            self.unrecorded,
            self.found.missing,
            reaches.join(","),
            notes.join(",")
        )
    }
}

/// The name §9.1 gives a stratum. Nothing grants 0, and nothing has to.
fn stratum_name(stratum: u8) -> &'static str {
    Depth::new(stratum).and_then(Capability::at).map_or("pure", Capability::name)
}

/// A position, named the way §8.1 requires: by the cairn of its source.
///
/// A path is a fact about one machine. If the ledger still holds the source
/// the offset becomes a line and a column; if it does not, the offset is the
/// most that can honestly be said.
fn place(store: &Store, at: Span) -> String {
    let Some(text) = source(store, at.source) else {
        return format!("{}:+{}", at.source.short(), at.start);
    };
    // An offset past the end of the source, or past what this machine can
    // index, is the end of the source: a span is a claim about bytes, and
    // those are all the bytes there are.
    let mut start = usize::try_from(at.start).map_or(text.len(), |n| n.min(text.len()));
    while !text.is_char_boundary(start) {
        start -= 1;
    }
    let before = &text[..start];
    let line = before.matches('\n').count() + 1;
    let line_start = before.rfind('\n').map_or(0, |i| i + 1);
    let column = text[line_start..start].chars().count() + 1;
    format!("{}:{line}:{column}", at.source.short())
}

/// The source a span names, if the ledger still holds it as text.
fn source(store: &Store, cairn: Cairn) -> Option<String> {
    match store.get(cairn) {
        Ok(Stored::Value(Value::Bytes(b))) => String::from_utf8(b).ok(),
        Ok(Stored::Value(Value::Str(s))) => Some(s),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::stratum_name;

    /// §9.1's table, read back. A stratum the CLI names differently from the
    /// specification is a stratum the reader cannot look up.
    #[test]
    fn every_stratum_is_named_as_section_09_names_it() {
        let named = [
            (0, "pure"),
            (1, "store"),
            (2, "env"),
            (3, "disk"),
            (4, "disk!"),
            (5, "net"),
            (6, "net!"),
            (7, "entropy"),
            (8, "unrecorded"),
        ];
        for (stratum, name) in named {
            assert_eq!(stratum_name(stratum), name, "stratum {stratum}");
        }
    }
}
