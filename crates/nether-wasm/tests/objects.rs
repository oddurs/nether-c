//! The browser receives the same canonical objects as native burial.
use std::collections::BTreeMap;

use nether_core::check;
use nether_ledger::{Cairn, Node, Stored, Value, decode_stored};
use nether_syntax::{lower, parse, print};

fn exported(source: &[u8]) -> BTreeMap<Cairn, Stored> {
    let json = nether_wasm::burying(source, 0);
    let payload = json.split_once("\"objects\":[").expect("objects").1;
    let mut objects = BTreeMap::new();
    for entry in payload.strip_suffix("]}").unwrap().split("],[") {
        let (id, hex) = entry.trim_matches(['[', ']']).split_once(',').unwrap();
        let id: Cairn = id.trim_matches('"').parse().unwrap();
        let hex = hex.trim_matches('"');
        let bytes: Vec<_> = hex
            .as_bytes()
            .chunks_exact(2)
            .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
            .collect();
        let value = decode_stored(&bytes).expect("canonical object");
        assert_eq!(value.encode(), bytes);
        assert_eq!(value.cairn(), id);
        assert!(objects.insert(id, value).is_none(), "duplicate object");
    }
    objects
}

#[test]
fn the_browser_receives_the_native_graph_not_a_summary_of_it() {
    for source in [
        &include_bytes!("../../../tests/programs/hello.nc")[..],
        &include_bytes!("../../../tests/programs/build.nc")[..],
        &b"U0 f() { -9223372036854775807 - 1; 9223372036854775807; b\"a\\0b\"; } demand f();"[..],
        &b""[..],
    ] {
        let objects = exported(source);
        let original = Stored::Value(Value::Bytes(source.to_vec()));
        assert_eq!(objects.get(&original.cairn()), Some(&original));
        let unit = lower(&parse(source).unwrap()).unwrap();
        assert!(check(&unit).is_empty());
        let residue = nether_bury::bury(&unit, original.cairn(), 1_000_000).unwrap();
        for (id, stored) in &residue.named {
            assert_eq!(objects.get(id), Some(stored));
        }
        let printed = Stored::Value(Value::Bytes(print(&residue.as_unit(&unit)).into_bytes()));
        assert_eq!(objects.get(&printed.cairn()), Some(&printed));
        let trace = Stored::Node(Node::Trace {
            residue: printed.cairn(),
            holes: residue.holes,
            witnesses: Vec::new(),
            deposits: residue.deposits,
            source: original.cairn(),
            depth: residue.depth.get(),
            unrecorded: false,
        });
        assert_eq!(objects.get(&trace.cairn()), Some(&trace));
        for stored in objects.values() {
            if let Stored::Node(node) = stored {
                for id in node.references() {
                    assert!(objects.contains_key(&id), "missing reference {id}");
                }
            }
        }
    }
}

#[test]
fn diagnostics_do_not_export_a_partial_graph() {
    let json = nether_wasm::burying(b"demand missing;", 0);
    assert!(json.starts_with("{\"error\":"));
    assert!(!json.contains("\"objects\":"));
}
