//! `nether lamp` — carry light down.
//!
//! `spec/08-rites.md` §8.4. A tool the operator carries, not a capability the
//! program holds: a program cannot invoke it, cannot reach a terminal, and has
//! no way to know whether anyone is looking. That is what makes it
//! structurally impossible for a Nether C program to leak to a log.

use std::collections::HashSet;
use std::process::ExitCode;

use nether_ledger::{AnswerOf, Cairn, Node, Refusal, Store, Stored, Value};

use crate::closing::store_err;
use crate::{FlagStep, bad_usage, finish, json, lamp_at, parse_args, take_val};

/// How far a provenance walk goes when nobody says, and the hard upper bound.
const DEFAULT_DEPTH: usize = 8;
const MAX_DEPTH: usize = 64;

pub(crate) const USAGE: &str = "usage: nether lamp <cairn> [--provenance | --at <byte> [--source <path>]] [--depth <n>] [--json]";

/// `nether lamp <cairn> [--provenance | --at <byte> [--source <path>]] [--depth <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    finish(run_inner(args))
}

fn run_inner(args: &[String]) -> Result<(), ExitCode> {
    let mut backwards = false;
    let mut at_byte: Option<u64> = None;
    let mut source_path: Option<&str> = None;
    let mut depth_opt: Option<usize> = None;
    let Some((name, wants_json)) =
        parse_args(args, USAGE, Some("one cairn at a time"), |flag, inline, rest| {
            match (flag, inline) {
                ("--provenance", None) => backwards = true,
                ("--depth", inline) => {
                    match take_val(inline, rest).and_then(|n| n.parse::<usize>().ok()) {
                        Some(_) if depth_opt.is_some() => {
                            return Err(complain("`--depth` is given once"));
                        }
                        Some(n) => depth_opt = Some(n.min(MAX_DEPTH)),
                        None => return Err(complain("`--depth` wants a number")),
                    }
                }
                ("--at", inline) => match take_val(inline, rest).and_then(|n| n.parse().ok()) {
                    Some(_) if at_byte.is_some() => return Err(complain("`--at` is given once")),
                    Some(n) => at_byte = Some(n),
                    None => return Err(complain("`--at` wants a byte offset")),
                },
                ("--source", inline) => match take_val(inline, rest) {
                    Some(_) if source_path.is_some() => {
                        return Err(complain("`--source` is given once"));
                    }
                    Some(p) if !p.starts_with("--") => source_path = Some(p),
                    _ => return Err(complain("`--source` wants a path")),
                },
                _ => return Ok(FlagStep::Unknown),
            }
            Ok(FlagStep::Handled)
        })?
    else {
        return Ok(());
    };
    if source_path.is_some() && at_byte.is_none() {
        return Err(complain("`--source` wants `--at`"));
    }
    if depth_opt.is_some() && !backwards && at_byte.is_none() {
        return Err(complain("`--depth` wants `--provenance` or `--at`"));
    }
    if backwards && at_byte.is_some() {
        return Err(complain("`--provenance` and `--at` are different walks"));
    }
    let depth = depth_opt.unwrap_or(DEFAULT_DEPTH);
    let Some(name) = name else {
        return Err(complain("which cairn?"));
    };

    let store = crate::opened_readonly();
    let cairn = crate::named(&store, name)?;
    if let Some(byte) = at_byte {
        lamp_at::inspect_at(&store, cairn, byte, source_path, depth, wants_json)
    } else if backwards {
        provenance(&store, cairn, depth, wants_json)
    } else {
        render(&store, cairn, wants_json)
    }
}

fn complain(what: &str) -> ExitCode {
    bad_usage(what, USAGE)
}

// ── forwards: what is there ─────────────────────────────────────────────────

fn render(store: &Store, cairn: Cairn, wants_json: bool) -> Result<(), ExitCode> {
    let stored = store.get(cairn).map_err(|e| store_err(&e))?;
    let text = match &stored {
        Stored::Value(v) => value(v),
        // A trace is read by what it left behind: every deposit under it, in
        // the order the program made them. §8.4.
        Stored::Node(Node::Trace { .. }) => {
            let found = deposits(store, cairn).map_err(|e| store_err(&e))?;
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
    Ok(())
}

/// Every `Deposit` a trace names, rendered, in the order they were made.
///
/// A deposit is not printed when it is made — nothing in Nether C is printed.
/// It is read afterwards, by somebody who decided to go and look.
///
/// §7.3.1: the list holds one entry per deposit, and a value deposited a
/// thousand times from one span is one node named a thousand times, so the
/// list is read and not the graph. §8.4: and it is read in its own order,
/// which is the order they were made in. Sorting it by source put a
/// hash in charge of which half of a program's output came first.
fn deposits(store: &Store, root: Cairn) -> Result<Vec<String>, nether_ledger::StoreError> {
    match store.get(root)? {
        Stored::Node(Node::Trace { deposits, .. }) => {
            deposits.iter().map(|at| deposited(store, *at)).collect()
        }
        Stored::Node(Node::Deposit { .. }) => Ok(vec![deposited(store, root)?]),
        _ => Ok(Vec::new()),
    }
}

/// One `Deposit`, rendered.
fn deposited(store: &Store, at: Cairn) -> Result<String, nether_ledger::StoreError> {
    let Stored::Node(Node::Deposit { value: held, .. }) = store.get(at)? else {
        return Err(nether_ledger::StoreError::Corrupt { asked: at, found: None, why: None });
    };
    let Stored::Value(v) = store.get(held)? else {
        return Err(nether_ledger::StoreError::Corrupt { asked: held, found: None, why: None });
    };
    Ok(value(&v))
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
pub(crate) fn node(store: &Store, n: &Node) -> String {
    match n {
        Node::Literal(v) => value(v),
        Node::Apply { function, args, result } => {
            let args: Vec<String> = args.iter().map(Cairn::short).collect();
            format!("apply    {}({})  →  {}", function.short(), args.join(", "), result.short())
        }
        Node::Hole { call, stratum, .. } => {
            format!("hole     {}   stratum {stratum}", said(store, call.function(), call.args()))
        }
        Node::Witness { call, stratum, answer, .. } => format!(
            "witness  {}   stratum {stratum}   →  {}",
            said(store, call.function(), call.args()),
            answer.short()
        ),
        Node::Deposit { value: v, span } => match store.get(*v) {
            Ok(Stored::Value(v)) => format!("deposit  {}   at {}", value(&v), span.start()),
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
        // §3.6: `#` and sixty-four hex digits, which is the literal a program
        // writes and the thing a reader can paste back.
        Value::Cairn(c) => format!("#{c}"),
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
fn provenance(store: &Store, cairn: Cairn, limit: usize, wants_json: bool) -> Result<(), ExitCode> {
    let head = match store.get(cairn).map_err(|e| store_err(&e))? {
        Stored::Value(v) => format!("{}  {}", cairn.short(), value(&v)),
        Stored::Node(n) => format!("{}  {}", cairn.short(), node(store, &n)),
    };
    let mut lines = vec![head];
    walk(store, cairn, limit, 0, &mut lines).map_err(|e| store_err(&e))?;
    if wants_json {
        let each: Vec<String> = lines.iter().map(|l| json::string(l)).collect();
        println!("{{\"cairn\":\"{cairn}\",\"provenance\":[{}]}}", each.join(","));
    } else {
        println!("{}", lines.join("\n"));
    }
    Ok(())
}

const MAX_PROVENANCE_LINES: usize = 10_000;

pub(crate) fn walk(
    store: &Store,
    cairn: Cairn,
    limit: usize,
    at: usize,
    out: &mut Vec<String>,
) -> Result<(), nether_ledger::StoreError> {
    let mut active = HashSet::new();
    active.insert(cairn);
    walk_acyclic(store, cairn, limit.min(MAX_DEPTH), at, &mut active, out)
}

fn walk_acyclic(
    store: &Store,
    cairn: Cairn,
    limit: usize,
    at: usize,
    active: &mut HashSet<Cairn>,
    out: &mut Vec<String>,
) -> Result<(), nether_ledger::StoreError> {
    if at >= limit || out.len() >= MAX_PROVENANCE_LINES {
        return Ok(());
    }
    let above = store.referrers(cairn)?;
    let last = above.len().saturating_sub(1);
    for (i, one) in above.iter().enumerate() {
        if out.len() >= MAX_PROVENANCE_LINES {
            break;
        }
        let stored = store.get(*one)?;
        let what = match &stored {
            Stored::Value(v) => value(v),
            Stored::Node(n) => node(store, n),
        };
        let stem = "   ".repeat(at);
        let elbow = if i == last { "└─" } else { "├─" };
        out.push(format!("{stem}{elbow} {what}"));
        if active.insert(*one) {
            walk_acyclic(store, *one, limit, at + 1, active, out)?;
            active.remove(one);
        }
    }
    Ok(())
}
