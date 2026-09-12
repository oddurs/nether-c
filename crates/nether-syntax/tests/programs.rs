//! The stage two proof: `hello.nc`, `stamp.nc` and `build.nc` compile
//! verbatim from the specification.
//!
//! The programs are lifted unchanged. Where a file has more in it than the
//! fence does — a header comment, or the `compile` §9.2 says a program has to
//! supply for itself — the extra is above the excerpt and the excerpt is still
//! there character for character, which this checks first.
//!
//! So if one of them needs editing to compile, the specification was wrong and
//! gets fixed first. That has happened once already: §1.6's error block
//! depicted a `stamp.nc` indented by two, and §1.6's sample is not.

use nether_core::{check, report};
use nether_syntax::{lower, parse};

fn root() -> String {
    format!("{}/../..", env!("CARGO_MANIFEST_DIR"))
}

fn program(name: &str) -> String {
    std::fs::read_to_string(format!("{}/tests/programs/{name}", root()))
        .unwrap_or_else(|e| panic!("tests/programs/{name}: {e}"))
}

/// The fenced block that starts at `line` of a specification file.
fn fence(file: &str, line: usize) -> String {
    let text = std::fs::read_to_string(format!("{}/spec/{file}", root())).expect("readable");
    text.lines().skip(line).take_while(|l| !l.starts_with("```")).collect::<Vec<_>>().join("\n")
}

/// Every program, and the sample it was lifted from.
const PROGRAMS: [(&str, &str, usize); 3] = [
    ("hello.nc", "00-overview.md", 125),
    ("stamp.nc", "01-strata.md", 114),
    ("build.nc", "06-evaluation.md", 33),
];

#[test]
fn each_program_is_the_specification_s_sample_unchanged() {
    for (name, file, line) in PROGRAMS {
        let source = program(name);
        let sample = fence(file, line);
        assert!(
            source.contains(&sample),
            "{name} is not spec/{file}:{line} verbatim.\n--- the sample ---\n{sample}\n\
             --- the file ---\n{source}"
        );
    }
}

fn compiles(name: &str) -> Result<nether_core::Unit, String> {
    let source = program(name);
    let ast = parse(source.as_bytes()).map_err(|f| format!("{name} does not parse: {f:?}"))?;
    lower(&ast).map_err(|f| format!("{name} does not lower: {f:?}"))
}

// ── the two that compile ────────────────────────────────────────────────────

#[test]
fn hello_nc_compiles() {
    let unit = compiles("hello.nc").expect("hello.nc");
    let faults = check(&unit);
    assert!(faults.is_empty(), "{faults:?}");

    // There is no `main`; there is a demand, and what it demands is the
    // function itself. §6.2.
    assert_eq!(unit.funcs.len(), 1);
    assert_eq!(unit.demands.len(), 1);
    assert_eq!(unit.demands[0].value.depth, nether_core::Depth::PURE);

    // And the deposit is a statement whose value is not `U0`, which is what
    // makes it a deposit rather than a discard. §4.7.
    let nether_core::Stmt::Expr(deposited) = &unit.funcs[0].body.stmts[0] else { panic!() };
    assert_eq!(deposited.ty, nether_core::Type::Str);
}

#[test]
fn build_nc_compiles_and_reaches_stratum_three() {
    let unit = compiles("build.nc").expect("build.nc");
    let faults = check(&unit);
    assert!(faults.is_empty(), "{faults:?}");

    let depth_of = |name: &str| {
        unit.globals.iter().find(|g| g.name == name).map(|g| g.value.depth).expect(name)
    };
    // `src` is a hole, `obj` starves on it, and `unused` is pure and nothing
    // demands it. §6.2.
    assert_eq!(depth_of("src"), nether_core::Depth::DISK);
    assert_eq!(depth_of("obj"), nether_core::Depth::DISK);
    assert_eq!(depth_of("unused"), nether_core::Depth::PURE);

    // And the annotation §6.2 writes is the one inference gives, which is why
    // the file compiles with it in.
    let src = unit.globals.iter().find(|g| g.name == "src").unwrap();
    assert_eq!(src.asserted, Some(nether_core::Depth::DISK));
}

// ── the one that does not ───────────────────────────────────────────────────

#[test]
fn stamp_nc_compiles_and_is_refused_in_exactly_the_way_the_codex_prints() {
    let unit = compiles("stamp.nc").expect("stamp.nc parses and lowers; it is the check it fails");
    let faults = check(&unit);
    assert_eq!(faults.len(), 1, "one mistake, one message: {faults:?}");

    let printed = report(&faults[0].diagnostic(), &program("stamp.nc"), "stamp.nc");
    assert_eq!(printed.trim_end(), the_error_in_the_codex());
}

/// The fenced block in §1.6 that starts with `error:`.
fn the_error_in_the_codex() -> String {
    let text = std::fs::read_to_string(format!("{}/spec/01-strata.md", root())).expect("readable");
    let mut block: Vec<&str> = Vec::new();
    let mut inside = false;
    for line in text.lines() {
        if line.starts_with("```") {
            if inside && block.first().is_some_and(|l| l.starts_with("error:")) {
                return block.join("\n");
            }
            inside = !inside;
            block.clear();
            continue;
        }
        if inside {
            block.push(line);
        }
    }
    panic!("§1.6 no longer prints an error");
}

#[test]
fn what_is_legal_in_stamp_nc_is_legal() {
    // The shade is carried up out of stratum 5 and bound at the surface, and
    // sealing it is legal. Only the look is not. §1.6.
    let unit = compiles("stamp.nc").expect("stamp.nc");
    let reply = unit.globals.iter().find(|g| g.name == "reply").expect("reply");
    assert_eq!(reply.value.depth, nether_core::Depth::PURE, "a shade is pure");
    assert_eq!(
        reply.ty,
        nether_core::Type::Shade {
            origin: nether_core::Depth::NET,
            inner: Box::new(nether_core::Type::Bytes)
        }
    );
    let witness = unit.globals.iter().find(|g| g.name == "witness").expect("witness");
    assert_eq!(witness.ty, nether_core::Type::Cairn);
    assert_eq!(witness.value.depth, nether_core::Depth::PURE);
}
