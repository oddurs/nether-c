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
