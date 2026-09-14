---
id: 237
title: The release workflow ships nothing, and its gate cannot fail
type: bug
status: buried
milestone: necropolis
assignee: Oddur Sigurdsson
created: 2026-09-14
updated: 2026-09-14
priority: p0
effort: m
area: .github/workflows
stratum: '4'
proof: a tagged release carries binaries, the face and the spec with checksums, its notes come from CHANGELOG.md, and a deliberately broken check fails the release run
---

## Two problems, one file

`.github/workflows/release.yml` has never run — there are no tags — and it has
two faults waiting for the first person who cuts one.

**Its gate cannot fail.** It installs cairn with

    cargo install cairn --locked || echo "cairn unavailable; scripts/task skips it"

which is the exact line `ci.yml` carries a comment about, having fixed it
there and not here. It is wrong twice: `cairn` on crates.io is an unrelated
crate — the binary comes from `cairn-md` in a different repository — and the
`||` means the release goes out green whatever happened.

**It ships nothing.** `gh release create --generate-notes` makes a release with
no assets. Somebody who wants `nether` has to build it, and somebody who wants
the face has to clone the repository for a 3.2kB file.

## What a release should carry

- `nether` for linux x86_64 and macOS on both architectures
- `nether.woff`, which people will want on its own
- the specification, as one archive
- `SHA256SUMS`, because a project about content addressing that ships
  unverifiable tarballs is not making its own argument

Notes should come from `CHANGELOG.md`, which is already written in a format
with a section per version, rather than from a list of commit subjects.

## 2026-09-14

Rewrote it. Three jobs: the same gate CI runs, a build matrix, then publish.

The gate could not fail. It installed 'cairn' from crates.io, which is an unrelated crate — the binary comes from cairn-md in another repository — and the '|| echo' meant the release went out green whatever happened. ci.yml carries a comment explaining that exact mistake, having fixed it there and not here. Same line, pinned to the tag, no ||.

A release now carries nether for linux x86_64 and macOS on both architectures, nether.woff on its own because people will want the face without cloning for one file, the spec as an archive, and SHA256SUMS. A project whose argument is that you can name what you were given should not ship tarballs nobody can check.

Notes come from CHANGELOG.md via scripts/notes rather than from a list of commit subjects, and the check job runs it before anything builds — a tag with no section in the changelog is a release nobody described, and it is cheaper to find that out before publishing.

scripts/task release vX.Y.Z cuts one: refuses off main, refuses a dirty tree, refuses if main is not what origin has, refuses an existing tag, refuses a version with no notes, then runs check and tags. The tag is the only trigger.

Two bugs of my own on the way, both mine and both caught by writing the test. scripts/notes first returned the heading's date as its first line, because the match stopped at the ']' rather than the end of the line. And my first fixture was malformed — I inserted a section between the Unreleased heading and its body — which made the script look wrong when it was right. tests/notes/run has six cases now and reinstating the first bug fails two of them.
