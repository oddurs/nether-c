---
id: 269
title: The release workflow is four majors behind on its artifacts
type: chore
status: buried
milestone: cortege
assignee: Oddur Sigurdsson
created: 2026-09-18
updated: 2026-09-18
priority: p3
effort: s
stratum: '0'
area: .github/workflows
proof: Every action the workflows use is the current major, and a release still uploads and downloads what it built
---

## Starting point and scope

Every action in `.github/workflows/` is current except the two the release
workflow uses to hand a built binary from one job to the next:

| Action | In use | Current |
| --- | --- | --- |
| `actions/upload-artifact` | v4 | v7.0.1 |
| `actions/download-artifact` | v4 | v8.0.1 |

Dependabot opened one pull request for each and neither was taken. They are
the only stale versions; `checkout`, `setup-python`, `configure-pages`,
`deploy-pages`, `upload-pages-artifact` and `rust-cache` are all on the
current major already.

## What the majors actually change

Read rather than assumed, because four majors between two of them is enough to
break a release quietly and a release is the one workflow nothing else
exercises.

- **download-artifact v5** is breaking only for *single artifact downloads by
  ID*. This workflow names no artifact and no id — it downloads all of them
  into `incoming/` and then `find`s the tarballs — so the case that changed is
  not the case this uses.
- **v6 and upload-artifact v5/v6** move to Node 24 and require an Actions
  Runner of at least 2.327.1. Every job here is on `ubuntu-latest` or a
  GitHub-hosted matrix, so this binds nothing; it would bind a self-hosted
  runner, which this repository does not have.
- **download-artifact v8** makes a hash mismatch an error by default rather
  than a warning. That is the rule §7.5.1 already states one layer down — a
  store must never serve bytes that are not what the name says — arriving in
  the build.

## Steps and prerequisites

1. Bump both, in one commit rather than two, since they are two halves of one
   hand-off and a release where only one moved is the state worth avoiding.
2. Close the two dependabot pull requests as superseded.

## Acceptance and evidence

- [x] Both actions are on the current major
- [x] The release workflow still uploads per-target tarballs and gathers them
- [x] The two dependabot pull requests are closed, not left open

## 2026-09-18

Read the majors rather than assumed them. download-artifact v5 is breaking only for single downloads by ID, and this workflow names neither an artifact nor an id. v6 and upload-artifact v5/v6 want a runner of at least 2.327.1, which binds nothing on GitHub-hosted. v8 makes a hash mismatch an error by default, which is §7.5.1's rule arriving in the build.
