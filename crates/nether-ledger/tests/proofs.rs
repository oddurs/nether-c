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

use std::collections::{BTreeMap, BTreeSet, HashSet};
use std::fmt::Write as _;
use std::fs;
use std::io::Write as _;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

use nether_ledger::{Cairn, Call, Node, Span, Store, Stored, Value, decode_stored};

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
        let hole = Node::Hole { call, stratum: 3, span };

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

// ── 0207: sixteen processes, one store ─────────────────────────────────────

struct Scratch(PathBuf);

impl Scratch {
    fn new(tag: &str) -> Self {
        static N: AtomicU64 = AtomicU64::new(0);
        let path = std::env::temp_dir().join(format!(
            "nether-0207-{tag}-{}-{}",
            std::process::id(),
            N.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 >> 12;
        self.0 ^= self.0 << 25;
        self.0 ^= self.0 >> 27;
        self.0.wrapping_mul(0x2545_F491_4F6C_DD1D)
    }

    fn below(&mut self, n: usize) -> usize {
        usize::try_from(self.next() % n as u64).unwrap_or(0)
    }
}

struct Subgraph {
    objects: Vec<(Cairn, Stored)>,
    edges: Vec<(Cairn, Cairn)>,
    trace: Cairn,
}

/// Commits one self-contained trace subgraph in dependency order (leaves before
/// nodes that reference them) so any prefix of writes interrupted by `SIGKILL`
/// still leaves every committed node with all its forward targets present.
fn commit_subgraph(store: &Store, tag: &str, n: i64) -> Subgraph {
    let v_src = Stored::Value(Value::Bytes(format!("src:{tag}:{n}").into_bytes()));
    let v_arg = Stored::Value(Value::Int(n));
    let v_ans = Stored::Value(Value::Bytes(format!("ans:{tag}:{n}").into_bytes()));
    let v_res = Stored::Value(Value::Int(n.wrapping_mul(3).wrapping_add(1)));

    let src = store.put(&v_src).expect("put src");
    let arg = store.put(&v_arg).expect("put arg");
    let ans = store.put(&v_ans).expect("put ans");
    let res = store.put(&v_res).expect("put res");

    let span = Span { source: src, start: 0, end: 8 };
    let call = Call { function: "read".to_owned(), args: vec![arg] };

    let n_hole = Stored::Node(Node::Hole { call: call.clone(), stratum: 3, span });
    let n_wit = Stored::Node(Node::Witness { stratum: 3, call, answer: ans, span });
    let n_dep = Stored::Node(Node::Deposit { value: res, span });
    let n_app = Stored::Node(Node::Apply { function: src, args: vec![arg, ans], result: res });

    let hole = store.put(&n_hole).expect("put hole");
    let wit = store.put(&n_wit).expect("put witness");
    let dep = store.put(&n_dep).expect("put deposit");
    let app = store.put(&n_app).expect("put apply");

    let n_trace = Stored::Node(Node::Trace {
        residue: res,
        holes: vec![hole],
        witnesses: vec![wit],
        deposits: vec![dep],
        source: src,
        depth: 3,
        unrecorded: false,
    });
    let trace = store.put(&n_trace).expect("put trace");

    let objects = vec![
        (src, v_src),
        (arg, v_arg),
        (ans, v_ans),
        (res, v_res),
        (hole, n_hole),
        (wit, n_wit),
        (dep, n_dep),
        (app, n_app),
        (trace, n_trace),
    ];
    let edges = vec![
        (arg, hole),
        (src, hole),
        (arg, wit),
        (ans, wit),
        (src, wit),
        (res, dep),
        (src, dep),
        (src, app),
        (arg, app),
        (ans, app),
        (res, app),
        (res, trace),
        (hole, trace),
        (wit, trace),
        (dep, trace),
        (src, trace),
    ];
    Subgraph { objects, edges, trace }
}

fn contention_worker() {
    let store_dir = PathBuf::from(std::env::var_os("NETHER_CONTENTION_STORE").unwrap());
    let out_path = PathBuf::from(std::env::var_os("NETHER_CONTENTION_OUT").unwrap());
    let worker_id: usize = std::env::var("NETHER_CONTENTION_ID").unwrap().parse().unwrap();
    let seed: u64 = std::env::var("NETHER_CONTENTION_SEED").unwrap().parse().unwrap();
    let duration_ms: u64 = std::env::var("NETHER_CONTENTION_MS").unwrap().parse().unwrap();
    let endless = std::env::var_os("NETHER_CONTENTION_ENDLESS").is_some();

    let mut store = Store::open(&store_dir).expect("open store in worker");
    let mut rng = Rng(seed);
    let started = Instant::now();
    let deadline = Duration::from_millis(duration_ms);

    let mut puts = 0u64;
    let mut gets = 0u64;
    let mut refs_queries = 0u64;
    let mut committed_objects: Vec<(Cairn, Stored)> = Vec::new();
    let mut committed_edges: Vec<(Cairn, Cairn)> = Vec::new();
    let mut committed_traces: BTreeSet<Cairn> = BTreeSet::new();

    let mut round = 0i64;
    while endless || started.elapsed() < deadline {
        round += 1;
        // Re-open the handle periodically so reads cross the OS page cache
        // rather than hitting only this process's in-memory `Seen` table.
        if round % 8 == 0 {
            store = Store::open(&store_dir).expect("reopen store");
        }

        let slot = rng.below(32);
        let (tag, n) = if slot < 16 {
            ("shared".to_owned(), i64::try_from(slot).unwrap_or(0))
        } else {
            (format!("w{worker_id}"), round)
        };
        let sub = commit_subgraph(&store, &tag, n);
        puts += sub.objects.len() as u64;

        // Also write a worker-distinct node pointing at the shared root so all
        // sixteen processes append concurrently to the same `refs/` file.
        if slot < 16 {
            let shared_target = sub.objects[0].0;
            let fan_in = Stored::Node(Node::Apply {
                function: shared_target,
                args: vec![sub.trace],
                result: shared_target,
            });
            let fan_cairn = store.put(&fan_in).expect("put fan_in");
            puts += 1;
            committed_objects.push((fan_cairn, fan_in));
            committed_edges.push((shared_target, fan_cairn));
            committed_edges.push((sub.trace, fan_cairn));
        }

        if endless {
            let mut log = fs::OpenOptions::new().create(true).append(true).open(&out_path).unwrap();
            log.write_all(sub.trace.as_bytes()).unwrap();
        }

        committed_objects.extend(sub.objects);
        committed_edges.extend(sub.edges);
        committed_traces.insert(sub.trace);

        // Exhumation read + reverse-edge check on an already-committed item.
        let pick_obj = rng.below(committed_objects.len());
        let (cairn, ref expected) = committed_objects[pick_obj];
        let got = store.get(cairn).expect("get committed object");
        assert_eq!(&got, expected, "worker {worker_id} read wrong value for {cairn}");
        gets += 1;

        let pick_edge = rng.below(committed_edges.len());
        let (target, referrer) = committed_edges[pick_edge];
        let who = store.referrers(target).expect("referrers on committed target");
        assert!(
            who.contains(&referrer),
            "worker {worker_id}: edge {referrer} -> {target} missing from referrers"
        );
        refs_queries += 1;
    }

    let mut out = String::new();
    let _ = writeln!(
        out,
        "seed={seed:#018x} puts={puts} gets={gets} refs={refs_queries} traces={}",
        committed_traces.len()
    );
    let seen_objs: BTreeSet<Cairn> = committed_objects.iter().map(|(c, _)| *c).collect();
    for c in seen_objs {
        let _ = writeln!(out, "O {c}");
    }
    let seen_edges: BTreeSet<(Cairn, Cairn)> = committed_edges.into_iter().collect();
    for (t, r) in seen_edges {
        let _ = writeln!(out, "E {t} {r}");
    }
    for t in committed_traces {
        let _ = writeln!(out, "T {t}");
    }
    fs::write(&out_path, out).expect("write worker summary");
}

fn spawn_worker(
    exe: &Path,
    store_dir: &Path,
    out_path: &Path,
    id: usize,
    seed: u64,
    duration_ms: u64,
    endless: bool,
) -> Child {
    let mut cmd = Command::new(exe);
    cmd.arg("--exact")
        .arg("sixteen_processes_contend_on_one_store")
        .arg("--nocapture")
        .env("NETHER_CONTENTION_WORKER", "1")
        .env("NETHER_CONTENTION_STORE", store_dir)
        .env("NETHER_CONTENTION_OUT", out_path)
        .env("NETHER_CONTENTION_ID", id.to_string())
        .env("NETHER_CONTENTION_SEED", seed.to_string())
        .env("NETHER_CONTENTION_MS", duration_ms.to_string())
        .stdout(Stdio::null())
        .stderr(Stdio::piped());
    if endless {
        cmd.env("NETHER_CONTENTION_ENDLESS", "1");
    }
    cmd.spawn().expect("spawn worker process")
}

struct ContentionReport {
    elapsed: Duration,
    total_puts: u64,
    total_gets: u64,
    total_refs: u64,
    unique_objects: usize,
    unique_edges: usize,
    unique_traces: usize,
    summaries: Vec<String>,
}

fn verify_store_and_claims(
    store_dir: &Path,
    all_objects: &BTreeSet<Cairn>,
    all_edges: &BTreeSet<(Cairn, Cairn)>,
    all_traces: &BTreeSet<Cairn>,
) {
    let store = Store::open(store_dir).unwrap();
    let audit = store.audit().expect("audit store after 16-process contention");
    assert!(
        audit.is_clean(),
        "store audit failed after contention: corrupt={:?} missing={:?} malformed={:?} index={:?}",
        audit.corrupt,
        audit.missing,
        audit.malformed,
        audit.index
    );
    assert!(
        audit.affected_traces.is_empty(),
        "affected traces after contention: {:?}",
        audit.affected_traces
    );

    let healthy: BTreeSet<Cairn> = audit.healthy_traces.iter().copied().collect();
    for trace in all_traces {
        assert!(healthy.contains(trace), "committed trace {trace} missing from healthy_traces");
    }
    for &cairn in all_objects {
        store.get(cairn).unwrap_or_else(|e| panic!("committed object {cairn} unreadable: {e}"));
    }
    let mut by_target: BTreeMap<Cairn, Vec<Cairn>> = BTreeMap::new();
    for &(target, referrer) in all_edges {
        by_target.entry(target).or_default().push(referrer);
    }
    for (target, expected_refs) in by_target {
        let actual: BTreeSet<Cairn> =
            store.referrers(target).expect("referrers check").into_iter().collect();
        for r in expected_refs {
            assert!(actual.contains(&r), "committed edge {r} -> {target} was lost");
        }
    }

    let leftover_tmp: Vec<_> = fs::read_dir(store_dir.join("tmp")).unwrap().collect();
    assert!(leftover_tmp.is_empty(), "{} files left in tmp", leftover_tmp.len());
}

fn run_sixteen_processes(duration: Duration) -> ContentionReport {
    let scratch = Scratch::new("sixteen");
    let store_dir = scratch.0.join("store");
    let out_dir = scratch.0.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let _ = Store::open(&store_dir).unwrap();

    let exe = std::env::current_exe().unwrap();
    let duration_ms = u64::try_from(duration.as_millis()).unwrap();
    let started = Instant::now();

    let mut children = Vec::with_capacity(16);
    for id in 0..16usize {
        let seed = 0x0207_0000_0000_0000u64 ^ ((id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let out_path = out_dir.join(format!("worker-{id:02}.txt"));
        let child = spawn_worker(&exe, &store_dir, &out_path, id, seed, duration_ms, false);
        children.push((id, seed, out_path, child));
    }

    let mut total_puts = 0u64;
    let mut total_gets = 0u64;
    let mut total_refs = 0u64;
    let mut all_objects = BTreeSet::new();
    let mut all_edges = BTreeSet::new();
    let mut all_traces = BTreeSet::new();
    let mut summaries = Vec::with_capacity(16);

    for (id, seed, out_path, child) in children {
        let output = child.wait_with_output().expect("wait for worker");
        assert!(
            output.status.success(),
            "worker {id} (seed {seed:#x}) failed with {:?}: {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        let report = fs::read_to_string(&out_path)
            .unwrap_or_else(|e| panic!("worker {id} produced no report: {e}"));
        let mut lines = report.lines();
        let header = lines.next().expect("missing header line");
        summaries.push(format!("w{id:02} exit=0 {header}"));

        let mut w_puts = 0u64;
        let mut w_gets = 0u64;
        let mut w_refs = 0u64;
        for field in header.split_whitespace() {
            if let Some(v) = field.strip_prefix("puts=") {
                w_puts = v.parse().unwrap();
            } else if let Some(v) = field.strip_prefix("gets=") {
                w_gets = v.parse().unwrap();
            } else if let Some(v) = field.strip_prefix("refs=") {
                w_refs = v.parse().unwrap();
            }
        }
        assert!(
            w_puts > 0 && w_gets > 0 && w_refs > 0,
            "worker {id} did partial or zero work: {header}"
        );
        total_puts += w_puts;
        total_gets += w_gets;
        total_refs += w_refs;

        for line in lines {
            if let Some(rest) = line.strip_prefix("O ") {
                all_objects.insert(rest.parse::<Cairn>().unwrap());
            } else if let Some(rest) = line.strip_prefix("E ") {
                let (t, r) = rest.split_once(' ').unwrap();
                all_edges.insert((t.parse::<Cairn>().unwrap(), r.parse::<Cairn>().unwrap()));
            } else if let Some(rest) = line.strip_prefix("T ") {
                all_traces.insert(rest.parse::<Cairn>().unwrap());
            }
        }
    }
    let elapsed = started.elapsed();
    assert!(
        elapsed >= duration,
        "contention finished early: {:.3}s < {:.3}s",
        elapsed.as_secs_f64(),
        duration.as_secs_f64()
    );

    verify_store_and_claims(&store_dir, &all_objects, &all_edges, &all_traces);

    ContentionReport {
        elapsed,
        total_puts,
        total_gets,
        total_refs,
        unique_objects: all_objects.len(),
        unique_edges: all_edges.len(),
        unique_traces: all_traces.len(),
        summaries,
    }
}

/// 0207. Sixteen child processes contend on one store on every test run.
///
/// Also acts as the child process entry when `NETHER_CONTENTION_WORKER=1`.
#[test]
fn sixteen_processes_contend_on_one_store() {
    if std::env::var_os("NETHER_CONTENTION_WORKER").is_some() {
        contention_worker();
        return;
    }
    let secs: u64 =
        std::env::var("NETHER_CONTENTION_SECS").ok().and_then(|s| s.parse().ok()).unwrap_or(2);
    let report = run_sixteen_processes(Duration::from_secs(secs));
    println!(
        "\n  0207  16 processes, {:.2}s: {} puts, {} gets, {} referrers ({} objects, {} edges, {} traces, audit clean)\n",
        report.elapsed.as_secs_f64(),
        report.total_puts,
        report.total_gets,
        report.total_refs,
        report.unique_objects,
        report.unique_edges,
        report.unique_traces,
    );
}

/// 0207. Terminated writers alongside live readers and writers.
///
/// Writers terminated mid-flight by `SIGKILL` alongside live readers and
/// writers never leave a corrupt object, a missing forward target, or a torn
/// reverse-index record, and every trace committed before termination survives.
#[test]
fn terminated_writers_leave_committed_objects_and_audit_clean() {
    let scratch = Scratch::new("killed");
    let store_dir = scratch.0.join("store");
    let out_dir = scratch.0.join("out");
    fs::create_dir_all(&out_dir).unwrap();
    let store = Store::open(&store_dir).unwrap();
    let seed_sub = commit_subgraph(&store, "seed", 0);

    let exe = std::env::current_exe().unwrap();

    // Eight normal processes contend for the whole window while eight endless
    // writers are spawned and killed mid-write with SIGKILL.
    let mut survivors = Vec::with_capacity(8);
    for id in 0..8usize {
        let seed = 0xDEAD_0207_0000_0000u64 ^ ((id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let out_path = out_dir.join(format!("survivor-{id:02}.txt"));
        survivors.push((
            id,
            out_path.clone(),
            spawn_worker(&exe, &store_dir, &out_path, id, seed, 1_500, false),
        ));
    }

    let mut doomed = Vec::with_capacity(8);
    for id in 8..16usize {
        let seed = 0xCAFE_0207_0000_0000u64 ^ ((id as u64 + 1).wrapping_mul(0x9E37_79B9_7F4A_7C15));
        let out_path = out_dir.join(format!("doomed-{id:02}.bin"));
        doomed.push((
            id,
            out_path.clone(),
            spawn_worker(&exe, &store_dir, &out_path, id, seed, 60_000, true),
        ));
    }

    // Wait until every doomed writer has committed at least one trace so we
    // know it is actively inside the write loop before sending SIGKILL.
    let wait_start = Instant::now();
    for (_, out_path, _) in &doomed {
        while fs::metadata(out_path).map_or(0, |m| m.len()) < 32 {
            assert!(
                wait_start.elapsed() < Duration::from_secs(10),
                "timed out waiting for doomed writer to commit first trace"
            );
            std::thread::sleep(Duration::from_millis(5));
        }
    }

    let mut killed_traces = BTreeSet::new();
    killed_traces.insert(seed_sub.trace);
    for (id, out_path, mut child) in doomed {
        std::thread::sleep(Duration::from_millis(15));
        child.kill().expect("kill writer");
        let status = child.wait().expect("reap killed writer");
        assert!(!status.success(), "doomed writer {id} exited cleanly instead of being killed");
        let bytes = fs::read(&out_path).unwrap_or_default();
        for chunk in bytes.chunks_exact(32) {
            let arr: [u8; 32] = chunk.try_into().unwrap();
            killed_traces.insert(Cairn::from_bytes(arr));
        }
    }

    for (id, out_path, child) in survivors {
        let output = child.wait_with_output().expect("wait survivor");
        assert!(
            output.status.success(),
            "survivor {id} failed while sibling writers were killed: {}",
            String::from_utf8_lossy(&output.stderr)
        );
        let report = fs::read_to_string(&out_path).unwrap();
        for line in report.lines().skip(1) {
            if let Some(rest) = line.strip_prefix("T ") {
                killed_traces.insert(rest.parse::<Cairn>().unwrap());
            }
        }
    }

    let fresh = Store::open(&store_dir).unwrap();
    let audit = fresh.audit().expect("audit store after SIGKILL");
    assert!(
        audit.is_clean(),
        "audit not clean after killed writers: corrupt={:?} missing={:?} malformed={:?} index={:?}",
        audit.corrupt,
        audit.missing,
        audit.malformed,
        audit.index
    );
    assert!(audit.affected_traces.is_empty());
    let healthy: BTreeSet<Cairn> = audit.healthy_traces.iter().copied().collect();
    for trace in &killed_traces {
        assert!(
            healthy.contains(trace),
            "trace {trace} committed before SIGKILL is not healthy after audit"
        );
    }
}

/// 0207. Sixteen processes for a minute.
///
/// Sixteen processes burying and exhuming against one store for a full
/// minute leave it consistent, checked by `Store::audit`.
#[test]
#[ignore = "a sixty-second process contention proof; run with scripts/task proofs"]
fn sixteen_processes_one_store_for_a_minute() {
    let report = run_sixteen_processes(Duration::from_secs(60));
    println!(
        "\n  0207  16 processes, {:.2}s measured\n\
           \x20       {} puts, {} gets, {} referrers\n\
           \x20       {} unique objects, {} unique edges, {} healthy traces, audit clean\n",
        report.elapsed.as_secs_f64(),
        report.total_puts,
        report.total_gets,
        report.total_refs,
        report.unique_objects,
        report.unique_edges,
        report.unique_traces,
    );
    for line in &report.summaries {
        println!("        {line}");
    }
}
