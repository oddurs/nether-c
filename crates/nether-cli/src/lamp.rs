//! `nether lamp` — carry light down.
//!
//! `spec/08-rites.md` §8.4. A tool the operator carries, not a capability the
//! program holds: a program cannot invoke it, cannot reach a terminal, and has
//! no way to know whether anyone is looking. That is what makes it
//! structurally impossible for a Nether C program to leak to a log.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::ExitCode;

use nether_core::{
    Block, Capability, Depth, Expr, ExprKind, FuncDef, LocalId, Span, Stmt, Type, Unit, bound_to,
    check, children,
};
use nether_ledger::{AnswerOf, Cairn, Node, Refusal, Store, Stored, Value};
use nether_syntax::{lower, parse};

use crate::{FAILED, code, json, strata::stratum_name, usage_error};

/// How far a provenance walk goes when nobody says.
const DEFAULT_DEPTH: usize = 8;

/// `nether lamp <cairn> [--provenance | --at <byte> [--source <path>]] [--depth <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut name = None;
    let mut backwards = false;
    let mut wants_json = false;
    let mut at_byte: Option<u64> = None;
    let mut source_path: Option<String> = None;
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
            "--at" => match rest.next().and_then(|n| n.parse().ok()) {
                Some(n) => at_byte = Some(n),
                None => return complain("`--at` wants a byte offset"),
            },
            "--source" => match rest.next() {
                Some(p) if !p.starts_with("--") => source_path = Some(p.clone()),
                _ => return complain("`--source` wants a path"),
            },
            other if other.starts_with("--") => {
                return complain("that is not a flag `lamp` has");
            }
            other if name.is_none() => name = Some(other.to_string()),
            _ => return complain("one cairn at a time"),
        }
    }
    if source_path.is_some() && at_byte.is_none() {
        return complain("`--source` wants `--at`");
    }
    if backwards && at_byte.is_some() {
        return complain("`--provenance` and `--at` are different walks");
    }
    let Some(name) = name else {
        return complain("which cairn?");
    };

    let store = match crate::opened() {
        Ok(store) => store,
        Err(code) => return code,
    };
    let cairn = match crate::named(&store, &name) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if let Some(byte) = at_byte {
        inspect_at(&store, cairn, byte, source_path.as_deref(), depth, wants_json)
    } else if backwards {
        provenance(&store, cairn, depth, wants_json)
    } else {
        render(&store, cairn, wants_json)
    }
}

fn complain(what: &str) -> ExitCode {
    eprintln!("nether: {what}");
    eprintln!(
        "usage: nether lamp <cairn> [--provenance | --at <byte> [--source <path>]] [--depth <n>] [--json]"
    );
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
fn deposits(store: &Store, root: Cairn) -> Vec<String> {
    match store.get(root) {
        Ok(Stored::Node(Node::Trace { deposits, .. })) => {
            deposits.iter().filter_map(|at| deposited(store, *at)).collect()
        }
        // A `Deposit` lamped on its own, rather than through a trace that
        // names it. There is no list to read, and it is one deposit.
        Ok(Stored::Node(Node::Deposit { .. })) => deposited(store, root).into_iter().collect(),
        _ => Vec::new(),
    }
}

/// One `Deposit`, rendered.
fn deposited(store: &Store, at: Cairn) -> Option<String> {
    let Ok(Stored::Node(Node::Deposit { value: held, .. })) = store.get(at) else {
        return None;
    };
    let Ok(Stored::Value(v)) = store.get(held) else { return None };
    Some(value(&v))
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

// ── at a byte offset: §8.4.1 ────────────────────────────────────────────────

struct Target<'a> {
    depth: Depth,
    ty: &'a Type,
    span: Span,
    expr: &'a Expr,
    func: Option<&'a FuncDef>,
    descend: Option<(Capability, Span)>,
}

struct Hit {
    cairn: Cairn,
    span: nether_ledger::Span,
    lines: Vec<String>,
}

fn inspect_at(
    store: &Store,
    cairn: Cairn,
    byte: u64,
    source_path: Option<&str>,
    limit: usize,
    wants_json: bool,
) -> ExitCode {
    let Ok(Stored::Node(trace @ Node::Trace { source, .. })) = store.get(cairn) else {
        return crate::not_a_trace(store, cairn);
    };
    if let Some(path) = source_path {
        let buf = match std::fs::read(path) {
            Ok(b) => b,
            Err(e) => {
                eprintln!("nether: {path}: {e}");
                return FAILED;
            }
        };
        let got = Value::Bytes(buf).cairn();
        if got != source {
            eprintln!(
                "nether: buffer {} does not match trace source {}; bury the buffer again",
                got.short(),
                source.short()
            );
            return FAILED;
        }
    }
    let src = match store.get(source) {
        Ok(Stored::Value(Value::Bytes(b))) => b,
        Ok(_) => {
            eprintln!("nether: source {} is not bytes", source.short());
            return FAILED;
        }
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let Some(at) =
        u32::try_from(byte).ok().filter(|_| usize::try_from(byte).is_ok_and(|n| n < src.len()))
    else {
        eprintln!(
            "nether: byte {byte} is past the end of {} ({} bytes)",
            source.short(),
            src.len()
        );
        return FAILED;
    };
    let Ok(unit) = parse(&src).and_then(|ast| lower(&ast)) else {
        return ExitCode::from(code::MALFORMED);
    };
    if !check(&unit).is_empty() {
        return ExitCode::from(code::MALFORMED);
    }
    let Some(hit) = find_at(&unit, at) else {
        eprintln!("nether: no expression at byte {byte} in {}", source.short());
        return FAILED;
    };
    let descend = hit.descend.or_else(|| find_descend(&unit, hit.func, hit.expr, hit.depth));
    let call = find_call(&unit, hit.func, hit.expr, hit.depth);
    let nodes = match trace_hits(store, &trace, source, byte, call.as_ref().map(|(_, s)| *s), limit)
    {
        Ok(n) => n,
        Err(code) => return code,
    };
    print_at(cairn, source, byte, &hit, descend, call, &nodes, wants_json);
    ExitCode::SUCCESS
}

#[expect(clippy::too_many_arguments, reason = "one report with all derived facts")]
fn print_at(
    cairn: Cairn,
    source: Cairn,
    byte: u64,
    hit: &Target<'_>,
    descend: Option<(Capability, Span)>,
    call: Option<(String, Span)>,
    nodes: &[Hit],
    wants_json: bool,
) {
    let d = hit.depth.get();
    let s = stratum_name(d);
    let ty = hit.ty.to_string();
    if wants_json {
        let desc = descend.map_or_else(
            || "null".to_string(),
            |(c, sp)| {
                format!(
                    "{{\"capability\":{},\"start\":{},\"end\":{}}}",
                    json::string(c.name()),
                    sp.start,
                    sp.end
                )
            },
        );
        let called = call.map_or_else(
            || "null".to_string(),
            |(name, sp)| {
                format!(
                    "{{\"name\":{},\"start\":{},\"end\":{}}}",
                    json::string(&name),
                    sp.start,
                    sp.end
                )
            },
        );
        let each: Vec<String> = nodes
            .iter()
            .map(|h| {
                let prov: Vec<String> = h.lines.iter().map(|l| json::string(l)).collect();
                format!(
                    "{{\"cairn\":\"{}\",\"start\":{},\"end\":{},\"provenance\":[{}]}}",
                    h.cairn,
                    h.span.start,
                    h.span.end,
                    prov.join(",")
                )
            })
            .collect();
        println!(
            "{{\"cairn\":\"{cairn}\",\"source\":\"{source}\",\"at\":{byte},\"depth\":{d},\
             \"stratum\":{},\"type\":{},\"span\":{{\"start\":{},\"end\":{}}},\
             \"descend\":{desc},\"call\":{called},\"nodes\":[{}]}}",
            json::string(s),
            json::string(&ty),
            hit.span.start,
            hit.span.end,
            each.join(",")
        );
        return;
    }
    let mut out = format!("depth {d}   {s}   {ty}   at {}..{}\n", hit.span.start, hit.span.end);
    if let Some((cap, sp)) = descend {
        let _ = writeln!(out, "descend {cap}   at {}..{}", sp.start, sp.end);
    }
    if let Some((name, sp)) = call {
        let _ = writeln!(out, "call {name}   at {}..{}", sp.start, sp.end);
    }
    for h in nodes {
        for line in &h.lines {
            let _ = writeln!(out, "{line}");
        }
    }
    print!("{out}");
}

fn trace_hits(
    store: &Store,
    trace: &Node,
    source: Cairn,
    byte: u64,
    call_span: Option<Span>,
    limit: usize,
) -> Result<Vec<Hit>, ExitCode> {
    let mut seen = HashSet::new();
    let mut all = Vec::new();
    for at in trace.nodes() {
        if !seen.insert(at) {
            continue;
        }
        let stored = store.get(at).map_err(|e| {
            eprintln!("nether: {e}");
            FAILED
        })?;
        let Stored::Node(n) = stored else { continue };
        let (Node::Deposit { span, .. } | Node::Hole { span, .. } | Node::Witness { span, .. }) =
            &n
        else {
            continue;
        };
        all.push((at, n.clone(), *span));
    }
    let direct =
        |sp: &nether_ledger::Span| sp.source == source && sp.start <= byte && byte < sp.end;
    let mut matched: Vec<(Cairn, Node, nether_ledger::Span)> =
        all.iter().filter(|(_, _, sp)| direct(sp)).cloned().collect();
    if matched.is_empty()
        && let Some(cs) = call_span
    {
        matched = all
            .into_iter()
            .filter(|(_, _, sp)| {
                sp.source == source
                    && sp.start <= u64::from(cs.start)
                    && u64::from(cs.end) <= sp.end
            })
            .collect();
    }
    let mut hits = Vec::new();
    for (at, n, span) in matched {
        for r in n.references() {
            store.get(r).map_err(|e| {
                eprintln!("nether: {e}");
                FAILED
            })?;
        }
        let mut lines = vec![format!("{}  {}", at.short(), node(store, &n))];
        walk(store, at, limit, 0, &mut lines);
        hits.push(Hit { cairn: at, span, lines });
    }
    Ok(hits)
}

const fn contains(s: Span, byte: u32) -> bool {
    s.start < s.end && s.start <= byte && byte < s.end
}

fn put<'a>(best: &mut Option<Target<'a>>, next: Target<'a>) {
    if let Some(cur) = best {
        if next.span.end - next.span.start <= cur.span.end - cur.span.start {
            *cur = next;
        }
    } else {
        *best = Some(next);
    }
}

fn find_at(unit: &Unit, byte: u32) -> Option<Target<'_>> {
    let mut best = None;
    for g in &unit.globals {
        if contains(g.span, byte) {
            put(
                &mut best,
                Target {
                    depth: g.value.depth,
                    ty: &g.ty,
                    span: g.span,
                    expr: &g.value,
                    func: None,
                    descend: None,
                },
            );
        }
        expr_at(&g.value, unit, None, None, byte, &mut best);
    }
    for f in &unit.funcs {
        block_at(&f.body, unit, Some(f), None, byte, &mut best);
    }
    for d in &unit.demands {
        expr_at(&d.value, unit, None, None, byte, &mut best);
    }
    best
}

fn block_at<'a>(
    b: &'a Block,
    unit: &'a Unit,
    func: Option<&'a FuncDef>,
    descend: Option<(Capability, Span)>,
    byte: u32,
    best: &mut Option<Target<'a>>,
) {
    for s in &b.stmts {
        match s {
            Stmt::Let { local, value } => {
                let def = func.map_or_else(|| unit.local(*local), |f| f.local(*local));
                if let Some(l) = def
                    && contains(l.span, byte)
                {
                    put(
                        best,
                        Target {
                            depth: value.depth,
                            ty: &l.ty,
                            span: l.span,
                            expr: value,
                            func,
                            descend,
                        },
                    );
                }
                expr_at(value, unit, func, descend, byte, best);
            }
            Stmt::Expr(x) => expr_at(x, unit, func, descend, byte, best),
            Stmt::Declare { .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        expr_at(t, unit, func, descend, byte, best);
    }
}

fn expr_at<'a>(
    x: &'a Expr,
    unit: &'a Unit,
    func: Option<&'a FuncDef>,
    descend: Option<(Capability, Span)>,
    byte: u32,
    best: &mut Option<Target<'a>>,
) {
    let descend = match &x.kind {
        ExprKind::Descend { capability, .. } if contains(x.span, byte) => {
            Some((*capability, x.span))
        }
        _ => descend,
    };
    if contains(x.span, byte) {
        put(best, Target { depth: x.depth, ty: &x.ty, span: x.span, expr: x, func, descend });
    }
    match &x.kind {
        ExprKind::Block(b) => block_at(b, unit, func, descend, byte, best),
        ExprKind::Call { callee, args } => {
            if !matches!(callee.kind, ExprKind::Prim(_) | ExprKind::Func(_)) {
                expr_at(callee, unit, func, descend, byte, best);
            }
            for a in args {
                expr_at(a, unit, func, descend, byte, best);
            }
        }
        _ => {
            for c in children(x) {
                expr_at(c, unit, func, descend, byte, best);
            }
        }
    }
}

fn find_descend(
    unit: &Unit,
    func: Option<&FuncDef>,
    x: &Expr,
    want: Depth,
) -> Option<(Capability, Span)> {
    if want == Depth::PURE {
        return None;
    }
    if let ExprKind::Descend { capability, body } = &x.kind
        && (capability.stratum() == want || body.depth == want)
    {
        return Some((*capability, x.span));
    }
    if let ExprKind::Local(id) = x.kind {
        return bound_local(unit, func, id).and_then(|v| find_descend(unit, func, v, want));
    }
    if let ExprKind::Global(id) = x.kind {
        return unit.global(id).and_then(|g| find_descend(unit, None, &g.value, want));
    }
    children(x).into_iter().find_map(|c| find_descend(unit, func, c, want))
}

fn find_call(unit: &Unit, func: Option<&FuncDef>, x: &Expr, want: Depth) -> Option<(String, Span)> {
    if want == Depth::PURE {
        return None;
    }
    if let ExprKind::Call { callee, .. } = &x.kind
        && let Type::Fn { latent, result_depth, .. } = callee.ty
        && (latent.holds(want) || result_depth == want)
    {
        let name = match &callee.kind {
            ExprKind::Prim(p) => Some(p.name().to_string()),
            ExprKind::Func(id) => unit.func(*id).map(|f| f.name.clone()),
            _ => None,
        };
        if let Some(name) = name {
            return Some((name, x.span));
        }
    }
    if let ExprKind::Local(id) = x.kind {
        return bound_local(unit, func, id).and_then(|v| find_call(unit, func, v, want));
    }
    if let ExprKind::Global(id) = x.kind {
        return unit.global(id).and_then(|g| find_call(unit, None, &g.value, want));
    }
    children(x).into_iter().find_map(|c| find_call(unit, func, c, want))
}

fn bound_local<'a>(unit: &'a Unit, func: Option<&'a FuncDef>, id: LocalId) -> Option<&'a Expr> {
    if let Some(f) = func {
        return bound_to(&f.body, id);
    }
    unit.globals
        .iter()
        .map(|g| &g.value)
        .chain(unit.demands.iter().map(|d| &d.value))
        .find_map(|e| bound_in_expr(e, id))
}

fn bound_in_expr(x: &Expr, id: LocalId) -> Option<&Expr> {
    if let ExprKind::Block(b) = &x.kind {
        return bound_to(b, id);
    }
    children(x).into_iter().find_map(|c| bound_in_expr(c, id))
}
