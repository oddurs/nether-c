//! The proofs The Ledger owes.
//!
//! Not unit tests. Each one is the observable fact that settles a roadmap
//! item, and each prints its measurement so the number lands in the pull
//! request rather than in somebody's memory.
//!
//! The million-node pass is `#[ignore]`d: it demonstrates that the format holds
//! at a size where mistakes become visible, which is not worth doing on every
//! commit. `scripts/task proofs` runs it.

#![expect(
    clippy::cast_precision_loss,
    reason = "node counts here are far below 2^52; these are printed measurements, not arithmetic anything depends on"
)]

use std::collections::HashSet;
use std::time::Instant;

use nether_ledger::{Cairn, Call, Node, Span, Stored, Value, decode_stored};

/// A stand-in for a burial, shaped like the thing it stands in for.
///
/// Leaves are files: a `Hole` asking for one, a `Witness` recording what the
/// world said, and a `Literal` holding the bytes. Above them, `Apply` nodes
/// combine pairs, each producing a `Literal` result, up to a single root.
///
/// The shape matters. An earlier version of this chained every node to its
/// predecessor, which made a one-leaf change rename everything after it and
/// reported 50% reuse — a measurement of the generator, not of the design.
struct Graph {
    nodes: Vec<Node>,
    root: Cairn,
}

fn build(leaves: &[&[u8]]) -> Graph {
    let source = Cairn::of_encoded(b"build.nc");
    let combine = Cairn::of_encoded(b"compile");
    let mut nodes = Vec::new();

    let mut level: Vec<Cairn> = Vec::with_capacity(leaves.len());
    for (i, bytes) in leaves.iter().enumerate() {
        let span = Span { source, start: i as u64, end: (i + 1) as u64 };
        let call = Call { function: "read".to_owned(), args: vec![Cairn::of_encoded(b"path")] };

        let literal = Node::Literal(Value::Bytes((*bytes).to_vec()));
        let answer = literal.cairn();
        let witness = Node::Witness { stratum: 3, call: call.clone(), answer, span };
        let hole = Node::Hole { call, stratum: 3, span, depends: vec![] };

        level.push(witness.cairn());
        nodes.push(literal);
        nodes.push(hole);
        nodes.push(witness);
    }

    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            let result =
                Node::Literal(Value::Array(pair.iter().copied().map(Value::Cairn).collect()));
            let apply =
                Node::Apply { function: combine, args: pair.to_vec(), result: result.cairn() };
            next.push(apply.cairn());
            nodes.push(result);
            nodes.push(apply);
        }
        level = next;
    }

    Graph { root: level[0], nodes }
}

fn leaves(n: usize) -> Vec<Vec<u8>> {
    (0..n).map(|i| format!("source chunk {i}").into_bytes()).collect()
}

/// 0043. One million nodes in and out, byte-identical.
#[test]
#[ignore = "a size demonstration; run with scripts/task proofs"]
fn one_million_nodes_round_trip() {
    // Five nodes per leaf once the tree above them is counted.
    let owned = leaves(200_000);
    let refs: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();

    let built = Instant::now();
    let graph = build(&refs);
    let build_time = built.elapsed();
    let n = graph.nodes.len();

    let started = Instant::now();
    let mut encoded_bytes = 0usize;
    for node in &graph.nodes {
        let bytes = node.encode();
        encoded_bytes += bytes.len();
        match decode_stored(&bytes).expect("a node this crate encoded must decode") {
            Stored::Node(back) => {
                assert_eq!(&back, node);
                assert_eq!(back.encode(), bytes, "round-trip was not byte-identical");
            }
            Stored::Value(_) => panic!("a node decoded as a value"),
        }
    }
    let elapsed = started.elapsed();

    println!(
        "\n  0043  {n} nodes  built in {:.2}s  round-tripped in {:.2}s\n\
           \x20       {:.1} MB encoded  {:.0} nodes/sec  root {}\n",
        build_time.as_secs_f64(),
        elapsed.as_secs_f64(),
        encoded_bytes as f64 / 1e6,
        n as f64 / elapsed.as_secs_f64(),
        graph.root.short(),
    );
    assert!(n >= 1_000_000, "wanted a million nodes, built {n}");
}

/// 0044. Two near-identical traces share almost everything.
///
/// This is the entire argument for content addressing. If the number is low,
/// the node model is entangling things that should be independent, and it is
/// better to find out here than in a build system.
#[test]
fn a_one_line_change_reuses_almost_everything() {
    let mut owned = leaves(8_192);
    let before = {
        let refs: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        build(&refs)
    };

    // One source file differs, as a single edited line would make it differ.
    owned[0] = b"source chunk 0, edited".to_vec();
    let after = {
        let refs: Vec<&[u8]> = owned.iter().map(Vec::as_slice).collect();
        build(&refs)
    };

    assert_ne!(before.root, after.root, "an edited source produced the same root");

    let old: HashSet<Cairn> = before.nodes.iter().map(Node::cairn).collect();
    let total = after.nodes.len();
    let reused = after.nodes.iter().filter(|n| old.contains(&n.cairn())).count();
    let ratio = reused as f64 / total as f64;
    let changed = total - reused;

    println!(
        "\n  0044  {total} nodes, one of 8192 sources edited\n\
           \x20       {reused} reused, {changed} renamed ({:.2}% shared)\n",
        ratio * 100.0
    );

    // A change reaches its own nodes and its ancestors, and nothing else.
    // Thirteen levels above 8192 leaves, two nodes per level, plus the leaf.
    assert!(changed < 64, "{changed} nodes changed; a leaf edit should reach ~30");
    assert!(ratio > 0.99, "only {:.2}% reused", ratio * 100.0);
}
