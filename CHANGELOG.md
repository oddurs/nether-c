# Changelog

Kept in the format of [Keep a Changelog](https://keepachangelog.com/en/1.1.0/).

Nether C follows [Semantic Versioning](https://semver.org/spec/v2.0.0.html)
once there is something to version. Until then the specification is a draft and
changes without notice, with one exception: there is no `run`, and there is not
going to be.

## [Unreleased]

### Added

- The specification, in `spec/`: twelve sections covering the strata, the
  calculus of depth, the grammar, the type system, burial and replay, the
  ledger format, the rites, the prelude, a glossary, and a rationale that
  states what each inversion cost.
- `site/`: the specification rendered to static HTML by `site/bake`, one file
  of standard library Python with no dependencies.
- `nether`, a binary that refuses `run` correctly and does nothing else yet.
- The Decay Rule: `scripts/decay` fails the build if the trusted core grows.
