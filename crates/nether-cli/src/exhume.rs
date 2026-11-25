//! `nether exhume` — grant a stratum, answer holes, bury what is left.
//!
//! `spec/08-rites.md` §8.3, and §6.6. Never revises the trace it was given:
//! answering produces a new trace with a new cairn, and the old one still
//! exists with its hole in it.

use std::collections::HashMap;
use std::fmt::Write as _;
use std::path::Path;
use std::process::ExitCode;

use nether_bury::{Answers, HaltKind, bury_with};
use nether_core::{Capability, Depth, Prim};
use nether_ledger::{Cairn, Call, Node, Span, Store, StoreError, Stored, Value};
use nether_syntax::print;
use nether_world::{
    Declared, Disk, Entropy, Env, Ledger, Net, Recorder, Replay, Unrecorded, World,
};

use crate::{FAILED, code, json, usage_error};

/// The budget, as §8.2 gives `bury` one.
const DEFAULT_FUEL: u64 = 1_000_000;

/// What one invocation asked for. §8.3, and §8.3.1 for the declaration.
struct Asked<'a> {
    name: Option<&'a str>,
    granted: Vec<Capability>,
    declared: Declared,
    reach: Vec<String>,
    load: Vec<String>,
    root: Option<&'a str>,
    clock: Option<i64>,
    target: Option<&'a str>,
    fuel: u64,
    replaying: bool,
    wants_json: bool,
}

impl Default for Asked<'_> {
    fn default() -> Self {
        Self {
            name: None,
            granted: Vec::new(),
            declared: Declared::default(),
            reach: Vec::new(),
            load: Vec::new(),
            root: None,
            clock: None,
            target: None,
            fuel: DEFAULT_FUEL,
            replaying: false,
            wants_json: false,
        }
    }
}

const USAGE: &str = "usage: nether exhume <cairn> [--grant <cap>]... \
                     [--declare <name>=<value>]...\n       [--clock <seconds>] \
                     [--target <triple>] [--root <dir>] [--reach <host[:port]>]...\n       \
                     [--load <path>]... [--fuel <n>] [--replay] [--json]";

fn wrong(what: &str) -> ExitCode {
    eprintln!("nether: {what}");
    eprintln!("{USAGE}");
    usage_error()
}

impl<'a> Asked<'a> {
    /// Read the flags, or say which one was wrong.
    fn parse(args: &'a [String]) -> Result<Self, ExitCode> {
        let mut it = Self::default();
        let mut fuel_set = false;
        let mut end_opts = false;
        let mut rest = args.iter();
        while let Some(arg) = rest.next() {
            if end_opts {
                if it.name.is_some() {
                    return Err(wrong("one cairn at a time"));
                }
                it.name = Some(arg.as_str());
                continue;
            }
            match arg.as_str() {
                "--" => end_opts = true,
                "--json" => it.wants_json = true,
                "--replay" => it.replaying = true,
                "--grant" => {
                    let cap = grant(rest.next())?;
                    if !it.granted.contains(&cap) {
                        it.granted.push(cap);
                    }
                }
                "--declare" => it.declared = declare(it.declared, rest.next())?,
                "--clock" => match rest.next().map(|n| n.parse()) {
                    Some(Ok(_)) if it.clock.is_some() => {
                        return Err(wrong("`--clock` is given once"));
                    }
                    Some(Ok(n)) => it.clock = Some(n),
                    _ => return Err(wrong("`--clock` wants whole seconds since the epoch")),
                },
                "--target" => match rest.next() {
                    Some(_) if it.target.is_some() => {
                        return Err(wrong("`--target` is given once"));
                    }
                    Some(t) if !t.is_empty() && !t.starts_with("--") => it.target = Some(t),
                    _ => return Err(wrong("`--target` wants a triple")),
                },
                "--fuel" => match rest.next().map(|n| n.parse::<u64>()) {
                    Some(Ok(_)) if fuel_set => return Err(wrong("`--fuel` is given once")),
                    Some(Ok(n)) => {
                        it.fuel = n;
                        fuel_set = true;
                    }
                    _ => return Err(wrong("`--fuel` wants a number of steps")),
                },
                "--reach" => match rest.next() {
                    Some(h) if !h.is_empty() && !h.starts_with("--") => it.reach.push(h.clone()),
                    _ => return Err(wrong("`--reach` wants a host, and optionally a port")),
                },
                // §8.3.4: once. Two roots would be two answers to one `read`.
                "--root" => match rest.next() {
                    Some(r) if !r.is_empty() && !r.starts_with("--") && it.root.is_none() => {
                        it.root = Some(r);
                    }
                    Some(_) if it.root.is_some() => return Err(wrong("`--root` is given once")),
                    _ => return Err(wrong("`--root` wants a directory")),
                },
                "--load" => match rest.next() {
                    Some(p) if !p.is_empty() && !p.starts_with("--") => it.load.push(p.clone()),
                    _ => return Err(wrong("`--load` wants the path of a shared object")),
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
            return Err(unused("a reach", "net", "§8.3.2", "how far"));
        }
        // §8.3.3, on the same reasoning.
        if !self.granted.contains(&Capability::Unrecorded) && !self.load.is_empty() {
            return Err(unused("a load", "unrecorded", "§8.3.3", "what"));
        }
        // §8.3.4, on §8.3.1's reasoning: the only default is wherever the rite
        // was run, and a root nothing can use could not be read.
        let disk =
            self.granted.iter().any(|c| matches!(c, Capability::Disk | Capability::DiskWrite));
        if disk && self.root.is_none() {
            eprintln!("nether: granting `disk` means rooting it: `--root`. §8.3.4: there is");
            eprintln!("        no default but where this was run, and §6.7 forbids that one.");
            return Err(usage_error());
        }
        if !disk && self.root.is_some() {
            return Err(unused("a root", "disk", "§8.3.4", "where"));
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

/// A flag that bounds a grant nobody made. §8.0's failure in a smaller place.
fn unused(flag: &str, grant: &str, section: &str, says: &str) -> ExitCode {
    eprintln!("nether: {flag} without `--grant {grant}` could not be used.");
    eprintln!("        {section}: `--grant {grant}` says which stratum, the flag says {says}.");
    usage_error()
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
    let store = if asked.replaying {
        crate::opened_readonly()
    } else {
        match crate::opened() {
            Ok(store) => store,
            Err(code) => return code,
        }
    };
    let cairn = match crate::named(&store, asked.name.unwrap_or_default()) {
        Ok(cairn) => cairn,
        Err(code) => return code,
    };
    match dig_up(&store, cairn, &asked) {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

/// Every hole a trace names, with the question it asks and where it was asked.
///
/// `Err` is the cairn of a hole this ledger cannot read. Dropping one instead
/// left it a hole in the new trace, so a trace half of whose holes were missing
/// exhumed to a trace with fewer holes and no complaint.
fn questions(store: &Store, holes: &[Cairn]) -> Result<Vec<(Call, Span)>, ExitCode> {
    holes
        .iter()
        .map(|h| match store.get(*h) {
            Ok(Stored::Node(Node::Hole { call, stratum, span }))
                if crate::closing::valid_stratum(&call, stratum) =>
            {
                Ok((call, span))
            }
            Ok(Stored::Node(Node::Hole { call, stratum, .. })) => {
                eprintln!(
                    "nether: hole {} claims stratum {stratum} for `{}`",
                    h.short(),
                    call.function
                );
                Err(ExitCode::from(code::MALFORMED))
            }
            Ok(_) => {
                eprintln!("nether: hole {} is not a hole node in this ledger", h.short());
                Err(ExitCode::from(code::MALFORMED))
            }
            Err(StoreError::Absent(_)) => Err(no_such_hole(*h)),
            Err(e) => {
                eprintln!("nether: {e}");
                Err(crate::closing::store_exit(&e))
            }
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

/// The world a grant asks for, or the exit code saying why it could not open.
fn world_from(asked: &Asked) -> Result<World, ExitCode> {
    let root = Path::new(asked.root.unwrap_or_default());
    let mut world = World::sealed();
    for cap in &asked.granted {
        let provider: Box<dyn nether_world::Provider> = match cap {
            Capability::Store => Box::new(Ledger),
            Capability::Env => Box::new(Env::pinned(
                asked.declared.clone(),
                asked.clock.unwrap_or_default(),
                asked.target.unwrap_or_default(),
            )),
            Capability::Entropy => match Entropy::opening() {
                Ok(source) => Box::new(source),
                Err(e) => {
                    eprintln!("nether: this machine has no entropy to grant: {e}");
                    return Err(ExitCode::from(code::UNIMPLEMENTED));
                }
            },
            Capability::Net => Box::new(Net::fetching(asked.reach.clone())),
            Capability::NetWrite => Box::new(Net::sending(asked.reach.clone())),
            Capability::Disk => Box::new(Disk::reading(root)),
            Capability::DiskWrite => Box::new(Disk::writing(root)),
            Capability::Unrecorded => match Unrecorded::loading(&asked.load) {
                Ok(foreign) => Box::new(foreign),
                Err(why) => {
                    eprintln!("nether: that shared object would not load: {why}");
                    return Err(ExitCode::from(code::UNIMPLEMENTED));
                }
            },
        };
        world = world.granting(provider);
    }
    Ok(world)
}

fn gather_replay(store: &Store, witnesses: &[Cairn], holes: &[Cairn]) -> Result<Answers, ExitCode> {
    let have = Replay::of_trace(store, witnesses).map_err(|e| {
        eprintln!("nether: {e}");
        crate::closing::store_exit(&e)
    })?;
    let asked = questions(store, holes)?;
    if let Some((call, _)) = asked.first() {
        // Any open hole in `trace.residue` was NOT answered by `trace.witnesses`
        // (prior witnesses were already consumed into `trace.residue`). Feeding
        // an earlier witness for the same `Call` into a new open hole would
        // fabricate an answer instead of refusing.
        eprintln!("nether: nothing recorded answers `{}`", call.function);
        eprintln!("        §6.7: replay serves the ledger and cannot reach the world.");
        return Err(FAILED);
    }
    let mut answers = Answers::none();
    for (call, value) in have.all() {
        answers = answers.and(call.clone(), value.clone());
    }
    Ok(answers)
}

fn prior_read_answers(
    store: &Store,
    witnesses: &[Cairn],
) -> Result<HashMap<Call, Cairn>, ExitCode> {
    let mut prior: HashMap<Call, Cairn> = HashMap::new();
    for &w in witnesses {
        match store.get(w) {
            Ok(Stored::Node(Node::Witness { stratum, call, answer, .. }))
                if crate::closing::valid_stratum(&call, stratum) =>
            {
                if Depth::new(stratum).is_some_and(Depth::reads) {
                    if let Some(&prev) = prior.get(&call) {
                        if prev != answer {
                            eprintln!(
                                "nether: contradictory prior read witnesses for `{}` ({} vs {})",
                                call.function,
                                prev.short(),
                                answer.short()
                            );
                            return Err(ExitCode::from(code::MALFORMED));
                        }
                    } else {
                        prior.insert(call, answer);
                    }
                }
            }
            Ok(_) => {
                eprintln!(
                    "nether: witness {} is not a valid witness node in this ledger",
                    w.short()
                );
                return Err(ExitCode::from(code::MALFORMED));
            }
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(crate::closing::store_exit(&e));
            }
        }
    }
    Ok(prior)
}

fn answer_holes(
    store: &Store,
    asked: &Asked,
    holes: &[Cairn],
    prior_witnesses: &[Cairn],
    said: &mut Vec<(Call, Cairn)>,
    recorded: &mut Vec<Cairn>,
) -> Result<Answers, ExitCode> {
    let asked_holes = questions(store, holes)?;
    let mut prior_reads = prior_read_answers(store, prior_witnesses)?;
    let world = world_from(asked)?;
    let into = Recorder::new(store);
    let outcomes = world.ask_all(&asked_holes, &into);
    let mut answers = Answers::none();
    for ((call, _), res) in asked_holes.into_iter().zip(outcomes) {
        match res {
            Err(nether_world::Unanswered::Ungranted { .. }) => {}
            Err(e @ nether_world::Unanswered::Collapsed(_)) => {
                eprintln!("nether: {e}");
                eprintln!("        §9.9: a collapse is a bug in the program, not an answer.");
                return Err(FAILED);
            }
            Err(e @ nether_world::Unanswered::Unavailable(_)) => {
                eprintln!("nether: {e}");
                return Err(ExitCode::from(code::UNIMPLEMENTED));
            }
            Err(e) => {
                eprintln!("nether: {e}");
                return Err(FAILED);
            }
            Ok(answer) => {
                if Prim::from_name(&call.function).is_some_and(|p| p.latent().reads()) {
                    if let Some(&prev) = prior_reads.get(&call)
                        && prev != answer.answer()
                    {
                        eprintln!(
                            "nether: `{}` returned {}, disagreeing with earlier read witness {}",
                            call.function,
                            answer.answer().short(),
                            prev.short()
                        );
                        return Err(FAILED);
                    }
                    prior_reads.entry(call.clone()).or_insert(answer.answer());
                }
                let Ok(Stored::Value(value)) = store.get(answer.answer()) else {
                    eprintln!("nether: the answer to `{}` is not readable back", call.function);
                    return Err(FAILED);
                };
                said.push((call.clone(), answer.answer()));
                recorded.push(answer.witness());
                answers = answers.and(call, value);
            }
        }
    }
    Ok(answers)
}

fn first_trace_diff(orig: &Node, next: &Node) -> String {
    let (
        Node::Trace {
            residue: r1,
            holes: h1,
            witnesses: w1,
            deposits: d1,
            source: s1,
            depth: dp1,
            unrecorded: u1,
        },
        Node::Trace {
            residue: r2,
            holes: h2,
            witnesses: w2,
            deposits: d2,
            source: s2,
            depth: dp2,
            unrecorded: u2,
        },
    ) = (orig, next)
    else {
        return "trace node shape changed".to_string();
    };
    if r1 != r2 {
        return format!("residue: expected {}, got {}", r1.short(), r2.short());
    }
    let diff_list = |label: &str, a: &[Cairn], b: &[Cairn]| -> Option<String> {
        for (i, (x, y)) in a.iter().zip(b).enumerate() {
            if x != y {
                return Some(format!("{label}[{i}]: expected {}, got {}", x.short(), y.short()));
            }
        }
        if a.len() != b.len() {
            return Some(format!("{label} count: expected {}, got {}", a.len(), b.len()));
        }
        None
    };
    if let Some(d) = diff_list("holes", h1, h2)
        .or_else(|| diff_list("witnesses", w1, w2))
        .or_else(|| diff_list("deposits", d1, d2))
    {
        return d;
    }
    if s1 != s2 {
        return format!("source: expected {}, got {}", s1.short(), s2.short());
    }
    if dp1 != dp2 {
        return format!("depth: expected {dp1}, got {dp2}");
    }
    if u1 != u2 {
        return format!("unrecorded: expected {u1}, got {u2}");
    }
    "trace hash differed".to_string()
}

fn finish_replay(
    cairn: Cairn,
    orig: &Node,
    deeper: &Node,
    wants_json: bool,
) -> Result<(), ExitCode> {
    let next = Stored::Node(deeper.clone()).cairn();
    if next == cairn {
        if wants_json {
            println!("{{\"cairn\":\"{cairn}\",\"identical\":true}}");
        } else {
            println!("identical.");
        }
        return Ok(());
    }
    let diff = first_trace_diff(orig, deeper);
    if wants_json {
        println!(
            "{{\"cairn\":\"{cairn}\",\"replayed\":\"{next}\",\"identical\":false,\"diff\":{}}}",
            json::string(&diff)
        );
    } else {
        eprintln!("nether: replay produced {} and not {}", next.short(), cairn.short());
        eprintln!("        first differing field: {diff}");
        eprintln!("        §6.7 calls that a bug in the implementation.");
    }
    Err(ExitCode::from(code::MALFORMED))
}

fn emit_sealed(store: &Store, told: &Sealed, wants_json: bool) {
    if wants_json {
        println!("{}", told.json());
    } else {
        print!("{}", told.text(store));
    }
}

fn replay_trace(
    store: &Store,
    cairn: Cairn,
    orig_trace: &Node,
    buried_residue: &nether_bury::Residue,
    residue_cairn: Cairn,
    recorded: Vec<Cairn>,
    wants_json: bool,
) -> Result<(), ExitCode> {
    let Node::Trace { deposits, source, unrecorded: marked, .. } = orig_trace else {
        unreachable!();
    };
    let unrecorded = *marked || crate::closing::unrecorded(store, &recorded)?;
    // Replay verifies equality without mutating the store; compute depth
    // of any replayed holes from `buried_residue.named` rather than `store`.
    let mut replay_hole_depth = buried_residue.depth.get();
    for (c, obj) in &buried_residue.named {
        if buried_residue.holes.contains(c)
            && let Stored::Node(Node::Hole { stratum, .. }) = obj
        {
            replay_hole_depth = replay_hole_depth.max(*stratum);
        }
    }
    let deepest = crate::closing::depth(store, replay_hole_depth, &[], &recorded)?;
    let deeper = Node::Trace {
        residue: residue_cairn,
        holes: buried_residue.holes.clone(),
        unrecorded,
        deposits: crate::closing::deposits(deposits, &buried_residue.deposits),
        witnesses: recorded,
        source: *source,
        depth: deepest,
    };
    finish_replay(cairn, orig_trace, &deeper, wants_json)
}

fn reject_unrecorded_replay(cairn: Cairn) -> ExitCode {
    eprintln!("nether: {} reached stratum 8, so replaying it proves nothing.", cairn.short());
    eprintln!();
    eprintln!("  §1.7: nothing downstream of an unrecorded call is recorded, so a");
    eprintln!("  second run agreeing with the first would be a coincidence and a");
    eprintln!("  second run disagreeing would be the same trace. `nether strata`");
    eprintln!("  says `replayable: no` about this one, and so does this.");
    FAILED
}

fn dig_up(store: &Store, cairn: Cairn, asked: &Asked) -> Result<(), ExitCode> {
    let (replaying, wants_json) = (asked.replaying, asked.wants_json);
    let Ok(Stored::Node(orig_trace @ Node::Trace { .. })) = store.get(cairn) else {
        return Err(crate::not_a_trace(store, cairn));
    };
    let Node::Trace {
        residue,
        ref holes,
        ref witnesses,
        ref deposits,
        source,
        depth: orig_depth,
        unrecorded: marked,
    } = orig_trace
    else {
        unreachable!();
    };

    let (text, unit) = crate::closing::residue_unit(store, cairn, residue)?;
    let shown = format!("{}:residue", cairn.short());

    // Validate holes first to preserve `no_such_hole` diagnostics, then check
    // source, deposits, witnesses, depth, and unrecorded invariants upfront.
    let _ = questions(store, holes)?;
    let prior_unrecorded = crate::closing::verify_trace_parts(
        store, source, deposits, holes, witnesses, orig_depth, marked,
    )?;
    let _ = prior_read_answers(store, witnesses)?;
    if replaying && (marked || prior_unrecorded) {
        return Err(reject_unrecorded_replay(cairn));
    }

    let mut said: Vec<(Call, Cairn)> = Vec::new();
    let mut recorded: Vec<Cairn> = witnesses.clone();
    let answers = if replaying {
        gather_replay(store, witnesses, holes)?
    } else {
        answer_holes(store, asked, holes, witnesses, &mut said, &mut recorded)?
    };

    if !replaying && said.is_empty() {
        let told = Sealed {
            was: cairn,
            now: cairn,
            said,
            depth: orig_depth,
            holes: holes.clone(),
            spent: 0,
            any_granted: !asked.granted.is_empty(),
        };
        emit_sealed(store, &told, wants_json);
        return Ok(());
    }

    // `unit` was parsed from `trace.residue`, so spans generated during
    // re-burial are byte offsets into `residue` rather than `source`.
    let buried_residue = match bury_with(&unit, residue, asked.fuel, &answers) {
        Ok(r) => r,
        Err(halt) => {
            eprint!(
                "{}",
                nether_core::report(&halt.diagnostic(), &String::from_utf8_lossy(&text), &shown)
            );
            return Err(match halt.kind {
                HaltKind::OutOfFuel { .. } => ExitCode::from(code::FUEL),
                _ => FAILED,
            });
        }
    };

    let printed = print(&buried_residue.as_unit(&unit));
    let residue_obj = Stored::Value(Value::Bytes(printed.into_bytes()));
    let unrecorded = marked || crate::closing::unrecorded(store, &recorded)?;

    if replaying {
        return replay_trace(
            store,
            cairn,
            &orig_trace,
            &buried_residue,
            residue_obj.cairn(),
            recorded,
            wants_json,
        );
    }

    for (_, stored) in &buried_residue.named {
        crate::kept(store, stored)?;
    }
    let next_residue = crate::kept(store, &residue_obj)?;
    let deepest =
        crate::closing::depth(store, buried_residue.depth.get(), &buried_residue.holes, &recorded)?;
    let deeper = Node::Trace {
        residue: next_residue,
        holes: buried_residue.holes.clone(),
        unrecorded,
        deposits: crate::closing::deposits(deposits, &buried_residue.deposits),
        witnesses: recorded,
        source,
        depth: deepest,
    };
    let next = crate::kept(store, &Stored::Node(deeper))?;

    let told = Sealed {
        was: cairn,
        now: next,
        said,
        depth: deepest,
        holes: buried_residue.holes.clone(),
        spent: buried_residue.fuel_spent,
        any_granted: !asked.granted.is_empty(),
    };
    emit_sealed(store, &told, wants_json);
    Ok(())
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
    any_granted: bool,
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
            if self.any_granted {
                let _ = writeln!(
                    out,
                    "\nnone of the granted capabilities matched the remaining holes. {hint}"
                );
            } else {
                let _ = writeln!(out, "\nnothing was granted, so nothing was answered. {hint}");
            }
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
