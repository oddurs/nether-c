//! `nether graft` — substitute an answer and re-bury only what changed.
//!
//! `spec/08-rites.md` §8.7. Sound by [§6.5](../spec/06-evaluation.md)'s staging
//! law: a residue is a program, so burying it again with a different answer is
//! burying a program, and content addressing does the rest — everything whose
//! inputs did not change already has its name.

use std::collections::HashSet;
use std::fmt::Write as _;
use std::process::ExitCode;

use nether_bury::{Answers, bury_with};
use nether_core::{Depth, Prim};
use nether_ledger::{AnswerOf, Cairn, Call, Node, Span, Store, Stored, Value};
use nether_syntax::print;
use nether_world::Replay;

use crate::{DEFAULT_FUEL, FAILED, bad_usage, code, parse_fuel, split_flag, take_val, usage_error};

pub(crate) const USAGE: &str =
    "usage: nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]";

/// `nether graft <cairn> --replace <cairn> --with <cairn> [--fuel <n>] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut fuel: Option<u64> = None;
    let (mut trace, mut replace, mut with): (Option<&str>, Option<&str>, Option<&str>) =
        (None, None, None);
    let mut end_opts = false;

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if end_opts {
            if trace.is_some() {
                return bad_usage("one trace cairn at a time", USAGE);
            }
            trace = Some(arg.as_str());
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
            ("--replace", inline) => match take_val(inline, &mut rest) {
                Some(v) if !v.starts_with("--") && replace.is_none() => replace = Some(v),
                _ => {
                    eprintln!("{USAGE}");
                    return usage_error();
                }
            },
            ("--with", inline) => match take_val(inline, &mut rest) {
                Some(v) if !v.starts_with("--") && with.is_none() => with = Some(v),
                _ => {
                    eprintln!("{USAGE}");
                    return usage_error();
                }
            },
            (other, _) if other.starts_with('-') && other != "-" => {
                return bad_usage(&format!("unknown option `{arg}`"), USAGE);
            }
            _ if trace.is_some() => return bad_usage("one trace cairn at a time", USAGE),
            _ => trace = Some(arg.as_str()),
        }
    }

    let (Some(trace), Some(replace), Some(with)) = (trace, replace, with) else {
        eprintln!("{USAGE}");
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
        Err(code) => {
            if code == ExitCode::from(code::ABSENT) {
                eprintln!(
                    "        hint: `nether cairn <file>` hashes without storing; `--with` needs a value already in the ledger."
                );
            }
            return code;
        }
    };
    take(&store, GraftIds { trace, replace, with }, fuel.unwrap_or(DEFAULT_FUEL), wants_json)
}

#[derive(Clone, Copy)]
struct GraftIds {
    trace: Cairn,
    replace: Cairn,
    with: Cairn,
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
    let mismatch = || {
        eprintln!("nether: {} does not match the return type of `{function}`", with.short());
        FAILED
    };
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
                return Err(mismatch());
            }
            Ok(raw_instead.clone())
        }
        said => {
            if !prim_value_matches(prim, said) {
                return Err(mismatch());
            }
            Ok(if refusable {
                Value::Answer(Box::new(AnswerOf::Given(said.clone())))
            } else {
                said.clone()
            })
        }
    }
}

fn check_draw_length(
    store: &Store,
    call: &Call,
    with: Cairn,
    raw_instead: &Value,
) -> Result<(), ExitCode> {
    let Some(&len_cairn) = call.args.first() else {
        return Err(ExitCode::from(code::MALFORMED));
    };
    let Stored::Value(Value::Int(n)) =
        store.get(len_cairn).map_err(|e| crate::closing::store_err(&e))?
    else {
        eprintln!("nether: `draw` argument {} is not an integer", len_cairn.short());
        return Err(ExitCode::from(code::MALFORMED));
    };
    let Value::Bytes(bytes) = raw_instead else {
        return Err(FAILED);
    };
    let Ok(want) = usize::try_from(n) else {
        eprintln!("nether: `draw({n})` asks for an invalid length");
        return Err(FAILED);
    };
    if bytes.len() != want {
        eprintln!(
            "nether: {} is {} bytes, but `draw({want})` requires {want} bytes",
            with.short(),
            bytes.len()
        );
        return Err(FAILED);
    }
    Ok(())
}

fn prepare_graft(
    store: &Store,
    trace: Cairn,
    holes: &[Cairn],
    witnesses: Vec<Cairn>,
    replace: Cairn,
    with: Cairn,
) -> Result<PreparedGraft, ExitCode> {
    let told = Replay::of_trace(store, &witnesses).map_err(|e| crate::closing::store_err(&e))?;
    let Stored::Value(raw_instead) = store.get(with).map_err(|e| crate::closing::store_err(&e))?
    else {
        eprintln!("nether: {} is not a value in this ledger", with.short());
        return Err(FAILED);
    };
    if witnesses.contains(&replace) || told.all().any(|(_, said)| said.cairn() == replace) {
        eprintln!("nether: {} was already answered under {}", replace.short(), trace.short());
        eprintln!("        §8.7: a graft replaces a hole. Graft or exhume the trace that");
        eprintln!("        still has it.");
        return Err(FAILED);
    }

    // Validate every hole in `holes` (no early break) before selecting the
    // targeted hole so corrupt/trailing holes are never skipped.
    let mut earlier_calls: HashSet<Call> = HashSet::new();
    let mut matched: Option<(Call, u8, Span, Value)> = None;
    for hole in holes {
        let Stored::Node(Node::Hole { call, stratum, span }) =
            store.get(*hole).map_err(|e| crate::closing::store_err(&e))?
        else {
            eprintln!("nether: hole {} is not a hole node in this ledger", hole.short());
            return Err(ExitCode::from(code::MALFORMED));
        };
        if !crate::closing::valid_stratum(&call, stratum) {
            eprintln!(
                "nether: hole {} claims stratum {stratum} for `{}`",
                hole.short(),
                call.function
            );
            return Err(ExitCode::from(code::MALFORMED));
        }
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
                return Err(ExitCode::from(code::MALFORMED));
            };
            let val = wrap_graft_value(prim, &call.function, with, &raw_instead)?;
            if prim == Prim::Draw {
                check_draw_length(store, &call, with, &raw_instead)?;
            }
            if is_read && told.answer(&call).is_some_and(|prev| prev != &val) {
                eprintln!(
                    "nether: {} disagrees with an earlier read witness for `{}` under {}",
                    with.short(),
                    call.function,
                    trace.short()
                );
                return Err(FAILED);
            }
            matched = Some((call.clone(), stratum, span, val));
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

fn take(store: &Store, ids: GraftIds, fuel: u64, wants_json: bool) -> ExitCode {
    let Ok(Stored::Node(Node::Trace {
        residue,
        holes,
        witnesses,
        deposits,
        source,
        depth: orig_depth,
        unrecorded: marked,
    })) = store.get(ids.trace)
    else {
        return crate::not_a_trace(store, ids.trace);
    };

    if let Err(code) = crate::closing::verify_trace_parts(
        store, source, &deposits, &holes, &witnesses, orig_depth, marked,
    ) {
        return code;
    }

    let prep = match prepare_graft(store, ids.trace, &holes, witnesses, ids.replace, ids.with) {
        Ok(p) => p,
        Err(code) => return code,
    };

    let (text, unit) = match crate::closing::residue_unit(store, ids.trace, residue) {
        Ok(pair) => pair,
        Err(code) => return code,
    };
    let shown = format!("{}:residue", ids.trace.short());
    let residue = match bury_with(&unit, residue, fuel, &prep.answers) {
        Ok(residue) => residue,
        Err(halt) => {
            return crate::closing::halt_exit(&halt, &String::from_utf8_lossy(&text), &shown);
        }
    };
    if residue.served == 0 {
        eprintln!(
            "nether: hole {} was not reached by {}; refusing to record an unused witness",
            ids.replace.short(),
            ids.trace.short()
        );
        return FAILED;
    }
    commit_graft(store, ids, &unit, &residue, prep, (&deposits, source, marked), wants_json)
}

fn commit_graft(
    store: &Store,
    ids: GraftIds,
    unit: &nether_core::Unit,
    residue: &nether_bury::Residue,
    mut prep: PreparedGraft,
    (deposits, source, marked): (&[Cairn], Cairn, bool),
    wants_json: bool,
) -> ExitCode {
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

    let printed = print(&residue.as_unit(unit));
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
        deposits: crate::closing::deposits(deposits, &residue.deposits),
        source,
        depth,
        unrecorded,
    });
    let next = match record(&node) {
        Ok(c) => c,
        Err(code) => return code,
    };

    let told =
        Grafted { was: ids.trace, now: next, replaced: ids.replace, with: ids.with, reused, made };
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
