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

### Added

- **The specification.** Twelve sections in `spec/`: the nine strata, the
  calculus of depth (eleven rules, one page, and that is a constraint), the
  lexical structure, the grammar, the type system, burial and replay, the
  ledger format, the six rites, the prelude, a glossary, and a rationale that
  states what every inversion cost rather than only what it bought.
- **The site.** `site/`, baked out of `spec/` by `site/bake` — one file of
  standard library Python, no dependencies. The HTML is committed. CI fails if
  it drifts.
- **The graphics.** `site/gfx.py`: a GIF89a encoder, an LZW compressor and a
  5×7 bitmap font, typed out rather than imported. There is no image library in
  this repository and there is not going to be one.
- **`nether`**, a binary that refuses `run` correctly and does nothing else.
- **The Decay Rule.** `.decay-ceiling` holds a number. Today it is 64. The
  build fails if the trusted core exceeds it. Below, things only decay.
