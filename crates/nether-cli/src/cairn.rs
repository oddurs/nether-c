//! `nether cairn` — name a thing by its content, or check that a name holds.
//!
//! `spec/08-rites.md` §8.5.

use std::path::Path;
use std::process::ExitCode;

use nether_ledger::{Cairn, Store, StoreError, Stored, Value};

use crate::{FAILED, code, json, usage_error};

/// `nether cairn <path> | --verify <cairn> [--json]`
pub fn run(args: &[String]) -> ExitCode {
    let mut wants_json = false;
    let mut verify_arg: Option<&str> = None;
    let mut path_arg: Option<&str> = None;
    let mut end_opts = false;
    let mut rest = args.iter();
    while let Some(arg) = rest.next() {
        if end_opts {
            if path_arg.is_some() || verify_arg.is_some() {
                return usage();
            }
            path_arg = Some(arg.as_str());
            continue;
        }
        match arg.as_str() {
            "--" => end_opts = true,
            "--json" => wants_json = true,
            "--verify" => match rest.next() {
                Some(v) if verify_arg.is_none() && path_arg.is_none() => verify_arg = Some(v),
                _ => return usage(),
            },
            other if other.starts_with("--") => return usage(),
            other if path_arg.is_none() && verify_arg.is_none() => path_arg = Some(other),
            _ => return usage(),
        }
    }
    match (verify_arg, path_arg) {
        (Some(name), None) => verify(name, wants_json),
        (None, Some(path)) => name(Path::new(path), wants_json),
        _ => usage(),
    }
}

fn usage() -> ExitCode {
    eprintln!("usage: nether cairn <path> | --verify <cairn> [--json]");
    usage_error()
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
    let store = crate::opened_readonly();
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
    let referrers = match store.referrers(cairn) {
        Ok(r) => r.len(),
        Err(e) => return complain(&e, &cairn.to_string(), wants_json),
    };
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
        StoreError::Bundle(why) => (FAILED, (*why).to_owned()),
    };
    if wants_json {
        let collapsed = said.split_whitespace().collect::<Vec<_>>().join(" ");
        println!(
            "{{\"cairn\":{},\"holds\":false,\"why\":{}}}",
            json::string(prefix),
            json::string(&collapsed)
        );
    } else {
        eprintln!("nether: {said}");
    }
    code
}
