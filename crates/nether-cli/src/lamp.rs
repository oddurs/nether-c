//! `nether lamp` — carry light down.
//!
//! `spec/08-rites.md` §8.4. A tool the operator carries, not a capability the
//! program holds: a program cannot invoke it, cannot reach a terminal, and has
//! no way to know whether anyone is looking. That is what makes it
//! structurally impossible for a Nether C program to leak to a log.

use std::process::ExitCode;

use nether_ledger::{AnswerOf, Cairn, Node, Refusal, Store, Stored, Value};

use crate::{FAILED, code, json, ledger, usage_error};

/// How far a provenance walk goes when nobody says.
const DEFAULT_DEPTH: usize = 8;

/// `nether lamp <cairn> [--provenance] [--depth <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut name = None;
    let mut backwards = false;
    let mut wants_json = false;
    let mut depth = DEFAULT_DEPTH;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--provenance" => backwards = true,
            "--json" => wants_json = true,
            "--depth" => match rest.next().and_then(|n| n.parse().ok()) {
                Some(n) => depth = n,
                None => return complain("`--depth` wants a number"),
            },
            other if other.starts_with("--") => {
                return complain("that is not a flag `lamp` has");
            }
            other if name.is_none() => name = Some(other.to_string()),
            _ => return complain("one cairn at a time"),
        }
    }
    let Some(name) = name else {
        return complain("which cairn?");
    };

    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let cairn = match store.resolve(&name) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return ExitCode::from(code::ABSENT);
        }
    };
    if backwards {
        provenance(&store, cairn, depth, wants_json)
    } else {
        render(&store, cairn, wants_json)
    }
}

fn complain(what: &str) -> ExitCode {
    eprintln!("nether: {what}");
    eprintln!("usage: nether lamp <cairn> [--provenance] [--depth <n>] [--json]");
    usage_error()
}

// ── forwards: what is there ─────────────────────────────────────────────────

fn render(store: &Store, cairn: Cairn, wants_json: bool) -> ExitCode {
    let stored = match store.get(cairn) {
        Ok(stored) => stored,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let text = match &stored {
        Stored::Value(v) => value(v),
        // A trace is read by what it left behind: every deposit under it, in
        // the order the program made them. §8.4.
        Stored::Node(Node::Trace { .. }) => {
            let found = deposits(store, cairn);
            // Concatenated, not joined. A deposit is what the program wrote,
            // and a program that wants a newline between two of them writes
            // one — `"a\n"` and `"b\n"` are two lines, not two lines with a
            // blank between. §8.4: lamp renders the value.
            if found.is_empty() { "nothing was deposited\n".to_string() } else { found.concat() }
        }
        Stored::Node(n) => node(store, n),
    };
    if wants_json {
        println!("{{\"cairn\":\"{cairn}\",\"shows\":{}}}", json::string(&text));
    } else if text.ends_with('\n') {
        // Exactly the bytes. A value that ends in a newline has one already,
        // and adding a second is the lamp deciding what the program meant.
        print!("{text}");
    } else {
        println!("{text}");
    }
    ExitCode::SUCCESS
}

/// Every `Deposit` under a trace, rendered, in the order they were written.
///
/// A deposit is not printed when it is made — nothing in Nether C is printed.
/// It is read afterwards, by somebody who decided to go and look.
fn deposits(store: &Store, root: Cairn) -> Vec<String> {
    let mut found = Vec::new();
    let mut seen = std::collections::HashSet::new();
    let mut stack = vec![root];
    while let Some(at) = stack.pop() {
        if !seen.insert(at) {
            continue;
        }
        let Ok(stored) = store.get(at) else { continue };
        let Stored::Node(n) = &stored else { continue };
        if let Node::Deposit { value: held, span } = n {
            if let Ok(Stored::Value(v)) = store.get(*held) {
                found.push((*span.source.as_bytes(), span.start, value(&v)));
            }
        }
        stack.extend(n.references());
    }
    // §8.4: grouped by source, then by offset within it. Sorting on the offset
    // alone interleaved two sources by byte position, which is an order
    // corresponding to nothing anybody wrote.
    found.sort_by_key(|(source, at, _)| (*source, *at));
    found.into_iter().map(|(_, _, text)| text).collect()
}

/// A value, as a person reads one.
pub fn value(v: &Value) -> String {
    match v {
        // Text comes out as text. This is the whole of what a lamp is for.
        Value::Str(s) => s.clone(),
        Value::Bytes(b) => match core::str::from_utf8(b) {
            Ok(s) => s.to_string(),
            Err(_) => format!("{} bytes, not text", b.len()),
        },
        Value::Int(n) => n.to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Unit => "()".to_string(),
        Value::Cairn(c) => c.to_string(),
        // Only the origin and the name, which is all a shade is. Going further
        // means lamping what is inside it, deliberately.
        Value::Shade { origin, value } => {
            format!("shade from stratum {origin} of {}", value.short())
        }
        Value::Struct { name, fields } => {
            let inside: Vec<String> = fields.iter().map(value).collect();
            format!("{name} {{ {} }}", inside.join(", "))
        }
        Value::Array(items) => {
            let inside: Vec<String> = items.iter().map(value).collect();
            format!("[{}]", inside.join(", "))
        }
        // A refusal reads as the word, not as its code. Which no it was is
        // what a person wants; the code is for the encoding.
        Value::Refusal(r) => refusal(*r).to_string(),
        // An answer says which it is, because "absent" and a value that
        // happens to be the string "absent" must not read the same.
        Value::Answer(a) => match a.as_ref() {
            AnswerOf::Given(v) => format!("given {}", value(v)),
            AnswerOf::Refused(r) => format!("refused: {}", refusal(*r)),
        },
    }
}

/// The word for a refusal. `spec/05-types.md` §5.1.1.
const fn refusal(r: Refusal) -> &'static str {
    match r {
        Refusal::Absent => "absent",
        Refusal::Denied => "denied",
        Refusal::Malformed => "malformed",
        Refusal::Unreachable => "unreachable",
        Refusal::Exhausted => "exhausted",
        Refusal::Conflict => "conflict",
    }
}

/// A node, as one line.
fn node(store: &Store, n: &Node) -> String {
    match n {
        Node::Literal(v) => value(v),
        Node::Apply { function, args, result } => {
            let args: Vec<String> = args.iter().map(Cairn::short).collect();
            format!("apply    {}({})  →  {}", function.short(), args.join(", "), result.short())
        }
        Node::Hole { call, stratum, .. } => {
            format!("hole     {}   stratum {stratum}", said(store, &call.function, &call.args))
        }
        Node::Witness { call, stratum, answer, .. } => format!(
            "witness  {}   stratum {stratum}   →  {}",
            said(store, &call.function, &call.args),
            answer.short()
        ),
        Node::Deposit { value: v, span } => match store.get(*v) {
            Ok(Stored::Value(v)) => format!("deposit  {}   at {}", value(&v), span.start),
            _ => format!("deposit  {}", v.short()),
        },
        Node::Trace { residue, holes, witnesses, deposits, depth, unrecorded, .. } => {
            let mark = if *unrecorded { "   UNRECORDED" } else { "" };
            // The residue is named here because nothing else names it, and
            // §6.5's MUST is about being able to read it back.
            let held = format!(
                "{} hole(s)   {} witness(es)   {} deposit(s)",
                holes.len(),
                witnesses.len(),
                deposits.len()
            );
            format!("trace    depth {depth}   residue {}   {held}{mark}", residue.short())
        }
    }
}

/// A call, with its arguments rendered if the ledger still holds them.
pub fn said(store: &Store, function: &str, args: &[Cairn]) -> String {
    let shown: Vec<String> = args
        .iter()
        .map(|a| match store.get(*a) {
            Ok(Stored::Value(v)) => as_written(&v),
            _ => a.short(),
        })
        .collect();
    format!("{function}({})", shown.join(", "))
}

/// An argument, the way a program would have written it.
///
/// Not [`value`]. A lamp renders a value for a person to read, and text comes
/// out as text — but a call is read back as the call that was made, so its
/// arguments keep the delimiters §03 gives them. Without that an empty `Bytes`
/// vanished and one holding a comma made a line that could not be read at all.
fn as_written(v: &Value) -> String {
    match v {
        Value::Str(s) => format!("{s:?}"),
        Value::Bytes(b) => match core::str::from_utf8(b) {
            Ok(s) => format!("b{s:?}"),
            // No literal form, so say what is there rather than pretend.
            Err(_) => format!("<{} bytes>", b.len()),
        },
        _ => value(v),
    }
}

// ── backwards: where it came from ───────────────────────────────────────────

/// §7.4: provenance is the forward edge, read backwards. A node names what it
/// was made from, so what a value was made *for* is found by asking the store
/// who names it.
fn provenance(store: &Store, cairn: Cairn, limit: usize, wants_json: bool) -> ExitCode {
    let head = match store.get(cairn) {
        Ok(Stored::Value(v)) => format!("{}  {}", cairn.short(), value(&v)),
        Ok(Stored::Node(n)) => format!("{}  {}", cairn.short(), node(store, &n)),
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let mut lines = vec![head];
    walk(store, cairn, limit, 0, &mut lines);
    if wants_json {
        let each: Vec<String> = lines.iter().map(|l| json::string(l)).collect();
        println!("{{\"cairn\":\"{cairn}\",\"provenance\":[{}]}}", each.join(","));
    } else {
        println!("{}", lines.join("\n"));
    }
    ExitCode::SUCCESS
}

fn walk(store: &Store, cairn: Cairn, limit: usize, at: usize, out: &mut Vec<String>) {
    if at >= limit {
        return;
    }
    let Ok(above) = store.referrers(cairn) else { return };
    let last = above.len().saturating_sub(1);
    for (i, one) in above.iter().enumerate() {
        let Ok(stored) = store.get(*one) else { continue };
        let what = match &stored {
            Stored::Value(v) => value(v),
            Stored::Node(n) => node(store, n),
        };
        let stem = "   ".repeat(at);
        let elbow = if i == last { "└─" } else { "├─" };
        out.push(format!("{stem}{elbow} {what}"));
        walk(store, *one, limit, at + 1, out);
    }
}
