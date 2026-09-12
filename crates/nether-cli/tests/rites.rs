//! The rites, run as a person runs them.
//!
//! The binary is invoked, not the functions, because §08 is about a command
//! line: its exit codes, its stdout, and the one thing it refuses.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use nether_ledger::{Cairn, Store, Stored, Value};

/// A directory nothing else is using, named after the test that asked.
fn scratch(what: &str) -> PathBuf {
    let at = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("nether-{what}-{at}"));
    std::fs::create_dir_all(&dir).expect("a scratch directory");
    dir
}

fn nether(store: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nether"));
    cmd.args(args);
    if let Some(store) = store {
        cmd.env("NETHER_STORE", store);
    } else {
        cmd.env_remove("NETHER_STORE");
    }
    cmd.output().expect("the binary runs")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("an exit code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("stdout is UTF-8")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("stderr is UTF-8")
}

// ── the refusal ─────────────────────────────────────────────────────────────

#[test]
fn there_is_no_run() {
    // §8.0, the only frozen requirement in the draft.
    let out = nether(None, &["run", "hello.nc"]);
    assert_eq!(code(&out), 64);
    assert!(stderr(&out).starts_with("nether: there is no `run`."), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "a refusal is not output");
    for instead in ["nether bury", "nether exhume", "nether lamp"] {
        assert!(stderr(&out).contains(instead), "it does not say to use `{instead}`");
    }
}

// ── §8.5 `cairn` ────────────────────────────────────────────────────────────

#[test]
fn naming_a_file_names_what_is_in_it() {
    let dir = scratch("name");
    let path = dir.join("kernel.nc");
    std::fs::write(&path, b"demand 1;\n").expect("write");

    let out = nether(None, &["cairn", path.to_str().unwrap()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));

    let want = Value::Bytes(b"demand 1;\n".to_vec()).cairn();
    assert!(stdout(&out).starts_with(&want.to_string()), "{}", stdout(&out));
    assert!(stdout(&out).contains("10 bytes"), "{}", stdout(&out));
}

#[test]
fn two_files_with_the_same_bytes_have_the_same_name() {
    let dir = scratch("same");
    for name in ["a.nc", "b.nc"] {
        std::fs::write(dir.join(name), b"demand 1;\n").expect("write");
    }
    let one = stdout(&nether(None, &["cairn", dir.join("a.nc").to_str().unwrap()]));
    let two = stdout(&nether(None, &["cairn", dir.join("b.nc").to_str().unwrap()]));
    let name = |line: &str| line.split_whitespace().next().unwrap_or_default().to_string();
    assert_eq!(name(&one), name(&two));
    assert_ne!(one, two, "and they are still two different files");
}

#[test]
fn json_is_one_object_and_nothing_else() {
    // §8.1: writes a single JSON object to stdout and nothing else.
    let dir = scratch("json");
    let path = dir.join("with \"quotes\".nc");
    std::fs::write(&path, b"x").expect("write");

    let out = nether(None, &["cairn", path.to_str().unwrap(), "--json"]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.starts_with('{') && text.trim_end().ends_with('}'), "{text}");
    // The path had a quote in it, and the object is still one object.
    assert!(text.contains("\\\"quotes\\\""), "{text}");
    assert!(stderr(&out).is_empty());
}

#[test]
fn a_file_that_is_not_there_fails_for_a_stated_reason() {
    let out = nether(None, &["cairn", "/nowhere/at/all.nc"]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("/nowhere/at/all.nc"), "{}", stderr(&out));
}

// ── §8.5 `--verify` ─────────────────────────────────────────────────────────

fn with_object(what: &str, value: Value) -> (PathBuf, Cairn) {
    let dir = scratch(what);
    let store = Store::open(&dir).expect("a store");
    let cairn = store.put(&Stored::Value(value)).expect("put");
    (dir, cairn)
}

#[test]
fn a_name_the_ledger_still_holds_holds() {
    let (dir, cairn) = with_object("holds", Value::Str("kernel.nc".into()));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.to_string()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
    assert!(stdout(&out).contains("holds"), "{}", stdout(&out));
    assert!(stdout(&out).contains("value"), "{}", stdout(&out));
}

#[test]
fn a_short_prefix_is_enough_when_it_is_unambiguous() {
    let (dir, cairn) = with_object("short", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.short()]);
    assert_eq!(code(&out), 0, "{}", stderr(&out));
}

#[test]
fn a_name_that_is_not_there_is_absent_and_says_so() {
    let (dir, _) = with_object("absent", Value::Int(7));
    let missing = Value::Int(99).cairn();
    let out = nether(Some(&dir), &["cairn", "--verify", &missing.to_string()]);
    assert_eq!(code(&out), 66, "§8.8: a cairn was not found in the ledger");
    assert!(stderr(&out).contains("nothing"), "{}", stderr(&out));
}

#[test]
fn something_that_is_not_a_cairn_is_a_usage_error() {
    let (dir, _) = with_object("bad", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", "zzzz"]);
    assert_eq!(code(&out), 64, "{}", stderr(&out));
}

#[test]
fn a_tampered_object_says_what_differs_and_not_merely_that_it_does() {
    // §8.5: `--verify` MUST report *what* differs. The store will not serve
    // bytes that do not hash to the name it was asked for, and this is the
    // rite that says which of the ways that happened.
    let (dir, cairn) = with_object("tampered", Value::Str("kernel.nc".into()));
    let hex = cairn.to_string();
    let object = dir.join("objects").join(&hex[..2]).join(&hex[2..]);
    std::fs::write(&object, Value::Str("something else".into()).encode()).expect("tamper");

    let out = nether(Some(&dir), &["cairn", "--verify", &hex]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    let said = stderr(&out);
    assert!(said.contains(&hex), "it does not say what was asked for:\n{said}");
    let actually = Value::Str("something else".into()).cairn();
    assert!(said.contains(&actually.to_string()), "it does not say what is there:\n{said}");
}

#[test]
fn bytes_that_hash_right_and_decode_wrong_say_that_instead() {
    // The other way an object can be wrong, and the one §7.7 is about: bytes
    // that are filed under their own name and are not a value this format
    // has. The store will not write one — `put` decodes before it commits —
    // so this is what a different format version, or a bad disk, looks like.
    let dir = scratch("garbage");
    let store = Store::open(&dir).expect("a store");
    drop(store);
    let garbage = [0xffu8, 0xff, 0xff];
    let cairn = Cairn::of_encoded(&garbage);
    let hex = cairn.to_string();
    let fanned = dir.join("objects").join(&hex[..2]);
    std::fs::create_dir_all(&fanned).expect("fan out");
    std::fs::write(fanned.join(&hex[2..]), garbage).expect("write");

    let out = nether(Some(&dir), &["cairn", "--verify", &hex]);
    assert_eq!(code(&out), 1, "{}", stderr(&out));
    assert!(stderr(&out).contains("do not decode"), "{}", stderr(&out));
}

#[test]
fn verify_answers_in_json_too() {
    let (dir, cairn) = with_object("verify-json", Value::Int(7));
    let out = nether(Some(&dir), &["cairn", "--verify", &cairn.to_string(), "--json"]);
    let text = stdout(&out);
    assert_eq!(text.lines().count(), 1, "{text}");
    assert!(text.contains("\"holds\":true"), "{text}");
}

// ── the rest ────────────────────────────────────────────────────────────────

#[test]
fn a_rite_that_is_not_built_yet_says_so_with_its_own_code() {
    for rite in ["bury", "exhume", "lamp", "strata", "graft"] {
        let out = nether(None, &[rite]);
        assert_eq!(code(&out), 69, "{rite}: §8.8 gives 69 to not implemented");
    }
}

#[test]
fn an_unknown_rite_is_a_usage_error_and_prints_the_six() {
    let out = nether(None, &["summon"]);
    assert_eq!(code(&out), 64);
    for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
        assert!(stderr(&out).contains(rite), "the usage does not mention `{rite}`");
    }
}
