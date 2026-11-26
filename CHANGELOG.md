# Changelog

This file is the only mutable thing in the project.

Everything else here is content-addressed, append-only and immutable by
construction. This file gets rewritten every release. Somebody should find that
uncomfortable and it may as well be you.

Kept in the format of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).
Nether C will follow [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once there is something to version. Until then the specification is a draft and
changes without notice.

With one exception. There is no `run`. There is not going to be a `run`.

## [Unreleased]

Nothing yet.

## [0.3.0] - 2026-11-26

### Mechanized metatheory and differential checking

- Added an in-tree LCF proof kernel in `proof/` (`kernel.py`, `calculus.py`,
  `orpheus.py`, `staging.py`, `encoding.py`, `check.py`) with zero external
  dependencies, proving depth-lattice soundness, Orpheus premise sufficiency
  and necessity, core residual round-trip and staging equivalence, and canonical
  encoding injectivity (`proof/check.py` runs in under 0.2s).
- Checked `nether-core` against a clean-room transcription of the calculus over
  10,000,000 seeded programs (`scripts/task fuzz`, with 50,000 on every
  `cargo test`), and added [§90.7](spec/90-rationale.md#907-what-is-proved-what-is-tested-and-what-is-believed)
  drawing the boundary between what is proved, what is tested, and what is
  believed.

### Parallel burial, exhumation, and distributed merge

- Independent demands bury in parallel while preserving exact serial trace
  equivalence ([§6.2.1](spec/06-evaluation.md#621-burying-in-parallel)).
- Contiguous read-stratum holes answer in parallel during `exhume` while keeping
  act-stratum holes sequential ([§6.6](spec/06-evaluation.md#answering-in-parallel)).
- Open hole slots can be partitioned across isolated machines and merged back
  deterministically without world capabilities
  ([§6.6](spec/06-evaluation.md#answering-across-machines)).

### Ledger encoding, bundles, reconciliation, audit, and packages

- Made invalid §7.1.1 encoding states unconstructible in `nether-ledger` via
  private-field `Stratum`, `NonEmptyName`, `Span`, and `Call` types (0115),
  dropping `Store::put`'s decode round-trip while keeping full hostile-byte
  rejection on decode.
- Added portable store export/import bundles ([§7.5.4](spec/07-ledger.md#754-moving-a-store)),
  top-down store reconciliation ([§7.5.3](spec/07-ledger.md#753-a-second-store-and-what-they-owe-each-other)),
  and read-only store `fsck` auditing ([§7.5.2](spec/07-ledger.md#752-what-a-damaged-store-can-still-tell-you)).
- Verified 16-process store contention and killed writers (0207), bounded
  single-object and bundle reads (0219), verified reverse-index edges on read,
  and taught `nether strata` to check `unrecorded` marks and corrupt nodes.
- Added offline cairn-addressed package resolution using canonical `Package` and
  `Dep` structs ([§7.5.5](spec/07-ledger.md#755-packages)).

### Burial, rites, and editor integration

- Added speculative reduction under undecided branches and past starved
  statements, plus depth preservation on residues so reduced code never breaks
  `[LOOK]` ([§6.5](spec/06-evaluation.md#65-residue)).
- Added burial memoization ([§8.2.1](spec/08-rites.md#821-burying-what-is-already-buried)),
  required `--root` for `disk`/`disk!` ([§8.3.4](spec/08-rites.md#834-declaring-a-root)),
  `answered` and `refused` constructors ([§9.2](spec/09-prelude.md#92-depth-0)),
  demand-order deposits ([§8.4](spec/08-rites.md#84-lamp)), hole depth in trace
  depth ([§7.3.2](spec/07-ledger.md#732-what-a-decoder-cannot-check)), and
  `graft` refusal on already-given answers ([§8.7](spec/08-rites.md#87-graft)).
- Added `--help`/`-h` and `--flag=value` across all six rites, named unknown
  identifiers in lowering diagnostics, stateless editor inspection
  (`nether lamp <cairn> --at <byte> [--source <path>]`,
  [§8.4.1](spec/08-rites.md#841-inspecting-a-byte-offset-in-an-editor)), and a
  thin Vim/Neovim adapter in `tools/vim/nether.vim`.

### Manual, site, face, and releases

- Wrote the ten-chapter executable manual in `docs/manual/` (every `$ nether`
  transcript executed on every build by `tests/transcripts/run`) and the threat
  model in [SECURITY.md](SECURITY.md).
- Published at `https://netherc.org` with the inspectable glyph specimen page
  (`site/specimen.html`, licence and `@font-face` usage), seamless joining
  block/box glyphs, 1/8-em typography grid, daylight-lit graphics, reduced-motion
  stills for every animated GIF, and Necropolis theme/UX parity.
- Added reproducible release builds (`scripts/task release:build` with
  `--remap-path-prefix` and `--locked`, verified by the second-runner
  `reproduce` job in `.github/workflows/release.yml`), deterministic tarballs,
  standalone `LICENSE` alongside `.woff` assets, and the release compatibility
  contract ([§0.6.1](spec/00-overview.md#061-what-a-release-promises)).

### Hardening and decay

- Completed multi-pass security and soundness hardening across the depth
  checker, parser, lowering, burier, ledger bounds, Linux directory-FD disk
  traversal, foreign FFI (`MAX_OUTPUT = 64 MiB`, temporary `AttemptGuard`),
  HTTP client framing, WASM allocator layout, CLI rites, proof kernel, and CI
  gates, while eliminating `/tmp/nether-*` test leaks.
- Decomposed monoliths across all eight crates, eliminated all lint pragmas and
  production panics/`unreachable!()`, shrank compressed `nether.wasm` to
  ~194.7 KiB (under a `200,000`-byte budget), and lowered `.decay-ceiling` to
  `17,335` lines.

## [0.2.0] - 2026-09-17

No descent closed. This is the accumulation between two of them, and the
largest thing in it is a correction to the calculus that had been sound on
paper and wrong in practice.

### The calculus holds a set, not a number

`δ` was *the deepest stratum whose capability is currently held*, which made one
total order carry two unrelated jobs: how far a value's history reaches, and
which doors are open. It does not survive the second.

`disk!` is stratum 4 and `net` is stratum 5, so this type-checked:

```
descend net { write("kernel.nc", bytes) }
```

`4 ≤ 5`, so the premise held — and a reader auditing that program's descents
was told it touches the network. [§9.1](spec/09-prelude.md) fixes the
capability names precisely so that reading them is an audit, and an order that
implies one permission from another takes the audit back.

So `δ` is a set of strata. The capabilities are in bijection with the strata,
so a set of one is a set of the other and no new object enters the calculus:
`[APP]` is `dƒ ⊆ δ`, `[DESCEND]` adds `{s(κ)}` rather than taking a maximum,
and `[LOOK]` is `d ∈ δ`. **Eleven rules, still.**

### Burial

- A hole is one entry per *question*, not one per node, and two acts are two
  holes. Both were hole-identity bugs: the first collapsed distinct questions
  into one entry, the second merged acts that were never the same act.
- A starved call residualises as its reduced body, which §6 now states.
- A `Bytes` literal has a byte escape, and the lexer reads and writes the bytes
  it holds.

### The Necropolis, partly

- Real trace objects can be navigated and their values inspected.
- A focused neighbourhood of trace records lays out around one node.
- `nether-wasm` exposes canonical trace objects for the graph to walk.

It is not finished and the descent stays open.

### Self-Burial, begun

The interpreter completes the sample-program burial proof, with numeric bounds
and call resolution hardened and names indexed. What the first projection
leaves is written down.

### The face

*There is one face.* That is settled in
[§90.2](spec/90-rationale.md) rather than left as something nobody chose, and
the bold weight is **drawn** rather than faked — which closes the last *Known*
item from 0.1.0. How to draw a glyph the build asks for is documented.

### The site

- Both rooms take an indigo and rose colour system.
- The manual is framed as a peculiar public terminal.
- Graphics recovered, and the bitmap font's spacing preserved.

### Known

- **No descent closed.** Fifty-one items are open across nine milestones. *The
  Necropolis*, *The Face* and *Self-Burial* are all in progress.
- The specification is still a draft and still changes without notice. Ten
  divergences between the spec and itself are filed rather than fixed.
- The workspace version is `0.0.0`. The tag is the version; the crates are not
  published.

## [0.1.0] - 2026-09-14

The first descent. The specification is written, the language buries and
exhumes, and a trace replays with the world switched off.

`nether run` exits 64. It is going to keep doing that.

### The specification

- **Twelve sections in `spec/`**: the nine strata, the calculus of depth
  (eleven rules on one page, and that is a constraint rather than a boast), the
  lexical structure, the grammar, the type system, burial and replay, the
  ledger format, the six rites, the prelude, a glossary, and a rationale that
  records what every inversion cost rather than only what it bought.
- Four holes in it were found by building it rather than by reading it again: a
  struct could not be constructed, a shade parameter had an origin nothing
  could infer, §5.6 described a binding the grammar had no way to write, and
  `Answer` and `Refusal` had encodings with no tags. Each is settled, built and
  tested, and the argument for each is in §90.

### The language

- **`nether bury`** evaluates a program as far as the world permits and emits a
  trace: the residue, the deposits, and the holes the world still owes an
  answer to.
- **`nether exhume`** grants a stratum, answers those holes, and emits a deeper
  trace. It never touches the old one.
- **`nether lamp`** renders a deposited value, or walks its provenance
  backwards.
- **`nether cairn`**, **`nether strata`** and **`nether graft`**: name a thing
  by its content, ask which stratum a trace reached and what took it there, and
  substitute one answer to re-bury only what changed.
- **`nether run`** exits 64 and says what to use instead. It is the only frozen
  requirement in the draft — §8.0.
- Replay holds no capabilities at all. A program that read a file replays on a
  machine where the file is gone, because the answer was written down before
  the program ever saw it.

### How it is built

- **Eight crates**, one registry dependency — `blake3`, because a hash function
  that is subtly wrong fails silently and years late. Everything else is typed
  out: a GIF89a encoder, an LZW compressor, a PNG encoder, a TrueType writer, a
  WOFF container, a markdown renderer, and the face the site reads in.
- **`unsafe` is forbidden in the workspace** and allowed in exactly two crates:
  `nether-foreign`, where Nether C calls out at stratum 8, and `nether-wasm`,
  where a browser calls in. Each is a crate rather than an `allow` on a module
  so the boundary is visible in the dependency graph.
- **The Decay Rule.** `.decay-ceiling` holds the line count of `crates/` and
  the build fails if it grows. Raising it takes an argument in the pull request
  body. Below, things only decay.
- **Eleven test harnesses** beyond the unit tests, including ones that check
  the committed site is not stale, that every glyph the site renders is in the
  face, that every outline decodes back to the pixels it was drawn from, that
  every colour pair clears WCAG AA on both palettes, and that nothing in the
  tree attributes the work to a tool.

### Known

- The specification is a draft and changes without notice. Semantic versioning
  starts when it stops.
- `nether-wasm` builds but the browser front end is unfinished — *The
  Necropolis* is the descent for it.
- The face has one weight, and seventeen stylesheet rules ask for bold, so a
  browser fakes it. *The Face* is the descent for that, and it is the first
  item in it.
