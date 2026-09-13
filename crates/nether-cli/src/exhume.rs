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
use nether_ledger::{Cairn, Call, Node, Span, Store, Stored, Value};
use nether_syntax::{lower, parse, print};
use nether_world::{Declared, Disk, Env, Ledger, Net, Recorder, Replay, World};

use crate::{FAILED, code, json, ledger, usage_error};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// What one invocation asked for. §8.3, and §8.3.1 for the declaration.
#[derive(Default)]
struct Asked<'a> {
    name: Option<&'a str>,
    granted: Vec<Capability>,
    declared: Declared,
    reach: Vec<String>,
    clock: Option<i64>,
    target: Option<&'a str>,
    replaying: bool,
    wants_json: bool,
}

const USAGE: &str = "usage: nether exhume <cairn> [--grant <cap>]... \
                     [--declare <name>=<value>]...\n       [--clock <seconds>] \
                     [--target <triple>] [--replay] [--json]";

fn wrong(what: &str) -> ExitCode {
    eprintln!("nether: {what}");
    eprintln!("{USAGE}");
    usage_error()
}

impl<'a> Asked<'a> {
    /// Read the flags, or say which one was wrong.
    fn parse(args: &'a [String]) -> Result<Self, ExitCode> {
        let mut it = Self::default();
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            match arg.as_str() {
                "--json" => it.wants_json = true,
                "--replay" => it.replaying = true,
                "--grant" => it.granted.push(grant(rest.next())?),
                "--declare" => it.declared = declare(it.declared, rest.next())?,
                "--clock" => match rest.next().map(|n| n.parse()) {
                    Some(Ok(n)) => it.clock = Some(n),
                    _ => return Err(wrong("`--clock` wants whole seconds since the epoch")),
                },
                "--target" => match rest.next() {
                    Some(t) => it.target = Some(t),
                    None => return Err(wrong("`--target` wants a triple")),
                },
                "--reach" => match rest.next() {
                    Some(h) if !h.is_empty() => it.reach.push(h.clone()),
                    _ => return Err(wrong("`--reach` wants a host, and optionally a port")),
                },
                other if other.starts_with("--") => {
                    return Err(wrong(&format!("unknown option {other}")));
                }
                // `lamp` refuses this too. Taking two and using one is the
                // same as being handed a cairn and exhuming a different one.
                _ if it.name.is_some() => return Err(wrong("one cairn at a time")),
                other => it.name = Some(other),
            }
        }
        it.settled()
    }

    /// The invocations §8.3 and §8.3.1 refuse outright.
    fn settled(self) -> Result<Self, ExitCode> {
        if self.name.is_none() {
            return Err(wrong("which cairn?"));
        }
        // Not a preference: replay that can be handed a grant is replay that
        // can reach the world, and then the flag is a lie. §6.7.
        if self.replaying && !self.granted.is_empty() {
            eprintln!("nether: `--replay` and `--grant` are mutually exclusive.");
            eprintln!();
            eprintln!("  §6.7: replay does not prefer the ledger over the world, it cannot");
            eprintln!("  reach the world. A replay holding a grant would be neither.");
            return Err(usage_error());
        }
        // §8.3.2, on §8.3.1's reasoning: a reach nothing can use could not be
        // read, and accepting it would be §8.0's failure in a smaller place.
        let net = self.granted.iter().any(|c| matches!(c, Capability::Net | Capability::NetWrite));
        if !net && !self.reach.is_empty() {
            eprintln!("nether: a reach without `--grant net` could not be used.");
            eprintln!();
            eprintln!("  §8.3.2: `--grant net` says which stratum, `--reach` says how far.");
            return Err(usage_error());
        }
        let env = self.granted.contains(&Capability::Env);
        if env && (self.clock.is_none() || self.target.is_none()) {
            eprintln!("nether: granting `env` means pinning it: `--clock` and `--target`.");
            eprintln!();
            eprintln!("  §8.3.1: there is no default, because a default would be this");
            eprintln!("  machine's, and a rite whose answer depends on which machine ran");
            eprintln!("  it is what §6.7 exists to prevent.");
            return Err(usage_error());
        }
        if !env
            && (self.clock.is_some() || self.target.is_some() || self.declared != Declared::none())
        {
            eprintln!("nether: a declaration without `--grant env` could not be read.");
            eprintln!();
            eprintln!("  §8.3.1, and the same reasoning as §8.0: a flag that validates");
            eprintln!("  its argument and changes nothing looks like it worked.");
            return Err(usage_error());
        }
        Ok(self)
    }
}

/// One `--grant`, by the names §9.1 fixes.
fn grant(arg: Option<&String>) -> Result<Capability, ExitCode> {
    match arg.map(|c| Capability::from_name(c)) {
        Some(Some(cap)) => Ok(cap),
        Some(None) => {
            eprintln!("nether: no capability by that name");
            eprintln!("        §9.1 lists them: store env disk disk! net net! entropy unrecorded");
            Err(usage_error())
        }
        None => Err(wrong("`--grant` wants a capability")),
    }
}

/// One `--declare name=value`. §8.3.1: an empty value declares and unsets.
fn declare(so_far: Declared, arg: Option<&String>) -> Result<Declared, ExitCode> {
    let Some((name, value)) = arg.and_then(|d| d.split_once('=')) else {
        return Err(wrong("`--declare` wants <name>=<value>"));
    };
    if name.is_empty() {
        return Err(wrong("`--declare` wants a name before the `=`"));
    }
    so_far.and(name, value).map_err(|twice| {
        eprintln!("nether: {twice}");
        eprintln!("        §8.3.1: the two invocations differ and only one can be meant.");
        usage_error()
    })
}

/// `nether exhume <cairn> [--grant <cap>]... [--declare <name>=<value>]...`
pub fn run(args: &[String]) -> ExitCode {
    let asked = match Asked::parse(args) {
        Ok(asked) => asked,
        Err(code) => return code,
    };
    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let cairn = match store.resolve(asked.name.unwrap_or_default()) {
        Ok(cairn) => cairn,
        Err(e) => {
            eprintln!("nether: {e}");
            return ExitCode::from(code::ABSENT);
        }
    };
    dig_up(&store, cairn, &asked)
}

/// Every hole a trace names, with the question it asks and where it was asked.
///
/// `Err` is the cairn of a hole this ledger cannot read. Dropping one instead
/// left it a hole in the new trace, so a trace half of whose holes were missing
/// exhumed to a trace with fewer holes and no complaint.
fn questions(store: &Store, holes: &[Cairn]) -> Result<Vec<(Call, u8, Span)>, Cairn> {
    holes
        .iter()
        .map(|h| match store.get(*h) {
            Ok(Stored::Node(Node::Hole { call, stratum, span })) => Ok((call, stratum, span)),
            _ => Err(*h),
        })
        .collect()
}

/// What to say about a hole named by a trace and absent from the ledger.
fn no_such_hole(hole: Cairn) -> ExitCode {
    eprintln!("nether: this trace names a hole the ledger does not hold");
    eprintln!("        {hole}");
    eprintln!("        Exhuming it would answer less than the trace asks and");
    eprintln!("        report that it had answered everything.");
    ExitCode::from(code::ABSENT)
}

/// The world a grant asks for, or the reason there is not one yet.
fn world_from(asked: &Asked, root: &Path) -> Result<World, Capability> {
    let mut world = World::sealed();
    for cap in &asked.granted {
        let provider: Box<dyn nether_world::Provider> = match cap {
            Capability::Store => Box::new(Ledger),
            // §8.3.1 makes both required when `env` is granted, and `settled`
            // has already refused the invocation that leaves either out.
            Capability::Env => Box::new(Env::pinned(
                asked.declared.clone(),
                asked.clock.unwrap_or_default(),
                asked.target.unwrap_or_default(),
            )),
            // §8.3.2 is the whole of where it may reach; granting `net` and
            // reaching nothing is a network with nothing in it.
            Capability::Net => Box::new(Net::fetching(asked.reach.clone())),
            Capability::NetWrite => Box::new(Net::sending(asked.reach.clone())),
            Capability::Disk => Box::new(Disk::reading(root)),
            Capability::DiskWrite => Box::new(Disk::writing(root)),
            // §09 has the rest and this build does not. Saying which is
            // better than answering nothing and calling it a refusal.
            other => return Err(*other),
        };
        world = world.granting(provider);
    }
    Ok(world)
}

#[expect(clippy::too_many_lines, reason = "one pass, in the order §6.6 puts it")]
fn dig_up(store: &Store, cairn: Cairn, asked: &Asked) -> ExitCode {
    let (replaying, wants_json) = (asked.replaying, asked.wants_json);
    let Ok(Stored::Node(Node::Trace { residue, holes, witnesses, deposits, source, .. })) =
        store.get(cairn)
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

    if replaying {
        // §6.7: only the ledger. There is no `World` in this branch to reach
        // the world with — which is the difference between preferring the
        // ledger and being unable to leave it.
        let have = Replay::of_trace(store, &witnesses);
        let asked = match questions(store, &holes) {
            Ok(asked) => asked,
            Err(hole) => return no_such_hole(hole),
        };
        for (call, _, _) in &asked {
            if have.answer(call).is_none() {
                eprintln!("nether: nothing recorded answers `{}`", call.function);
                eprintln!("        §6.7: replay serves the ledger and cannot reach the world.");
                return FAILED;
            }
        }
        for (call, value) in have.all() {
            answers = answers.and(call.clone(), value.clone());
        }
    } else {
        let root = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
        let world = match world_from(asked, &root) {
            Ok(world) => world,
            Err(cap) => {
                eprintln!("nether: `{cap}` is in §9.1 and is not built yet.");
                eprintln!(
                    "        store env disk disk! net net! are; entropy is 0071 and unrecorded 0072."
                );
                return ExitCode::from(code::UNIMPLEMENTED);
            }
        };
        let into = Recorder::new(store);
        let asked = match questions(store, &holes) {
            Ok(asked) => asked,
            Err(hole) => return no_such_hole(hole),
        };
        for (call, _, span) in asked {
            match world.ask(&call, span, &into) {
                // A hole nothing granted answers stays a hole, which is what
                // makes exhumation incremental rather than all or nothing.
                Err(nether_world::Unanswered::Ungranted { .. }) => {}
                // §9.9: not catchable, and not the world saying no. The rite
                // reports where it happened and stops, as a burial does.
                Err(e @ nether_world::Unanswered::Collapsed(_)) => {
                    eprintln!("nether: {e}");
                    eprintln!("        §9.9: a collapse is a bug in the program, not an answer.");
                    return FAILED;
                }
                Err(e) => {
                    eprintln!("nether: {e}");
                    return FAILED;
                }
                Ok(answer) => {
                    let Ok(Stored::Value(value)) = store.get(answer.answer()) else {
                        eprintln!("nether: the answer to `{}` is not readable back", call.function);
                        return FAILED;
                    };
                    said.push((call.clone(), answer.answer()));
                    recorded.push(answer.witness());
                    answers = answers.and(call, value);
                }
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
    let deepest = crate::closing::depth(store, residue.depth.get(), &recorded);
    let deeper = Node::Trace {
        residue: next_residue,
        holes: residue.holes.clone(),
        unrecorded: crate::closing::unrecorded(store, &recorded),
        deposits: crate::closing::deposits(&deposits, &residue.deposits),
        witnesses: recorded,
        source,
        depth: deepest,
    };
    let next = match store.put(&Stored::Node(deeper)) {
        Ok(c) => c,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };

    // §6.7's replay law: replaying a sealed trace produces the same cairn.
    if replaying {
        if next == cairn {
            println!("identical.");
            return ExitCode::SUCCESS;
        }
        eprintln!("nether: replay produced {} and not {}", next.short(), cairn.short());
        eprintln!("        §6.7 calls that a bug in the implementation.");
        return ExitCode::from(code::MALFORMED);
    }

    let told = Sealed {
        was: cairn,
        now: next,
        said,
        depth: deepest,
        holes: residue.holes.clone(),
        spent: residue.fuel_spent,
    };
    if wants_json {
        println!("{}", told.json());
    } else {
        print!("{}", told.text(store));
    }
    ExitCode::SUCCESS
}

/// The summary §6.6 prints.
struct Sealed {
    was: Cairn,
    now: Cairn,
    said: Vec<(Call, Cairn)>,
    depth: u8,
    holes: Vec<Cairn>,
    /// §8.2: a rite reports what it spent, because that is where it is a fact.
    /// Not in the trace — a trace holding it could not be replayed (§6.7).
    spent: u64,
}

impl Sealed {
    /// The word for what happened. §6.6: a trace with no holes is *sealed*.
    ///
    /// Nothing else earns the word. A grant that answered nothing leaves the
    /// trace exactly as deep and exactly as full of holes as it was, and
    /// saying "sealed" over that is saying a thing happened.
    fn what(&self) -> &'static str {
        match (self.said.is_empty(), self.holes.is_empty()) {
            (true, _) => "unchanged",
            (false, true) => "sealed",
            (false, false) => "exhumed",
        }
    }

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
        // `was + a + b → now` when something was answered; just the name when
        // nothing was, because there is no arrow to draw between a trace and
        // itself.
        let answered: Vec<String> = self.said.iter().map(|(_, a)| a.short()).collect();
        let became = if answered.is_empty() {
            self.was.short()
        } else {
            format!("{} + {} → {}", self.was.short(), answered.join(" + "), self.now.short())
        };
        let word = self.what();
        let _ =
            writeln!(out, "{word:<8} {became}   depth {}   holes {}", self.depth, self.holes.len());
        if self.said.is_empty() && !self.holes.is_empty() {
            let hint = "§9.1 names the capability each hole's stratum asks for.";
            let _ = writeln!(out, "\nnothing was granted, so nothing was answered. {hint}");
        }
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
        let named = format!(
            "\"was\":\"{}\",\"sealed\":\"{}\",\"was_it\":{}",
            self.was,
            self.now,
            json::string(self.what())
        );
        format!(
            "{{{named},\"depth\":{},\"holes\":{},\"fuel_spent\":{},\"answered\":[{}]}}",
            self.depth,
            self.holes.len(),
            self.spent,
            said.join(",")
        )
    }
}
