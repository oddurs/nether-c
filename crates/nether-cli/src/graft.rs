//! `nether graft` — substitute an answer and re-bury only what changed.
//!
//! `spec/08-rites.md` §8.7. Sound by [§6.5](../spec/06-evaluation.md)'s staging
//! law: a residue is a program, so burying it again with a different answer is
//! burying a program, and content addressing does the rest — everything whose
//! inputs did not change already has its name.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::ExitCode;

use nether_bury::{Answers, HaltKind, bury_with};
use nether_core::{Depth, Prim};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Span, Store, Stored, Value};
use nether_syntax::print;
use nether_world::Replay;

use crate::{FAILED, code, usage_error};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// `nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut fuel = DEFAULT_FUEL;
    let (mut trace, mut replace, mut with): (Option<&str>, Option<&str>, Option<&str>) =
        (None, None, None);

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
            "--replace" => match rest.next() {
                Some(v) if !v.starts_with("--") && replace.is_none() => replace = Some(v),
                _ => {
                    eprintln!(
                        "usage: nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]"
                    );
                    return usage_error();
                }
            },
            "--with" => match rest.next() {
                Some(v) if !v.starts_with("--") && with.is_none() => with = Some(v),
                _ => {
                    eprintln!(
                        "usage: nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]"
                    );
                    return usage_error();
                }
            },
            other if other.starts_with("--") => {
                eprintln!("nether: unknown option {other}");
                return usage_error();
            }
            _ if trace.is_some() => {
                eprintln!("nether: one trace cairn at a time");
                return usage_error();
            }
            other => trace = Some(other),
        }
    }

    let (Some(trace), Some(replace), Some(with)) = (trace, replace, with) else {
        eprintln!(
            "usage: nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]"
        );
        return usage_error();
    };

    let store = match crate::opened() {
        Ok(store) => store,
        Err(code) => return code,
    };
    let trace = match crate::named(&store, trace) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let replace = match crate::named(&store, replace) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let with = match crate::named(&store, with) {
        Ok(c) => c,
        Err(code) => return code,
    };
    take(&store, trace, replace, with, fuel, wants_json)
}

struct PreparedGraft {
    answers: Answers,
    witnesses: Vec<Cairn>,
    call: Call,
    stratum: u8,
    span: Span,
    value: Value,
    is_read: bool,
}

fn prim_value_matches(p: Prim, v: &Value) -> bool {
    match p {
        Prim::Read | Prim::Get | Prim::Post | Prim::FetchNode | Prim::Draw | Prim::CallForeign => {
            matches!(v, Value::Bytes(_))
        }
        Prim::Env | Prim::Target => matches!(v, Value::Str(_)),
        Prim::Clock => matches!(v, Value::Int(_)),
        Prim::Exists | Prim::HasNode => matches!(v, Value::Bool(_)),
        Prim::Write | Prim::Remove => matches!(v, Value::Unit),
        Prim::List => {
            matches!(v, Value::Array(items) if items.iter().all(|x| matches!(x, Value::Str(_))))
        }
        _ => false,
    }
}

fn wrap_graft_value(
    prim: Prim,
    function: &str,
    with: Cairn,
    raw_instead: &Value,
) -> Result<Value, ExitCode> {
    let refusable = prim.refusable();
    match raw_instead {
        Value::Answer(_) if !refusable => {
            eprintln!("nether: {} is an Answer, but `{function}` is not refusable", with.short());
            Err(FAILED)
        }
        Value::Answer(ans) => {
            if let AnswerOf::Given(inner) = ans.as_ref()
                && !prim_value_matches(prim, inner)
            {
                eprintln!(
                    "nether: {} does not match the return type of `{function}`",
                    with.short()
                );
                return Err(FAILED);
            }
            Ok(raw_instead.clone())
        }
        said if refusable => {
            if !prim_value_matches(prim, said) {
                eprintln!(
                    "nether: {} does not match the return type of `{function}`",
                    with.short()
                );
                return Err(FAILED);
            }
            Ok(Value::Answer(Box::new(AnswerOf::Given(said.clone()))))
        }
        said => {
            if !prim_value_matches(prim, said) {
                eprintln!(
                    "nether: {} does not match the return type of `{function}`",
                    with.short()
                );
                return Err(FAILED);
            }
            Ok(said.clone())
        }
    }
}

fn prepare_graft(
    store: &Store,
    trace: Cairn,
    holes: &[Cairn],
    witnesses: Vec<Cairn>,
    replace: Cairn,
    with: Cairn,
) -> Result<PreparedGraft, ExitCode> {
    let told = match Replay::of_trace(store, &witnesses) {
        Ok(t) => t,
        Err(e) => {
            eprintln!("nether: {e}");
            return Err(FAILED);
        }
    };
    let Ok(Stored::Value(raw_instead)) = store.get(with) else {
        eprintln!("nether: {} is not a value in this ledger", with.short());
        return Err(ExitCode::from(code::ABSENT));
    };
    if told.all().any(|(_, said)| said.cairn() == replace) {
        eprintln!("nether: {} was already answered under {}", replace.short(), trace.short());
        eprintln!("        §8.7: a graft replaces a hole. Graft or exhume the trace that");
        eprintln!("        still has it.");
        return Err(FAILED);
    }

    // Only supply the targeted hole's replacement answer. Seeding `told.all()`
    // would feed stale prior witnesses into remaining open holes of the same
    // call (e.g. a second `draw` or `write`) instead of `--with`.
    let mut earlier_calls: HashSet<Call> = HashSet::new();
    let mut matched: Option<(Call, u8, Span, Value)> = None;
    for hole in holes {
        let Ok(Stored::Node(Node::Hole { call, stratum, span })) = store.get(*hole) else {
            eprintln!("nether: hole {} is not readable in this ledger", hole.short());
            return Err(ExitCode::from(code::ABSENT));
        };
        if *hole == replace && matched.is_none() {
            let is_read = Depth::new(stratum).is_some_and(Depth::reads);
            if !is_read && earlier_calls.contains(&call) {
                eprintln!(
                    "nether: {} is not the first open act hole for `{}` under {}",
                    replace.short(),
                    call.function,
                    trace.short()
                );
                return Err(FAILED);
            }
            let Some(prim) = Prim::from_name(&call.function) else {
                eprintln!(
                    "nether: unknown prelude call `{}` in hole {}",
                    call.function,
                    hole.short()
                );
                return Err(FAILED);
            };
            let val = wrap_graft_value(prim, &call.function, with, &raw_instead)?;
            matched = Some((call, stratum, span, val));
            break;
        }
        earlier_calls.insert(call);
    }
    let Some((call, stratum, span, value)) = matched else {
        eprintln!("nether: {} answers nothing under {}", replace.short(), trace.short());
        eprintln!("        `nether strata {}` says what it was told.", trace.short());
        return Err(FAILED);
    };
    let is_read = Depth::new(stratum).is_some_and(Depth::reads);
    let answers = Answers::none().and(call.clone(), value.clone());
    Ok(PreparedGraft { answers, witnesses, call, stratum, span, value, is_read })
}

fn take(
    store: &Store,
    trace: Cairn,
    replace: Cairn,
    with: Cairn,
    fuel: u64,
    wants_json: bool,
) -> ExitCode {
    let Ok(Stored::Node(Node::Trace {
        residue,
        holes,
        witnesses,
        deposits,
        source,
        unrecorded: marked,
        ..
    })) = store.get(trace)
    else {
        return crate::not_a_trace(store, trace);
    };

    let mut prep = match prepare_graft(store, trace, &holes, witnesses, replace, with) {
        Ok(p) => p,
        Err(code) => return code,
    };

    let (text, unit) = match crate::closing::residue_unit(store, trace, residue) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let shown = format!("{}:residue", trace.short());
    let residue = match bury_with(&unit, source, fuel, &prep.answers) {
        Ok(residue) => residue,
        Err(halt) => {
            eprint!(
                "{}",
                nether_core::report(&halt.diagnostic(), &String::from_utf8_lossy(&text), &shown)
            );
            return match halt.kind {
                HaltKind::OutOfFuel { .. } => ExitCode::from(code::FUEL),
                _ => FAILED,
            };
        }
    };

    let (mut reused, mut made) = (0usize, 0usize);
    let mut record = |stored: &Stored| -> Result<Cairn, ExitCode> {
        if store.has(stored.cairn()) {
            reused += 1;
        } else {
            made += 1;
        }
        crate::kept(store, stored)
    };
    let answer_cairn = match record(&Stored::Value(prep.value)) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let witness_cairn = match record(&Stored::Node(Node::Witness {
        call: prep.call,
        answer: answer_cairn,
        stratum: prep.stratum,
        span: prep.span,
    })) {
        Ok(c) => c,
        Err(code) => return code,
    };
    if !prep.is_read || !prep.witnesses.contains(&witness_cairn) {
        prep.witnesses.push(witness_cairn);
    }
    for (_, stored) in &residue.named {
        if let Err(code) = record(stored) {
            return code;
        }
    }

    let printed = print(&residue.as_unit(&unit));
    let next_residue = match record(&Stored::Value(Value::Bytes(printed.into_bytes()))) {
        Ok(c) => c,
        Err(code) => return code,
    };
    let (depth, unrecorded) = match (
        crate::closing::depth(store, residue.depth.get(), &residue.holes, &prep.witnesses),
        crate::closing::unrecorded(store, &prep.witnesses),
    ) {
        (Ok(d), Ok(u)) => (d, marked || u),
        (Err(c), _) | (_, Err(c)) => return c,
    };
    let node = Stored::Node(Node::Trace {
        residue: next_residue,
        holes: residue.holes.clone(),
        witnesses: prep.witnesses,
        deposits: crate::closing::deposits(&deposits, &residue.deposits),
        source,
        depth,
        unrecorded,
    });
    let next = match record(&node) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let told = Grafted { was: trace, now: next, replaced: replace, with, reused, made };
    if wants_json {
        println!("{}", told.json());
    } else {
        print!("{}", told.text());
    }
    ExitCode::SUCCESS
}

/// What §8.7 requires a graft to say.
struct Grafted {
    was: Cairn,
    now: Cairn,
    replaced: Cairn,
    with: Cairn,
    reused: usize,
    made: usize,
}

impl Grafted {
    fn text(&self) -> String {
        let mut out = String::new();
        let _ = writeln!(out, "  {} → {}", self.replaced.short(), self.with.short());
        let _ = writeln!(
            out,
            "grafted  {} → {}   {} reused   {} recomputed",
            self.was.short(),
            self.now.short(),
            self.reused,
            self.made
        );
        out
    }

    fn json(&self) -> String {
        format!(
            "{{\"was\":\"{}\",\"grafted\":\"{}\",\"replaced\":\"{}\",\"with\":\"{}\",\"reused\":{},\"recomputed\":{}}}",
            self.was, self.now, self.replaced, self.with, self.reused, self.made
        )
    }
}
