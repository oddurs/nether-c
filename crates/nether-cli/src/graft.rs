//! `nether graft` — substitute an answer and re-bury only what changed.
//!
//! `spec/08-rites.md` §8.7. Sound by [§6.5](../spec/06-evaluation.md)'s staging
//! law: a residue is a program, so burying it again with a different answer is
//! burying a program, and content addressing does the rest — everything whose
//! inputs did not change already has its name.

use std::fmt::Write as _;
use std::process::ExitCode;

use nether_bury::{Answers, bury_with};
use nether_core::check;
use nether_ledger::{Cairn, Node, Store, Stored, Value};
use nether_syntax::{lower, parse, print};
use nether_world::Replay;

use crate::{FAILED, code, ledger, usage_error};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// `nether graft <cairn> --replace <cairn> --with <cairn> [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let (mut trace, mut replace, mut with): (Option<&str>, Option<&str>, Option<&str>) =
        (None, None, None);

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => wants_json = true,
            "--replace" => replace = rest.next().map(|c| &**c),
            "--with" => with = rest.next().map(|c| &**c),
            other if other.starts_with("--") => {
                eprintln!("nether: unknown option {other}");
                return usage_error();
            }
            other => trace = Some(other),
        }
    }

    let (Some(trace), Some(replace), Some(with)) = (trace, replace, with) else {
        eprintln!("usage: nether graft <cairn> --replace <cairn> --with <cairn> [--json]");
        return usage_error();
    };

    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let name = |what: &str| match store.resolve(what) {
        Ok(c) => Ok(c),
        Err(e) => {
            eprintln!("nether: {e}");
            Err(ExitCode::from(code::ABSENT))
        }
    };
    let (Ok(trace), Ok(replace), Ok(with)) = (name(trace), name(replace), name(with)) else {
        return ExitCode::from(code::ABSENT);
    };
    take(&store, trace, replace, with, wants_json)
}

#[expect(clippy::too_many_lines, reason = "one pass, in the order §8.7 puts it")]
fn take(store: &Store, trace: Cairn, replace: Cairn, with: Cairn, wants_json: bool) -> ExitCode {
    let Ok(Stored::Node(Node::Trace { residue, holes, witnesses, source, .. })) = store.get(trace)
    else {
        eprintln!("nether: {} is not a trace", trace.short());
        return FAILED;
    };

    // What the trace was told, with one thing said differently. The answer
    // being replaced is named by its own cairn, because that is what a value
    // is: §7.2's whole argument is that a thing and its name are the same
    // question.
    let told = Replay::of_trace(store, &witnesses);
    let Ok(Stored::Value(instead)) = store.get(with) else {
        eprintln!("nether: {} is not a value in this ledger", with.short());
        return ExitCode::from(code::ABSENT);
    };
    // `--with` names what the world should have said, and what a witness holds
    // is what the world said — an `Answer`. A bare value is wrapped the way the
    // recorder would have wrapped it, because "pretend `read` returned these
    // bytes" is the thing anybody grafting is trying to say.
    let instead = match instead {
        Value::Answer(_) => instead,
        said => Value::Answer(Box::new(nether_ledger::AnswerOf::Given(said))),
    };
    let mut answers = Answers::none();
    for (call, said) in told.all() {
        answers = answers.and(call.clone(), said.clone());
    }

    // What is substituted is a subtrace, and the subtrace worth substituting is
    // a hole: a trace that has already been answered has the answer folded into
    // its residue, so replacing it there would change nothing. Answering a hole
    // differently is the operation a build tool wants — "pretend this file said
    // that" — and it is what makes the reuse count mean anything.
    let mut grafted = 0usize;
    for hole in &holes {
        let Ok(Stored::Node(Node::Hole { call, .. })) = store.get(*hole) else { continue };
        if *hole == replace {
            answers = answers.and(call, instead.clone());
            grafted += 1;
        }
    }
    // Or an answer the trace was already given, which is the other thing a
    // cairn here could name.
    for (call, said) in told.all() {
        if said.cairn() == replace {
            answers = answers.and(call.clone(), instead.clone());
            grafted += 1;
        }
    }
    // The trace has to name what answered it, and after a graft that is the
    // witness that recorded the substitute — §7.4's reverse index is how a
    // value finds the node that produced it.
    // The trace has to name what answered it, and after a graft that is
    // whatever recorded the substitute — §7.4's reverse index is how a value
    // finds the node that produced it. A substitute nothing witnessed means
    // nobody was ever told it, and then there is no witness to name.
    let recorded = witnessing(store, with);
    let mut witnesses: Vec<Cairn> = witnesses
        .iter()
        .map(|w| if answered_by(store, *w) == Some(replace) { recorded.unwrap_or(*w) } else { *w })
        .collect();
    if let Some(w) = recorded {
        if !witnesses.contains(&w) {
            witnesses.push(w);
        }
    }
    if grafted == 0 {
        eprintln!("nether: {} answers nothing under {}", replace.short(), trace.short());
        eprintln!("        `nether strata {}` says what it was told.", trace.short());
        return FAILED;
    }

    let Ok(Stored::Value(Value::Bytes(text))) = store.get(residue) else {
        eprintln!("nether: the residue of {} is not in this ledger", trace.short());
        return ExitCode::from(code::ABSENT);
    };
    let Ok(unit) = parse(&text).and_then(|ast| lower(&ast)) else {
        eprintln!("nether: the residue of {} does not lower", trace.short());
        return ExitCode::from(code::MALFORMED);
    };
    if !check(&unit).is_empty() {
        eprintln!("nether: the residue of {} does not check", trace.short());
        return ExitCode::from(code::MALFORMED);
    }

    let Ok(residue) = bury_with(&unit, source, DEFAULT_FUEL, &answers) else {
        eprintln!("nether: burying the graft did not finish");
        return FAILED;
    };

    // §8.7: report how many were reused and how many recomputed. The store
    // already knows — a name it holds is a thing nothing had to make again.
    let (mut reused, mut made) = (0usize, 0usize);
    for (_, stored) in &residue.named {
        // Asked before it is written, because writing is what makes the
        // difference disappear: a name the store already holds is a thing
        // nothing had to make again.
        if store.has(stored.cairn()) {
            reused += 1;
        } else {
            made += 1;
        }
        if let Err(e) = store.put(stored) {
            eprintln!("nether: {e}");
            return FAILED;
        }
    }

    let printed = print(&residue.as_unit(&unit));
    let next_residue = Stored::Value(Value::Bytes(printed.into_bytes()));
    if store.has(next_residue.cairn()) {
        reused += 1;
    } else {
        made += 1;
    }
    let Ok(next_residue) = store.put(&next_residue) else {
        eprintln!("nether: the ledger would not take the residue");
        return FAILED;
    };

    let node = Stored::Node(Node::Trace {
        residue: next_residue,
        holes: residue.holes.clone(),
        witnesses: witnesses.clone(),
        deposits: residue.deposits.clone(),
        source,
        depth: crate::closing::depth(store, residue.depth.get(), &witnesses),
        unrecorded: crate::closing::unrecorded(store, &witnesses),
    });
    if store.has(node.cairn()) {
        reused += 1;
    } else {
        made += 1;
    }
    let Ok(next) = store.put(&node) else {
        eprintln!("nether: the ledger would not take the trace");
        return FAILED;
    };

    let told = Grafted { was: trace, now: next, replaced: replace, with, reused, made };
    if wants_json {
        println!("{}", told.json());
    } else {
        print!("{}", told.text());
    }
    ExitCode::SUCCESS
}

/// What that witness recorded.
fn answered_by(store: &Store, witness: Cairn) -> Option<Cairn> {
    match store.get(witness) {
        Ok(Stored::Node(Node::Witness { answer, .. })) => Some(answer),
        _ => None,
    }
}

/// The witness that recorded that answer, read backwards. §7.4.
fn witnessing(store: &Store, answer: Cairn) -> Option<Cairn> {
    store
        .referrers(answer)
        .ok()?
        .into_iter()
        .find(|c| matches!(store.get(*c), Ok(Stored::Node(Node::Witness { .. }))))
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
