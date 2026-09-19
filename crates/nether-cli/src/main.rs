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
mod strata;

use std::path::PathBuf;
use std::process::ExitCode;

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

/// The codes as this program hands them back.
const FAILED: ExitCode = ExitCode::FAILURE;
fn usage_error() -> ExitCode {
    ExitCode::from(code::USAGE_ERROR)
}

/// Where the ledger is.
///
/// `$NETHER_STORE`, or `.nether` beside the work. §07 does not say, and does
/// not need to: where a store sits changes nothing about what is in it, and a
/// trace means the same thing wherever it was written.
fn ledger() -> Result<Store, std::io::Error> {
    let root =
        std::env::var_os("NETHER_STORE").map_or_else(|| PathBuf::from(".nether"), PathBuf::from);
    Store::open(root)
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
        ExitCode::from(code::ABSENT)
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
///
/// Three rites read a trace before they do anything and all three said so
/// differently. Two said only "is not a trace", which tells a reader they are
/// wrong and not what they have — `strata` already said which, and offered
/// the rite that would show them. That is the sentence, and now it is the
/// only one.
///
/// The `let … else` that failed stays where it is: what was duplicated is the
/// sentence, not the shape of reading a trace, and a helper that returned one
/// would leave every caller destructuring a `Node` it had just been promised.
fn not_a_trace(store: &Store, cairn: Cairn) -> ExitCode {
    match store.get(cairn) {
        Ok(stored) => {
            eprintln!("nether: {} is {}, not a trace", cairn.short(), kind(&stored));
            eprintln!("        `nether lamp {}` shows what is there", cairn.short());
        }
        Err(e) => eprintln!("nether: {e}"),
    }
    FAILED
}

const USAGE: &str = "\
nether — the Nether C rites

  bury <file.nc>     evaluate as far as the world allows; emit a trace
  exhume <cairn>     grant a stratum, answer holes, emit a deeper trace
  lamp <cairn>       carry light down: render a value, or its provenance
                     `--provenance` walks backwards; `--depth <n>` bounds it
  cairn <path>       name a thing by its content; verify a name still holds
                     `--verify <cairn>` checks the ledger still holds it
  strata <cairn>     which stratum this reached, and the line that took it there
  graft <cairn>      substitute an answer and re-bury only what changed
                     --replace <cairn> --with <cairn>

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

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();
    match args.first().map(String::as_str) {
        Some("--version" | "-V") => {
            println!("nether {VERSION}");
            ExitCode::SUCCESS
        }
        Some("--help" | "-h") | None => {
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
