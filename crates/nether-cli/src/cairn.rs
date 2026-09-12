//! `nether cairn` — name a thing by its content, or check that a name holds.
//!
//! `spec/08-rites.md` §8.5.

use std::path::Path;
use std::process::ExitCode;

use nether_ledger::{Cairn, Store, StoreError, Stored, Value};

use crate::{FAILED, code, json, ledger, usage_error};

/// `nether cairn <path> | --verify <cairn> [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let wants_json = args.iter().any(|a| a == "--json");
    let rest: Vec<&str> = args.iter().filter(|a| *a != "--json").map(String::as_str).collect();

    match rest.as_slice() {
        ["--verify", name] => verify(name, wants_json),
        [path] if !path.starts_with("--") => name(Path::new(path), wants_json),
        _ => {
            eprintln!("usage: nether cairn <path> | --verify <cairn> [--json]");
            usage_error()
        }
    }
}

/// The cairn of a file's contents, as the `Bytes` they are.
///
/// A file is bytes. Reading it as a `Str` would mean deciding what to do about
/// one that is not UTF-8, and there is nothing to decide: the name of a thing
/// is the name of what it holds.
fn name(path: &Path, wants_json: bool) -> ExitCode {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(e) => {
            eprintln!("nether: {}: {e}", path.display());
            return FAILED;
        }
    };
    let size = bytes.len();
    let cairn = Value::Bytes(bytes).cairn();
    if wants_json {
        println!(
            "{{\"cairn\":\"{cairn}\",\"path\":{},\"bytes\":{size}}}",
            json::string(&path.display().to_string())
        );
    } else {
        println!("{cairn}   {size} bytes   {}", path.display());
    }
    ExitCode::SUCCESS
}

/// Whether the store still holds what that name says it holds.
///
/// §8.5: this MUST report *what* differs, not merely that something does. The
/// store already refuses to serve bytes that do not hash to the name it was
/// asked for; what this adds is saying which of the three things happened.
fn verify(prefix: &str, wants_json: bool) -> ExitCode {
    let store = match ledger() {
        Ok(store) => store,
        Err(e) => {
            eprintln!("nether: {e}");
            return FAILED;
        }
    };
    let cairn = match store.resolve(prefix) {
        Ok(cairn) => cairn,
        Err(e) => return complain(&e, prefix, wants_json),
    };
    match store.get(cairn) {
        Ok(stored) => holds(&store, cairn, &stored, wants_json),
        Err(e) => complain(&e, prefix, wants_json),
    }
}

fn holds(store: &Store, cairn: Cairn, stored: &Stored, wants_json: bool) -> ExitCode {
    let bytes = stored.encode();
    let what = match stored {
        Stored::Value(_) => "value",
        Stored::Node(_) => "node",
    };
    let referrers = store.referrers(cairn).map(|r| r.len()).unwrap_or_default();
    if wants_json {
        println!(
            "{{\"cairn\":\"{cairn}\",\"holds\":true,\"kind\":\"{what}\",\
             \"bytes\":{},\"referrers\":{referrers}}}",
            bytes.len()
        );
    } else {
        println!("{cairn}   holds   {what}, {} bytes", bytes.len());
        if referrers > 0 {
            println!("  named by {referrers} other object(s)");
        }
    }
    ExitCode::SUCCESS
}

/// What went wrong, in the words the store used, and the exit code §8.8 gives
/// it.
fn complain(e: &StoreError, prefix: &str, wants_json: bool) -> ExitCode {
    let (code, said) = match e {
        StoreError::Absent(c) => {
            (ExitCode::from(code::ABSENT), format!("nothing of the name {c} is here"))
        }
        StoreError::NoMatch(p) => {
            (ExitCode::from(code::ABSENT), format!("nothing here starts with {p}"))
        }
        StoreError::Ambiguous { prefix, at_least } => (
            FAILED,
            format!("{prefix} names at least {at_least} objects; picking one would be worse"),
        ),
        StoreError::BadPrefix(p) => {
            (ExitCode::from(code::USAGE_ERROR), format!("{p} is not a cairn or a prefix of one"))
        }
        // §7.5.1: a torn reverse-index record is reported rather than read
        // past. The store is still readable; its provenance is not.
        StoreError::Ragged { of, bytes } => (
            ExitCode::from(code::MALFORMED),
            format!("the reverse index for {of} is {bytes} bytes, which is not whole records"),
        ),
        StoreError::Corrupt { asked, found, why } => (
            FAILED,
            // Bytes that hash to something else are answered with the hash;
            // bytes that hash right and do not decode are the other thing, and
            // §7.7 says what that means.
            match (found, why) {
                (Some(found), _) => format!(
                    "the bytes filed under {asked} hash to {found}.\n  \
                     Something rewrote them, or the disk did."
                ),
                (None, Some(why)) => format!(
                    "the bytes filed under {asked} do not decode: {why}.\n  \
                     They were written under another format version, or they rotted."
                ),
                (None, None) => format!("the bytes filed under {asked} are not what it says"),
            },
        ),
        StoreError::Io(io) => (FAILED, format!("{io}")),
        StoreError::Invalid(why) => (FAILED, format!("{why}")),
    };
    if wants_json {
        println!(
            "{{\"cairn\":{},\"holds\":false,\"why\":{}}}",
            json::string(prefix),
            json::string(&said.replace('\n', " ").replace("  ", " "))
        );
    } else {
        eprintln!("nether: {said}");
    }
    code
}
