//! The rites.
//!
//! `spec/08-rites.md`. Six verbs and one refusal, and the refusal is the one
//! behaviour the language will never change.

mod cairn;
mod json;
mod lamp;
mod strata;

use std::path::PathBuf;
use std::process::ExitCode;

use nether_ledger::Store;

const VERSION: &str = env!("CARGO_PKG_VERSION");

/// §8.8's exit codes, by the names the table gives them.
mod code {
    pub const USAGE_ERROR: u8 = 64;
    pub const MALFORMED: u8 = 65;
    pub const ABSENT: u8 = 66;
    pub const UNIMPLEMENTED: u8 = 69;
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

const USAGE: &str = "\
nether — the Nether C rites

  bury <file.nc>     evaluate as far as the world allows; emit a trace
  exhume <cairn>     grant a stratum, answer holes, emit a deeper trace
  lamp <cairn>       carry light down: render a value, or its provenance
                     `--provenance` walks backwards; `--depth <n>` bounds it
  cairn <path>       name a thing by its content; verify a name still holds
                     `--verify <cairn>` checks the ledger still holds it
  strata <cairn>     which stratum this reached, and the line that took it there
  graft <cairn>      substitute a subtrace and re-bury only what changed

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
        Some("bury" | "exhume" | "graft") => {
            eprintln!("nether: not yet. The specification lands before the compiler does.");
            eprintln!("        See spec/00-overview.md, and `cairn next` for what is ready.");
            ExitCode::from(code::UNIMPLEMENTED)
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
