//! The rites.
//!
//! Nothing here compiles Nether C yet — the specification comes first, and the
//! specification is not finished. What this binary does today is refuse
//! correctly, which is the one behaviour the language will never change.

use std::process::ExitCode;

const VERSION: &str = env!("CARGO_PKG_VERSION");

const USAGE: &str = "\
nether — the Nether C rites

  bury <file.nc>     evaluate as far as the world allows; emit a trace
  exhume <cairn>     grant a stratum, answer holes, emit a deeper trace
  lamp <cairn>       carry light down: render a value, or its provenance
  cairn <path>       name a thing by its content; verify a name still holds
  strata <cairn>     which stratum this reached, and the line that took it there
  graft <cairn>      substitute a subtrace and re-bury only what changed

There is no `nether run`.
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
            ExitCode::from(64)
        }
        Some("bury" | "exhume" | "lamp" | "cairn" | "strata" | "graft") => {
            eprintln!("nether: not yet. The specification lands before the compiler does.");
            eprintln!("        See spec/00-overview.md, and `cairn next` for what is ready.");
            ExitCode::from(69)
        }
        Some(other) => {
            eprintln!("nether: unknown rite `{other}`");
            eprint!("{USAGE}");
            ExitCode::from(64)
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
