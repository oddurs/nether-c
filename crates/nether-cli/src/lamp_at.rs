//! `nether lamp --at <byte>` — inspect depth, origin, and trace provenance at a
//! source byte offset (`spec/08-rites.md` §8.4.1).

use std::collections::HashSet;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use nether_core::{
    Block, Capability, Depth, Expr, ExprKind, FuncDef, LocalId, Span, Stmt, Type, Unit, bound_to,
    children,
};
use nether_ledger::{Cairn, Node, Store, Stored, Value};

use crate::closing::{TraceParts, load_trace, store_err};
use crate::{FAILED, json, lamp, read_file_bytes, strata::stratum_name};

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

struct AtReport<'a> {
    cairn: Cairn,
    source: Cairn,
    byte: u64,
    hit: &'a Target<'a>,
    descend: Option<(Capability, Span)>,
    call: Option<(String, Span)>,
    nodes: &'a [Hit],
}

pub(crate) fn inspect_at(
    store: &Store,
    cairn: Cairn,
    byte: u64,
    source_path: Option<&str>,
    limit: usize,
    wants_json: bool,
) -> Result<(), ExitCode> {
    let trace = load_trace(store, cairn)?;
    if let Some(path) = source_path {
        let buf = read_file_bytes(Path::new(path))?;
        let got = Value::Bytes(buf).cairn();
        if got != trace.source {
            eprintln!(
                "nether: buffer {} does not match trace source {}; bury the buffer again",
                got.short(),
                trace.source.short()
            );
            return Err(FAILED);
        }
    }
    let Stored::Value(Value::Bytes(src)) = store.get(trace.source).map_err(|e| store_err(&e))?
    else {
        eprintln!("nether: source {} is not bytes", trace.source.short());
        return Err(ExitCode::from(crate::code::MALFORMED));
    };
    let Some(at) =
        u32::try_from(byte).ok().filter(|_| usize::try_from(byte).is_ok_and(|n| n < src.len()))
    else {
        eprintln!(
            "nether: byte {byte} is past the end of {} ({} bytes)",
            trace.source.short(),
            src.len()
        );
        return Err(FAILED);
    };
    let unit = crate::closing::checked_unit(&src, &trace.source.short())?;
    let Some(hit) = find_at(&unit, at) else {
        eprintln!("nether: no expression at byte {byte} in {}", trace.source.short());
        return Err(FAILED);
    };
    let descend = hit.descend.or_else(|| {
        find_origin(&unit, hit.func, hit.expr, hit.depth, &|x| match &x.kind {
            ExprKind::Descend { capability, body }
                if capability.stratum() == hit.depth || body.depth == hit.depth =>
            {
                Some((*capability, x.span))
            }
            _ => None,
        })
    });
    let call = find_origin(&unit, hit.func, hit.expr, hit.depth, &|x| {
        if let ExprKind::Call { callee, .. } = &x.kind
            && let Type::Fn { latent, result_depth, .. } = callee.ty
            && (latent.holds(hit.depth) || result_depth == hit.depth)
        {
            let name = match &callee.kind {
                ExprKind::Prim(p) => Some(p.name().to_string()),
                ExprKind::Func(id) => unit.func(*id).map(|f| f.name.clone()),
                _ => None,
            };
            return name.map(|n| (n, x.span));
        }
        None
    });
    let nodes = trace_hits(store, &trace, byte, call.as_ref().map(|(_, s)| *s), limit)?;
    let report =
        AtReport { cairn, source: trace.source, byte, hit: &hit, descend, call, nodes: &nodes };
    if wants_json {
        print_at_json(&report);
    } else {
        print_at_text(&report);
    }
    Ok(())
}

fn print_at_json(r: &AtReport<'_>) {
    let AtReport { cairn, source, byte, hit, descend, call, nodes } = r;
    let d = hit.depth.get();
    let s = stratum_name(d);
    let ty = hit.ty.to_string();
    let desc = descend.as_ref().map_or_else(
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
    let called = call.as_ref().map_or_else(
        || "null".to_string(),
        |(name, sp)| {
            format!("{{\"name\":{},\"start\":{},\"end\":{}}}", json::string(name), sp.start, sp.end)
        },
    );
    let each: Vec<String> = nodes
        .iter()
        .map(|h| {
            let prov: Vec<String> = h.lines.iter().map(|l| json::string(l)).collect();
            format!(
                "{{\"cairn\":\"{}\",\"start\":{},\"end\":{},\"provenance\":[{}]}}",
                h.cairn,
                h.span.start(),
                h.span.end(),
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
}

fn print_at_text(r: &AtReport<'_>) {
    let AtReport { hit, descend, call, nodes, .. } = r;
    let d = hit.depth.get();
    let mut out = format!(
        "depth {d}   {}   {}   at {}..{}\n",
        stratum_name(d),
        hit.ty,
        hit.span.start,
        hit.span.end
    );
    if let Some((cap, sp)) = descend {
        let _ = writeln!(out, "descend {cap}   at {}..{}", sp.start, sp.end);
    }
    if let Some((name, sp)) = call {
        let _ = writeln!(out, "call {name}   at {}..{}", sp.start, sp.end);
    }
    for h in *nodes {
        for line in &h.lines {
            let _ = writeln!(out, "{line}");
        }
    }
    print!("{out}");
}

fn trace_hits(
    store: &Store,
    trace: &TraceParts,
    byte: u64,
    call_span: Option<Span>,
    limit: usize,
) -> Result<Vec<Hit>, ExitCode> {
    let mut seen = HashSet::new();
    let mut all = Vec::new();
    for &at in trace.holes.iter().chain(&trace.witnesses).chain(&trace.deposits) {
        if !seen.insert(at) {
            continue;
        }
        let Stored::Node(n) = store.get(at).map_err(|e| store_err(&e))? else { continue };
        let (Node::Deposit { span, .. } | Node::Hole { span, .. } | Node::Witness { span, .. }) =
            &n
        else {
            continue;
        };
        all.push((at, n.clone(), *span));
    }
    let direct = |sp: &nether_ledger::Span| {
        sp.source() == trace.source && sp.start() <= byte && byte < sp.end()
    };
    let mut matched: Vec<(Cairn, Node, nether_ledger::Span)> =
        all.iter().filter(|(_, _, sp)| direct(sp)).cloned().collect();
    if matched.is_empty()
        && let Some(cs) = call_span
    {
        matched = all
            .into_iter()
            .filter(|(_, _, sp)| {
                sp.source() == trace.source
                    && sp.start() <= u64::from(cs.start)
                    && u64::from(cs.end) <= sp.end()
            })
            .collect();
    }
    let mut hits = Vec::new();
    for (at, n, span) in matched {
        // Verify referenced objects upfront so corrupt/absent arguments or
        // answers fail cleanly rather than falling back to short cairns in `node`.
        for r in n.references() {
            store.get(r).map_err(|e| store_err(&e))?;
        }
        let mut lines = vec![format!("{}  {}", at.short(), lamp::node(store, &n))];
        lamp::walk(store, at, limit, 0, &mut lines).map_err(|e| store_err(&e))?;
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

fn find_origin<T>(
    unit: &Unit,
    func: Option<&FuncDef>,
    x: &Expr,
    want: Depth,
    pred: &impl Fn(&Expr) -> Option<T>,
) -> Option<T> {
    fn walk<T>(
        unit: &Unit,
        func: Option<&FuncDef>,
        x: &Expr,
        seen: &mut HashSet<nether_core::GlobalId>,
        seen_locals: &mut HashSet<LocalId>,
        pred: &impl Fn(&Expr) -> Option<T>,
    ) -> Option<T> {
        if let Some(hit) = pred(x) {
            return Some(hit);
        }
        if let ExprKind::Local(id) = x.kind {
            if !seen_locals.insert(id) {
                return None;
            }
            return local_sources(unit, func, id)
                .into_iter()
                .find_map(|v| walk(unit, func, v, seen, seen_locals, pred));
        }
        if let ExprKind::Global(id) = x.kind {
            if !seen.insert(id) {
                return None;
            }
            let mut global_locals = HashSet::new();
            return unit
                .global(id)
                .and_then(|g| walk(unit, None, &g.value, seen, &mut global_locals, pred));
        }
        children(x).into_iter().find_map(|c| walk(unit, func, c, seen, seen_locals, pred))
    }
    if want == Depth::PURE {
        return None;
    }
    walk(unit, func, x, &mut HashSet::new(), &mut HashSet::new(), pred)
}

/// Collect every expression that can contribute depth to local `id`: its `Let`
/// initializer (if any) plus every `Assign` targeting `id` (both RHS and
/// `place.path` index expressions), ordered deepest-first so reassignments
/// deeper than a pure `Let` initializer are never hidden.
fn local_sources<'a>(unit: &'a Unit, func: Option<&'a FuncDef>, id: LocalId) -> Vec<&'a Expr> {
    let mut out = Vec::new();
    if let Some(f) = func {
        if let Some(init) = bound_to(&f.body, id) {
            out.push(init);
        }
        collect_assigns_in_block(&f.body, id, &mut out);
    } else {
        for e in unit.globals.iter().map(|g| &g.value).chain(unit.demands.iter().map(|d| &d.value))
        {
            if let Some(init) = bound_in_expr(e, id) {
                out.push(init);
            }
            collect_assigns_in_expr(e, id, &mut out);
        }
    }
    out.sort_by_key(|a| std::cmp::Reverse(a.depth));
    out
}

fn bound_in_expr(x: &Expr, id: LocalId) -> Option<&Expr> {
    if let ExprKind::Block(b) = &x.kind {
        return bound_to(b, id);
    }
    children(x).into_iter().find_map(|c| bound_in_expr(c, id))
}

fn collect_assigns_in_block<'a>(b: &'a Block, id: LocalId, out: &mut Vec<&'a Expr>) {
    for s in &b.stmts {
        match s {
            Stmt::Let { value, .. } | Stmt::Expr(value) => collect_assigns_in_expr(value, id, out),
            Stmt::Declare { .. } => {}
        }
    }
    if let Some(t) = &b.tail {
        collect_assigns_in_expr(t, id, out);
    }
}

fn collect_assigns_in_expr<'a>(x: &'a Expr, id: LocalId, out: &mut Vec<&'a Expr>) {
    if let ExprKind::Assign { place, value } = &x.kind
        && place.local == id
    {
        out.push(value);
        for proj in &place.path {
            if let nether_core::Proj::Index(idx) = proj {
                out.push(idx);
            }
        }
    }
    if let ExprKind::Block(b) = &x.kind {
        collect_assigns_in_block(b, id, out);
        return;
    }
    for c in children(x) {
        collect_assigns_in_expr(c, id, out);
    }
}
