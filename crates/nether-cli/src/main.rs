//! The rites.
//!
//! `spec/08-rites.md`. Six verbs and one refusal, and the refusal is the one
//! behaviour the language will never change.

mod bury;
mod cairn;
mod closing;
mod exhume;
mod graft;
mod json;
mod lamp;
mod lamp_at;
mod memo;
mod strata;

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use nether_core::Capability;
use nether_ledger::{Cairn, Node, Store, Stored};

/// What this build is, and it is the tag or it is nothing.
///
/// `CARGO_PKG_VERSION` is the workspace version, which is `0.0.0` and always
/// will be: the crates are not published and the tag is the version. A binary
/// built from `cargo build` is not a release and says so rather than claiming
/// a number somebody might quote back. `scripts/task release` and the release
/// workflow set `NETHER_VERSION` from the tag. 0271.
const VERSION: &str = match option_env!("NETHER_VERSION") {
    Some(tag) => tag,
    None => "(not a release build)",
};

/// §8.8's exit codes, by the names the table gives them.
mod code {
    pub const USAGE_ERROR: u8 = 64;
    pub const MALFORMED: u8 = 65;
    pub const ABSENT: u8 = 66;
    pub const UNIMPLEMENTED: u8 = 69;
    pub const FUEL: u8 = 75;
}

/// The budget a burial or exhumation runs under when nobody says otherwise.
///
/// §8.2 requires this to be finite; it is reported on request rather than
/// recorded in the trace, because a budget that bound leaves no trace.
pub(crate) const DEFAULT_FUEL: u64 = 1_000_000;

/// The codes as this program hands them back.
const FAILED: ExitCode = ExitCode::FAILURE;
fn usage_error() -> ExitCode {
    ExitCode::from(code::USAGE_ERROR)
}

pub(crate) fn bad_usage(what: &str, usage: &str) -> ExitCode {
    eprintln!("nether: {what}");
    eprintln!("{usage}");
    usage_error()
}

/// Split `--flag=value` into `("--flag", Some("value"))`; pass everything else through.
pub(crate) fn split_flag(arg: &str) -> (&str, Option<&str>) {
    if let Some((flag, val)) = arg.split_once('=')
        && flag.starts_with("--")
    {
        return (flag, Some(val));
    }
    (arg, None)
}

pub(crate) fn take_val<'a>(
    inline: Option<&'a str>,
    rest: &mut std::slice::Iter<'a, String>,
) -> Option<&'a str> {
    inline.or_else(|| rest.next().map(String::as_str))
}

pub(crate) fn parse_fuel(
    slot: &mut Option<u64>,
    val: Option<&str>,
    usage: &str,
) -> Result<(), ExitCode> {
    match val.map(str::parse::<u64>) {
        Some(Ok(_)) if slot.is_some() => Err(bad_usage("`--fuel` is given once", usage)),
        Some(Ok(n)) => {
            *slot = Some(n);
            Ok(())
        }
        _ => Err(bad_usage("`--fuel` wants a number of steps", usage)),
    }
}

pub(crate) fn finish(res: Result<(), ExitCode>) -> ExitCode {
    match res {
        Ok(()) => ExitCode::SUCCESS,
        Err(code) => code,
    }
}

pub(crate) enum FlagStep {
    Handled,
    Unknown,
}

/// Parse `--`, `-h`/`--help`, `--json`, rite-specific flags via `on_flag`, and
/// at most one positional argument. Returns `Ok(None)` when help was printed.
pub(crate) fn parse_args<'a>(
    args: &'a [String],
    usage: &str,
    too_many_msg: Option<&str>,
    mut on_flag: impl FnMut(
        &str,
        Option<&'a str>,
        &mut std::slice::Iter<'a, String>,
    ) -> Result<FlagStep, ExitCode>,
) -> Result<Option<(Option<&'a str>, bool)>, ExitCode> {
    let mut wants_json = false;
    let mut pos: Option<&'a str> = None;
    let mut end_opts = false;
    let mut rest = args.iter();
    let too_many = || {
        if let Some(msg) = too_many_msg {
            bad_usage(msg, usage)
        } else {
            eprintln!("{usage}");
            usage_error()
        }
    };
    while let Some(arg) = rest.next() {
        if end_opts {
            if pos.is_some() {
                return Err(too_many());
            }
            pos = Some(arg.as_str());
            continue;
        }
        let (flag, inline) = split_flag(arg);
        match (flag, inline) {
            ("--", None) => end_opts = true,
            ("--help" | "-h", None) => {
                println!("{usage}");
                return Ok(None);
            }
            ("--json", None) => wants_json = true,
            _ => match on_flag(flag, inline, &mut rest)? {
                FlagStep::Handled => {}
                FlagStep::Unknown => {
                    if flag.starts_with('-') && flag != "-" {
                        return Err(bad_usage(&format!("unknown option `{arg}`"), usage));
                    }
                    if pos.is_some() {
                        return Err(too_many());
                    }
                    pos = Some(arg.as_str());
                }
            },
        }
    }
    Ok(Some((pos, wants_json)))
}

/// One `--grant`, by the names §9.1 fixes.
pub(crate) fn parse_grant(arg: Option<&str>, usage: &str) -> Result<Capability, ExitCode> {
    match arg.map(Capability::from_name) {
        Some(Some(cap)) => Ok(cap),
        Some(None) => {
            eprintln!("nether: no capability by that name");
            eprintln!("        §9.1 lists them: store env disk disk! net net! entropy unrecorded");
            Err(usage_error())
        }
        None => Err(bad_usage("`--grant` wants a capability", usage)),
    }
}

/// Read a file from disk, or report why it could not be read.
pub(crate) fn read_file_bytes(path: &Path) -> Result<Vec<u8>, ExitCode> {
    std::fs::read(path).map_err(|e| {
        eprintln!("nether: {}: {e}", path.display());
        FAILED
    })
}

/// Where the ledger is: `$NETHER_STORE` (if non-empty), or `.nether` beside the work.
///
/// §07 does not say, and does not need to: where a store sits changes nothing
/// about what is in it, and a trace means the same thing wherever it was written.
fn store_root() -> PathBuf {
    std::env::var_os("NETHER_STORE")
        .filter(|v| !v.is_empty())
        .map_or_else(|| PathBuf::from(".nether"), PathBuf::from)
}

fn ledger() -> Result<Store, std::io::Error> {
    Store::open(store_root())
}

/// Open the ledger without creating any directories on disk (read-only rites).
fn opened_readonly() -> Store {
    Store::open_readonly(store_root())
}

/// Put it in the ledger, or say why it would not go.
///
/// Twenty-four places across six rites said this in seven lines each, and the
/// shape of the rite was buried under its own error handling. §7.5: a store
/// that will not take an answer is this machine failing rather than the world
/// saying no, so there is nothing for a caller to decide.
///
/// # Errors
///
/// [`FAILED`], with the reason already printed.
fn kept(store: &Store, what: &Stored) -> Result<Cairn, ExitCode> {
    store.put(what).map_err(|e| {
        eprintln!("nether: {e}");
        FAILED
    })
}

/// Open the ledger, or say why it would not open.
///
/// Every rite begins here and each of them said so in six lines.
///
/// # Errors
///
/// [`FAILED`], with the reason already printed.
fn opened() -> Result<Store, ExitCode> {
    ledger().map_err(|e| {
        eprintln!("nether: {e}");
        FAILED
    })
}

/// The cairn a prefix names, or say why it names nothing.
///
/// §7.5 lets a person type as much of a name as tells it apart, so a rite
/// takes a prefix and the store settles it. Not finding one is `ABSENT`, and
/// finding two is the store's sentence rather than this one's.
///
/// # Errors
///
/// [`code::ABSENT`], with the reason already printed.
fn named(store: &Store, prefix: &str) -> Result<Cairn, ExitCode> {
    store.resolve(prefix).map_err(|e| {
        eprintln!("nether: {e}");
        match e {
            nether_ledger::StoreError::Absent(_) | nether_ledger::StoreError::NoMatch(_) => {
                ExitCode::from(code::ABSENT)
            }
            nether_ledger::StoreError::BadPrefix(_) => usage_error(),
            _ => FAILED,
        }
    })
}

/// What a cairn turned out to name, for the sentence that says it is not a
/// trace.
fn kind(stored: &Stored) -> &'static str {
    match stored {
        Stored::Value(_) => "a value",
        Stored::Node(Node::Literal(_)) => "a literal",
        Stored::Node(Node::Apply { .. }) => "an application",
        Stored::Node(Node::Hole { .. }) => "a hole",
        Stored::Node(Node::Deposit { .. }) => "a deposit",
        Stored::Node(Node::Witness { .. }) => "a witness",
        Stored::Node(Node::Trace { .. }) => "a trace",
    }
}

/// Say that a cairn does not name a trace, and what it names instead.
fn not_a_trace(store: &Store, cairn: Cairn) -> ExitCode {
    match store.get(cairn) {
        Ok(stored) => {
            eprintln!("nether: {} is {}, not a trace", cairn.short(), kind(&stored));
            eprintln!("        `nether lamp {}` shows what is there", cairn.short());
            FAILED
        }
        Err(e) => closing::store_err(&e),
    }
}

const USAGE: &str = "\
nether — the Nether C rites

  bury <file.nc>     evaluate as far as the world allows; emit a trace
                     [--fuel <n>] [--json]
  exhume <cairn>     grant a stratum, answer holes, emit a deeper trace
                     [--grant <cap>]... [--declare <k>=<v>]... [--clock <n>]
                     [--target <triple>] [--root <dir>] [--reach <host[:port]>]...
                     [--load <path>]... [--fuel <n>] [--replay] [--json]
  lamp <cairn>       carry light down: render a value, or its provenance
                     [--provenance | --at <byte> [--source <path>]] [--depth <n>] [--json]
  cairn <path>       name a thing by its content; verify a name still holds
                     --verify <cairn> [--json]
  strata <cairn>     which stratum this reached, and the line that took it there
                     [--json]
  graft <cairn>      substitute an answer and re-bury only what changed
                     --replace <cairn> --with <cairn> [--fuel <n>] [--json]

Options:
  -h, --help [rite]  show this message, or usage for one rite
  -V, --version      print version

There is no `nether run`.

The ledger is $NETHER_STORE, or .nether beside the work.
";

/// The refusal. Stated once, here, so it is a rule rather than a joke.
const NO_RUN: &str = "\
nether: there is no `run`.

  A Nether C program is not executed. It is buried — evaluated as far as the
  world allows — and what remains is a trace and the holes the world still
  owes an answer to.

  You probably want:

    nether bury <file.nc>     to evaluate it
    nether exhume <cairn>     to answer its holes
    nether lamp <cairn>       to see what it left behind
";

fn rite_usage(rite: &str) -> Option<&'static str> {
    Some(match rite {
        "bury" => bury::USAGE,
        "exhume" => exhume::USAGE,
        "lamp" => lamp::USAGE,
        "cairn" => cairn::USAGE,
        "strata" => strata::USAGE,
        "graft" => graft::USAGE,
        _ => return None,
    })
}

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("nether {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") => match args.get(1).map(String::as_str) {
            None => {
                print!("{USAGE}");
                ExitCode::SUCCESS
            }
            Some("run") => {
                eprint!("{NO_RUN}");
                usage_error()
            }
            Some(rite) => {
                if let Some(usage) = rite_usage(rite) {
                    println!("{usage}");
                    ExitCode::SUCCESS
                } else {
                    eprintln!("nether: unknown rite `{rite}`");
                    eprint!("{USAGE}");
                    usage_error()
                }
            }
        },
        None => {
            print!("{USAGE}");
            ExitCode::SUCCESS
        }
        Some("run") => {
            eprint!("{NO_RUN}");
            usage_error()
        }
        Some("cairn") => cairn::run(&args[1..]),
        Some("lamp") => lamp::run(&args[1..]),
        Some("strata") => strata::run(&args[1..]),
        Some("bury") => bury::run(&args[1..]),
        Some("exhume") => exhume::run(&args[1..]),
        Some("graft") => graft::run(&args[1..]),
        Some(other) if other.starts_with('-') => {
            eprintln!("nether: unknown option `{other}`");
            eprint!("{USAGE}");
            usage_error()
        }
        Some(other) => {
            eprintln!("nether: unknown rite `{other}`");
            eprint!("{USAGE}");
            usage_error()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{NO_RUN, USAGE};

    /// The one guarantee this binary already makes.
    #[test]
    fn there_is_no_run() {
        assert!(USAGE.contains("There is no `nether run`."));
        assert!(NO_RUN.starts_with("nether: there is no `run`."));
    }

    /// Every rite named in the usage text is a rite the specification defines.
    #[test]
    fn the_six_rites_are_named() {
        for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
            assert!(USAGE.contains(rite), "usage does not mention `{rite}`");
        }
    }
}
