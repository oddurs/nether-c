//! The proof for *diagnostics with source spans*: every diagnostic names a
//! span, and a depth error names the line that caused the descent.
//!
//! Depth errors are the ones people hit constantly and they are the hardest to
//! phrase, because the number and the reason for it are almost never on the
//! same line. *This is @3 because line 10 descended to disk* is the shape, and
//! the way to get it is a second snippet rather than a longer sentence.

use nether_core::{Fault, check, report};
use nether_syntax::{lower, parse};

const PATH: &str = "table.nc";

/// Lex, parse, lower and check, and hand back what went wrong, rendered.
fn diagnose(src: &str) -> Vec<String> {
    let rendered = |f: &nether_syntax::Fault| report(&f.diagnostic(), src, PATH);
    let Ok(ast) = parse(src.as_bytes()) else {
        return parse(src.as_bytes()).unwrap_err().iter().map(rendered).collect();
    };
    match lower(&ast) {
        Err(faults) => faults.iter().map(rendered).collect(),
        Ok(unit) => {
            check(&unit).iter().map(|f: &Fault| report(&f.diagnostic(), src, PATH)).collect()
        }
    }
}

/// Every line of every diagnostic a source can produce, for the shape tests.
fn every_diagnostic() -> Vec<(&'static str, String)> {
    const SOURCES: [&str; 8] = [
        // Lexical.
        "I64 n = @9;\n",
        "I64 n = \"never closed;\n",
        // Syntactic.
        "I64 a = 1\nI64 b = ;\n",
        "U0 f() { descend { 1 }; }\n",
        // Resolution.
        "demand nowhere;\n",
        "U0 f() { descend sudo { 1 }; }\n",
        // Depth.
        "Bytes@0 src = descend disk { must(read(\"main.nc\")) };\n",
        "Shade<Bytes> s = descend net { shade must(get(\"u\")) };\nBytes b = look s;\n",
    ];
    SOURCES.iter().flat_map(|src| diagnose(src).into_iter().map(move |d| (*src, d))).collect()
}

// ── every diagnostic names a span ───────────────────────────────────────────

#[test]
fn every_diagnostic_says_where() {
    let all = every_diagnostic();
    assert!(all.len() >= 8, "only {} diagnostics across the corpus", all.len());
    for (src, rendered) in &all {
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[0].starts_with("error: "), "no headline:\n{rendered}\nfrom {src}");
        assert!(
            lines[1].trim_start().starts_with(&format!("--> {PATH}:")),
            "no location:\n{rendered}\nfrom {src}"
        );
        // A location that names a line and a column, both of them real.
        let at = lines[1].rsplit(':').take(2).collect::<Vec<_>>();
        for part in at {
            assert!(part.parse::<usize>().is_ok_and(|n| n > 0), "{}: {rendered}", lines[1]);
        }
        assert!(rendered.contains('^'), "nothing underlined:\n{rendered}");
    }
}

#[test]
fn the_underlined_text_is_the_text_that_is_wrong() {
    // The caret row lines up under the source row, so the columns can be read
    // off both and compared.
    for (_, rendered) in every_diagnostic() {
        let lines: Vec<&str> = rendered.lines().collect();
        let source_row = lines.iter().find(|l| l.contains(" | ") && !l.contains('^')).unwrap();
        let caret_row = lines.iter().find(|l| l.contains('^')).unwrap();
        let gutter = source_row.find(" | ").unwrap() + 3;
        let first_caret = caret_row.find('^').unwrap();
        assert!(first_caret >= gutter, "the carets are inside the gutter:\n{rendered}");
        assert!(
            first_caret - gutter <= source_row.len() - gutter,
            "the carets are past the end of the line:\n{rendered}"
        );
    }
}

// ── a depth error names the cause ───────────────────────────────────────────

#[test]
fn an_annotation_that_disagrees_names_the_line_that_descended() {
    let src = "// a table\n\
               Bytes@0 src =\n\
               \x20 must(descend disk { read(\"main.nc\") });\n";
    let rendered = diagnose(src);
    assert_eq!(rendered.len(), 1, "{rendered:?}");
    let text = &rendered[0];

    // The symptom is on line 2, where the annotation is.
    assert!(text.contains("error: annotated @0; inference gives 3"), "{text}");
    assert!(text.contains(&format!("--> {PATH}:2:")), "{text}");

    // The cause is on line 3, where the `read` is, and it is named.
    assert!(text.contains(&format!("--> {PATH}:3:")), "the cause is not located:\n{text}");
    assert!(text.contains("`read` reaches stratum 3"), "the cause is not named:\n{text}");

    // And what to do about it.
    assert!(text.contains("inference is not a coercion"), "{text}");
}

#[test]
fn a_descent_with_nothing_in_it_is_named_as_the_cause() {
    // Nothing in the block reaches the disk on its own, so the `descend` is
    // what took it there and the `descend` is what the error names.
    let src = "Bytes@0 b = descend disk { b\"\" };\n";
    let rendered = diagnose(src);
    assert!(rendered.is_empty(), "a descent that reaches nothing is depth 0: {rendered:?}");

    // With something in it, the something is named rather than the descent,
    // because `read` is what somebody wrote and `descend disk` is only where
    // they said they were going.
    let src = "Bytes@0 b = descend disk { must(read(\"k\")) };\n";
    let text = &diagnose(src)[0];
    assert!(text.contains("`read` reaches stratum 3"), "{text}");
}

#[test]
fn a_latent_depth_that_disagrees_names_what_asked_for_it() {
    let src = "Answer<Bytes> raw(Str p) @0\n\
               {\n\
               \x20 read(p)\n\
               }\n";
    let text = &diagnose(src)[0];
    assert!(text.contains("annotated @0; inference gives 3"), "{text}");
    assert!(text.contains(&format!("--> {PATH}:3:")), "the cause is not on line 3:\n{text}");
    assert!(text.contains("`read` reaches stratum 3"), "{text}");
}

#[test]
fn the_orpheus_error_says_the_cause_on_the_caret_row_and_not_twice() {
    // The one depth error whose cause and symptom are the same expression.
    // §1.6 prints it with the blame inline, and a second snippet would be the
    // same line again.
    let src = "Shade<Bytes> s = descend net { shade must(get(\"u\")) };\nBytes b = look s;\n";
    let text = &diagnose(src)[0];
    assert!(text.contains("cannot look at a shade from stratum 5 at depth 0"), "{text}");
    assert!(text.contains("this shade came from `get` at stratum 5"), "{text}");
    assert_eq!(text.matches("-->").count(), 1, "the cause is said twice:\n{text}");
}

// ── the shape of the whole thing ────────────────────────────────────────────

#[test]
fn a_diagnostic_with_a_cause_reads_as_one_thing() {
    let src = "// a table\n\
               Bytes@0 src =\n\
               \x20 must(descend disk { read(\"main.nc\") });\n";
    let text = &diagnose(src)[0];
    let lines: Vec<&str> = text.lines().collect();

    // headline, location, bar, source, carets, bar, location, bar, source,
    // carets, bar, note.
    assert_eq!(lines.len(), 12, "{text}");
    assert!(lines[0].starts_with("error: "));
    assert!(lines[1].contains("-->"));
    assert!(lines[6].contains("-->"));
    assert!(lines[11].contains(" = "));
    // No two adjacent blank gutters, which is what a missing separator looks
    // like.
    for pair in lines.windows(2) {
        assert!(
            !(pair[0].trim() == "|" && pair[1].trim() == "|"),
            "two separators in a row:\n{text}"
        );
    }
}
