---
id: 271
title: The released binary says it is 0.0.0
type: bug
status: buried
milestone: after
created: 2026-09-18
updated: 2026-09-18
priority: p1
effort: s
area: .github
stratum: '0'
proof: A released binary reports the tag it was built from, and the release fails if it does not
---

Downloaded from the v0.2.0 release, checksum verified, and run:

```console
$ ./nether --version
nether 0.0.0
```

`.github/workflows/release.yml` opens by saying what it means to do:

> A tag is the trigger, and the tag is the version.

It never does it. The build is a plain `cargo build`, so `VERSION` is
`env!("CARGO_PKG_VERSION")` and the workspace version is `0.0.0`. Every binary
of every release says the same thing.

## Why nothing caught it

Nothing runs a released binary. `scripts/task check` builds and tests the
workspace, and the release workflow checks that the tag has a changelog
section, that the checksums are written and that the archives are not empty --
but no gate ever asks the artifact what it is. It was found by downloading it
and typing `--version`.

A project whose subject is that a thing should be able to tell you what it is
should not ship one that cannot.

## Acceptance criteria

- [x] A release binary reports the tag it was built from
- [x] A build that is not a release says so rather than claiming a version
- [x] The release fails if the binary and the tag disagree

## 2026-09-18

Found by downloading the v0.2.0 artifact, verifying its checksum and typing --version. Nothing in the release pipeline ever asks the artifact what it is: the gates check that the tag has a changelog section, that the checksums are written and that the archives are not empty.

## 2026-09-18

The gate reads the binary rather than running it, because a cross-compiled artifact does not run on the machine that built it. Two exact strings settle it: the version this tag is, and the sentence a build without one says instead.
