//! CLI UX regression tests: help flags, `--flag=value`, unknown options,
//! `bury --grant` permanent refusal, `graft --with` missing-value hint, and
//! unknown-identifier diagnostics.

use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::sync::atomic::{AtomicU64, Ordering};

use nether_ledger::{Store, Stored, Value};

/// Temporary test directory cleaned up automatically when dropped.
struct Scratch(PathBuf);

impl Scratch {
    fn new(label: &str) -> Self {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let n = NEXT.fetch_add(1, Ordering::Relaxed);
        let dir =
            std::env::temp_dir().join(format!("nether-ux-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&dir).expect("create scratch dir");
        Self(dir)
    }
}

impl Deref for Scratch {
    type Target = Path;
    fn deref(&self) -> &Path {
        &self.0
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn nether(store: Option<&Path>, args: &[&str]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_nether"));
    cmd.args(args);
    if let Some(store) = store {
        cmd.env("NETHER_STORE", store);
    } else {
        cmd.env_remove("NETHER_STORE");
    }
    cmd.output().expect("nether runs")
}

fn code(out: &Output) -> i32 {
    out.status.code().expect("exit code")
}

fn stdout(out: &Output) -> String {
    String::from_utf8(out.stdout.clone()).expect("utf8 stdout")
}

fn stderr(out: &Output) -> String {
    String::from_utf8(out.stderr.clone()).expect("utf8 stderr")
}

fn json_field<'a>(line: &'a str, key: &str) -> &'a str {
    let needle = format!("\"{key}\":\"");
    let start =
        line.find(&needle).unwrap_or_else(|| panic!("missing `{key}` in {line}")) + needle.len();
    let end = line[start..].find('"').expect("closing quote");
    &line[start..start + end]
}

#[test]
fn every_rite_supports_help_and_short_h() {
    for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
        for flag in ["--help", "-h"] {
            let out = nether(None, &[rite, flag]);
            assert_eq!(code(&out), 0, "{rite} {flag}: {}", stderr(&out));
            assert!(
                stdout(&out).starts_with(&format!("usage: nether {rite} ")),
                "{rite} {flag}: {}",
                stdout(&out)
            );
            assert!(stderr(&out).is_empty(), "{rite} {flag}: {}", stderr(&out));
        }
    }
}

#[test]
fn top_level_help_names_rite_help_and_flags() {
    let top = nether(None, &["--help"]);
    assert_eq!(code(&top), 0);
    let text = stdout(&top);
    for needle in [
        "--help",
        "--version",
        "--json",
        "--fuel",
        "--at",
        "--grant",
        "--replay",
        "There is no `nether run`.",
    ] {
        assert!(text.contains(needle), "top-level help missing `{needle}`:\n{text}");
    }

    for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
        let out = nether(None, &["--help", rite]);
        assert_eq!(code(&out), 0, "--help {rite}: {}", stderr(&out));
        assert!(
            stdout(&out).starts_with(&format!("usage: nether {rite} ")),
            "--help {rite}: {}",
            stdout(&out)
        );
    }
}

#[test]
fn unknown_top_level_and_rite_options_report_unknown_option() {
    for bogus in ["--bogus", "-z"] {
        let out = nether(None, &[bogus]);
        assert_eq!(code(&out), 64);
        assert!(stderr(&out).contains("unknown option"), "{bogus}: {}", stderr(&out));
        assert!(!stderr(&out).contains("unknown rite"), "{bogus}: {}", stderr(&out));
    }
    for rite in ["bury", "exhume", "lamp", "cairn", "strata", "graft"] {
        for bogus in ["--bogus", "--bogus=1", "-z"] {
            let out = nether(None, &[rite, bogus]);
            assert_eq!(code(&out), 64, "{rite} {bogus}");
            assert!(stderr(&out).contains("unknown option"), "{rite} {bogus}: {}", stderr(&out));
            assert!(
                stderr(&out).contains(&format!("usage: nether {rite} ")),
                "{rite} {bogus}: {}",
                stderr(&out)
            );
        }
    }
}

#[test]
fn bury_usage_omits_grant_and_refuses_grant_permanently() {
    let help = nether(None, &["bury", "--help"]);
    assert_eq!(code(&help), 0);
    assert!(!stdout(&help).contains("--grant"), "bury usage must not advertise --grant");

    let out = nether(None, &["bury", "--grant=disk", "tests/programs/build.nc"]);
    assert_eq!(code(&out), 64);
    let err = stderr(&out);
    assert!(err.contains("never grants capabilities"), "{err}");
    assert!(!err.contains("yet"), "{err}");
    assert!(err.contains("nether exhume"), "{err}");
}

#[test]
fn flag_equals_value_works_across_rites() {
    let dir = Scratch::new("eq-flags");
    let src = dir.join("prog.nc");
    let data = dir.join("in.txt");
    std::fs::write(&data, b"hi").expect("write data");
    std::fs::write(
        &src,
        "Bytes body() { descend disk { must(read(\"in.txt\")) } }\ndemand body();\n",
    )
    .expect("write src");

    let buried =
        nether(Some(&dir), &["bury", src.to_str().expect("utf8"), "--fuel=100000", "--json"]);
    assert_eq!(code(&buried), 0, "{}", stderr(&buried));
    let trace = json_field(&stdout(&buried), "cairn").to_owned();

    let verified = nether(Some(&dir), &["cairn", &format!("--verify={trace}"), "--json"]);
    assert_eq!(code(&verified), 0, "{}", stderr(&verified));
    assert!(stdout(&verified).contains("\"holds\":true"));

    let prov = nether(Some(&dir), &["lamp", &trace, "--provenance", "--depth=2"]);
    assert_eq!(code(&prov), 0, "{}", stderr(&prov));

    let exhumed = nether(
        Some(&dir),
        &[
            "exhume",
            &trace,
            "--grant=disk",
            &format!("--root={}", dir.display()),
            "--fuel=100000",
            "--json",
        ],
    );
    assert_eq!(code(&exhumed), 0, "{}", stderr(&exhumed));
    assert!(stdout(&exhumed).contains("\"was_it\":\"sealed\""), "{}", stdout(&exhumed));
}

#[test]
fn graft_missing_with_cairn_hints_that_cairn_does_not_store() {
    let dir = Scratch::new("graft-hint");
    let src = dir.join("prog.nc");
    std::fs::write(
        &src,
        "Bytes body() { descend disk { must(read(\"a.txt\")) } }\ndemand body();\n",
    )
    .expect("write src");
    let buried = nether(Some(&dir), &["bury", src.to_str().expect("utf8"), "--json"]);
    assert_eq!(code(&buried), 0, "{}", stderr(&buried));
    let out_json = stdout(&buried);
    let trace = json_field(&out_json, "cairn");

    let store = Store::open(&*dir).expect("open store");
    let missing_val = Value::Bytes(b"not in store".to_vec()).cairn();
    let Stored::Node(nether_ledger::Node::Trace { holes, .. }) =
        store.get(trace.parse().expect("cairn")).expect("get trace")
    else {
        panic!("expected trace");
    };
    let hole = holes[0].to_string();

    let grafted = nether(
        Some(&dir),
        &["graft", trace, &format!("--replace={hole}"), &format!("--with={missing_val}")],
    );
    assert_eq!(code(&grafted), 66, "{}", stderr(&grafted));
    assert!(stderr(&grafted).contains("nether cairn"), "{}", stderr(&grafted));
}

#[test]
fn unknown_identifier_diagnostic_prints_the_name_without_cascade() {
    let dir = Scratch::new("unknown-name");
    let src = dir.join("bad.nc");
    std::fs::write(&src, "U0 f() { deposit(1); }\ndemand f();\n").expect("write src");

    let out = nether(Some(&dir), &["bury", src.to_str().expect("utf8")]);
    assert_eq!(code(&out), 65);
    let err = stderr(&out);
    assert!(err.contains("unknown name `deposit`"), "{err}");
    assert!(!err.contains("this does not name this name"), "{err}");
    assert!(!err.contains("a function to apply"), "unexpected cascade:\n{err}");
}

#[test]
fn doctrinal_refusals_cite_the_spec_and_preserve_unknown_rite() {
    let cases = [
        ("exec", "nether: no `exec` either; use `bury`, `exhume`, or `lamp` (§8.0)\n"),
        ("ascend", "nether: there is no `ascend`; depth only ever increases (§1.1)\n"),
        ("print", "nether: nothing is printed; carry `nether lamp <cairn>` (§8.4)\n"),
        ("undo", "nether: nothing in the ledger is revised; use `graft` (§8.7)\n"),
        ("edit", "nether: nothing in the ledger is revised; use `graft` (§8.7)\n"),
        ("delete", "nether: nothing in the ledger is revised; use `graft` (§8.7)\n"),
        ("mutate", "nether: nothing in the ledger is revised; use `graft` (§8.7)\n"),
        ("summon", "nether: unknown rite `summon`\n"),
    ];
    for (verb, expected) in cases {
        let out = nether(None, &[verb]);
        assert_eq!(code(&out), 64, "{verb}: {}", stderr(&out));
        assert!(stdout(&out).is_empty(), "{verb} stdout: {}", stdout(&out));
        let err = stderr(&out);
        assert!(err.starts_with(expected), "{verb}: got:\n{err}");
        assert!(err.contains("There is no `nether run`."), "{verb}: missing usage:\n{err}");
    }

    let help_ascend = nether(None, &["--help", "ascend"]);
    assert_eq!(code(&help_ascend), 64, "--help ascend: {}", stderr(&help_ascend));
    assert!(stdout(&help_ascend).is_empty(), "--help ascend stdout: {}", stdout(&help_ascend));
    let err = stderr(&help_ascend);
    assert!(
        err.starts_with("nether: there is no `ascend`; depth only ever increases (§1.1)\n"),
        "--help ascend: got:\n{err}"
    );
    assert!(err.contains("There is no `nether run`."), "--help ascend: missing usage:\n{err}");
}
