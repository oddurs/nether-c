//! `nether exhume` — grant a stratum, answer holes, bury what is left.
//!
//! `spec/08-rites.md` §8.3, and §6.6. Never revises the trace it was given:
//! answering produces a new trace with a new cairn, and the old one still
//! exists with its hole in it.

use std::fmt::Write as _;
use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nether_bury::{Answers, HaltKind, bury_with};
use nether_core::{Capability, check};
use nether_ledger::{Cairn, Call, Node, Store, Stored, Value};
use nether_syntax::{lower, parse, print};
use nether_world::{Disk, Recorder, World};

use crate::{FAILED, code, json, ledger, usage_error};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// `nether exhume <cairn> [--grant <cap>]... [--replay] [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut replaying = false;
    let mut granted: Vec<Capability> = Vec::new();
    let mut name: Option<&str> = None;

    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        match arg.as_str() {
            "--json" => wants_json = true,
            "--replay" => replaying = true,
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
            other => name = Some(other),
        }
    }

    let Some(name) = name else {
        eprintln!("usage: nether exhume <cairn> [--grant <cap>]... [--replay] [--json]");
        return usage_error();
    };
    // §8.3. Not a preference: replay that can be handed a grant is replay that
    // can reach the world, and then the flag is a lie.
    if replaying && !granted.is_empty() {
        eprintln!("nether: `--replay` and `--grant` are mutually exclusive.");
        eprintln!();
        eprintln!("  §6.7: replay does not prefer the ledger over the world, it cannot");
        eprintln!("  reach the world. A replay holding a grant would be neither.");
        return usage_error();
    }

    // §6.7's replay law is unsatisfiable while a trace records how many steps
    // it cost: replaying re-buries an already-folded residue, which is a
    // cheaper burial, and every other field comes back the same. Measured in
    // 0163, which takes the field out.
    if replaying {
        eprintln!("nether: `--replay` cannot reproduce a trace yet.");
        eprintln!();
        eprintln!("  A trace records `fuel_spent`, which is what the burial that made it");
        eprintln!("  cost. Replaying buries an already-folded residue, which costs less, so");
        eprintln!("  the trace that comes back differs in that field and no other. §6.7");
        eprintln!("  wants byte-identical. 0163.");
        return ExitCode::from(code::UNIMPLEMENTED);
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
    dig_up(&store, cairn, &granted, wants_json)
}

/// Every hole a trace names, with the question it asks.
fn questions(store: &Store, holes: &[Cairn]) -> Vec<(Cairn, Call, u8)> {
    holes
        .iter()
        .filter_map(|h| match store.get(*h) {
            Ok(Stored::Node(Node::Hole { call, stratum, .. })) => Some((*h, call, stratum)),
            _ => None,
        })
        .collect()
}

/// The world a grant asks for, or the reason there is not one yet.
fn world_from(granted: &[Capability], root: &Path) -> Result<World, Capability> {
    let mut world = World::sealed();
    for cap in granted {
        match cap {
            Capability::Disk => world = world.granting(Box::new(Disk::reading(root))),
            Capability::DiskWrite => world = world.granting(Box::new(Disk::writing(root))),
            // §09 has the rest and this build does not. Saying which is
            // better than answering nothing and calling it a refusal.
            other => return Err(*other),
        }
    }
    Ok(world)
}

#[expect(clippy::too_many_lines, reason = "one pass, in the order §6.6 puts it")]
fn dig_up(store: &Store, cairn: Cairn, granted: &[Capability], wants_json: bool) -> ExitCode {
    let Ok(Stored::Node(Node::Trace { residue, holes, witnesses, source, .. })) = store.get(cairn)
    else {
        eprintln!("nether: {} is not a trace", cairn.short());
        return FAILED;
    };

    // The residue is source (§6.5). Burying it again means lexing, parsing and
    // lowering it again, which is exactly what burying anything means.
    let Ok(Stored::Value(Value::Bytes(text))) = store.get(residue) else {
        eprintln!("nether: the residue of {} is not in this ledger", cairn.short());
        return ExitCode::from(code::ABSENT);
    };
    let shown = format!("{}:residue", cairn.short());
    let unit = match parse(&text)
        .map_err(|f| f[0].diagnostic())
        .and_then(|ast| lower(&ast).map_err(|f| f[0].diagnostic()))
    {
        Ok(ast) => ast,
        Err(d) => {
            eprint!("{}", nether_core::report(&d, &String::from_utf8_lossy(&text), &shown));
            return ExitCode::from(code::MALFORMED);
        }
    };
    if let Some(fault) = check(&unit).first() {
        eprint!(
            "{}",
            nether_core::report(&fault.diagnostic(), &String::from_utf8_lossy(&text), &shown)
        );
        return ExitCode::from(code::MALFORMED);
    }

    // Answer what can be answered, and write every answer down first.
    let mut answers = Answers::none();
    let mut said: Vec<(Call, Cairn)> = Vec::new();
    let mut recorded: Vec<Cairn> = witnesses.clone();

    let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    let world = match world_from(granted, &root) {
        Ok(world) => world,
        Err(cap) => {
            eprintln!("nether: `{cap}` is in §9.1 and is not built yet.");
            eprintln!("        disk and disk! are; the rest are 0068, 0070, 0071 and 0072.");
            return ExitCode::from(code::UNIMPLEMENTED);
        }
    };
    let into = Recorder::new(store, source);
    for (hole, call, _) in questions(store, &holes) {
        let Ok(Stored::Node(Node::Hole { span, .. })) = store.get(hole) else { continue };
        match world.ask(&call, span, &into) {
            // A hole nothing granted answers stays a hole, which is what
            // makes exhumation incremental rather than all or nothing.
            Err(nether_world::Unanswered::Ungranted { .. }) => {}
            Err(e) => {
                eprintln!("nether: {e}");
                return FAILED;
            }
            Ok(answer) => {
                let Ok(Stored::Value(value)) = store.get(answer.answer()) else { continue };
                said.push((call.clone(), answer.answer()));
                recorded.push(answer.witness());
                answers = answers.and(call, value);
            }
        }
    }

    let residue = match bury_with(&unit, source, DEFAULT_FUEL, &answers) {
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

    for (_, stored) in &residue.named {
        if let Err(e) = store.put(stored) {
            eprintln!("nether: {e}");
            return FAILED;
        }
    }
    let printed = print(&residue.as_unit(&unit));
    let next_residue = match store.put(&Stored::Value(Value::Bytes(printed.into_bytes()))) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    // §7.3.2: the join of what the residue still reaches and what a witness
    // reached. A sealed trace has no holes left, so the second half is the
    // whole of it — which is why §6.6 reads `depth 3   holes 0` over a program
    // that has finished.
    let deepest = recorded.iter().map(|w| reached(store, *w)).fold(residue.depth.get(), u8::max);
    let deeper = Node::Trace {
        residue: next_residue,
        holes: residue.holes.clone(),
        // §1.7: marked when something *reached* stratum 8, not when a hole
        // would.
        unrecorded: recorded.iter().any(|w| reached(store, *w) == 8),
        witnesses: recorded,
        deposits: residue.deposits.clone(),
        source,
        fuel_spent: residue.fuel_spent,
        depth: deepest,
    };
    let next = match store.put(&Stored::Node(deeper)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    let told = Sealed { was: cairn, now: next, said, depth: deepest, holes: residue.holes.len() };
    if wants_json {
        println!("{}", told.json());
    } else {
        print!("{}", told.text(store));
    }
    ExitCode::SUCCESS
}

/// The stratum a witness reached, if it is one.
fn reached(store: &Store, witness: Cairn) -> u8 {
    match store.get(witness) {
        Ok(Stored::Node(Node::Witness { stratum, .. })) => stratum,
        _ => 0,
    }
}

/// The summary §6.6 prints.
struct Sealed {
    was: Cairn,
    now: Cairn,
    said: Vec<(Call, Cairn)>,
    depth: u8,
    holes: usize,
}

impl Sealed {
    fn text(&self, store: &Store) -> String {
        let mut out = String::new();
        for (n, (call, answer)) in self.said.iter().enumerate() {
            let asked = crate::lamp::said(store, &call.function, &call.args);
            let size = match store.get(*answer) {
                Ok(Stored::Value(v)) => crate::bury::measure(&v),
                _ => String::new(),
            };
            let numeral = crate::bury::circled(n + 1);
            let _ = writeln!(out, "  {numeral}  {asked}  →  {size}  {}", answer.short());
        }
        let answered: Vec<String> = self.said.iter().map(|(_, a)| a.short()).collect();
        let _ = writeln!(
            out,
            "sealed   {} + {} → {}   depth {}   holes {}",
            self.was.short(),
            answered.join(" + "),
            self.now.short(),
            self.depth,
            self.holes
        );
        out
    }

    fn json(&self) -> String {
        let said: Vec<String> = self
            .said
            .iter()
            .map(|(call, answer)| {
                format!("{{\"call\":{},\"answer\":\"{answer}\"}}", json::string(&call.function))
            })
            .collect();
        format!(
            "{{\"was\":\"{}\",\"sealed\":\"{}\",\"depth\":{},\"holes\":{},\"answered\":[{}]}}",
            self.was,
            self.now,
            self.depth,
            self.holes,
            said.join(",")
        )
    }
}
