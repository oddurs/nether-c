# Contributing

## The one rule

`main` only ever advances through a merged pull request.

Not for a one-line fix. Not for a typo. Not for setup. A repository ruleset on
the server refuses it and a `pre-push` hook on your machine refuses it, and
both have been tested by trying to violate them, which is the only test of a
protection that means anything.

Discipline is not a mechanism. If the wrong thing is possible, somebody tired
will eventually do it.

## Setup

```sh
scripts/setup            # once per clone: wires .githooks
scripts/agent doctor     # reports every problem, not just the first
```

You need `cargo`, `python3`, `git`, and `gh`. You want
[cairn](https://github.com/oddurs/cairn) too — the roadmap is readable without
it, but not writable.

## One unit of work, one worktree, one branch, one pull request

Parallel work never shares a checkout. Worktrees live beside the repository, not
inside it:

```
nether-c/                              always on main
../.worktrees/nether-c/<branch>/       one directory per branch
```

```sh
scripts/agent start spec/0031-failure-handling
cd ../.worktrees/nether-c/spec/0031-failure-handling
scripts/agent commit "spec(prelude): give starvation a recovery form"
scripts/agent pr
scripts/agent wait
scripts/agent done
```

`start` is also spelled `descend`, `done` is also spelled `surface`, and `list`
is also spelled `strata`. The aliases are not a joke — they are the same verbs
the language uses for the same reasons, and using them keeps the vocabulary in
one piece.

Branch names are `<type>/<slug>`, where type is one of `feat`, `fix`, `chore`,
`docs`, `spec`, `perf`, `refactor` or `test`. Where the work has a roadmap item,
the slug starts with its id: `spec/0031-failure-handling`.

## Commits

Conventional Commits, imperative mood, subject at most 72 characters, no
trailing period. `spec` is a first-class type here, because most of the work is.

```
spec(strata): state the witness obligation for stratum 5

Reading the network sealed the response but never said what the request had
to record alongside it, so two implementations could disagree about what a
replayable trace contains.

Refs: 0012
```

The body explains *why*. The diff already says what.

Never attribute a commit to a tool. The `commit-msg` hook rejects co-author
trailers naming a model, "generated with" footers, and robot emoji.

## Before a pull request

```sh
scripts/task check
```

which is format, lint with warnings denied, tests, build, `site/bake --check`,
the Decay Rule, and `cairn check`. `scripts/agent pr` runs it once through
the pre-push hook and refuses to push if it fails. It requires installed hooks
and a clean feature worktree, including untracked files. Retrying reuses the
open PR rather than creating another one. The hook checks the exact checked-out
commit; it refuses pushes of other branches or commits.

CI runs exactly the same command, so the two cannot drift.

Approvals are not required to merge — this is a one-maintainer project and
requiring a review would deadlock it. The status check still is. That changes to
one approval the moment there is a second maintainer.

## After a pull request

`scripts/agent pr` arms auto-merge. Nobody has to come back and press a button:
the moment `required` goes green the pull request squashes onto `main` and its
branch is deleted. If CI fails it sits there until it is fixed.

`scripts/agent pr --draft` opens it without arming anything, which is how to
say *not yet*.

`scripts/agent wait` waits for checks, then verifies that the PR actually
merged at your local HEAD. Green checks alone are not a merge. Failed checks,
a changed PR head, or a merge that does not complete are reported as failures.

`scripts/agent done` removes only a clean worktree whose exact HEAD was merged.
It never force-removes a worktree, and refuses one holding a `.nether` ledger.
A dirty primary checkout is left untouched and reported; there is no implicit
stash, reset or forced pull. Build caches are ignored and may be removed with
the worktree. Keep unrelated valuable files outside it.

`scripts/agent policy` audits the live server settings without changing them.
Publishing runs this audit too and stops if protection has drifted or cannot
be verified. GitHub is the authority: local hooks are early feedback, not a
replacement for server protection. No extra approval, bot, service, or merge
workflow is needed. Ordinary feature pushes are checked once locally and once
on the PR; the existing main check still verifies the resulting squash.

`scripts/task workflow:check` tests the guards in disposable Git repositories
with an offline GitHub fixture. It is included in the normal test suite.

`main` is protected on the server, and the rule this repository keeps repeating
is now true rather than aspirational:

- a pull request is the only way in, and it must be up to date with `main`
- the `required` check must pass
- history stays linear; no merge commits
- no force pushes, no deleting the branch
- **and all of that is enforced on administrators too**

A direct push to `main` is refused by GitHub, not by a hook somebody can skip.

## Releases

A release is a tag, and `scripts/task release vX.Y.Z` is what makes one. It
refuses off `main`, on a dirty tree, when `main` is behind origin, when the tag
already exists, and when `CHANGELOG.md` has no section for the version — then
runs the full check and pushes the tag.

The tag is the only trigger. `.github/workflows/release.yml` builds `nether`
for linux and both macOS architectures, and publishes them with `nether.woff`,
the specification, and `SHA256SUMS`. The notes are the changelog section, not a
list of commit subjects.

## The Decay Rule

**The trusted core may only ever get smaller.** Keep the code that every
guarantee rests on small enough to inspect.

`.decay-ceiling` records the current line count of `crates/`. `scripts/decay`
fails the build if the count exceeds it. Lowering the ceiling is an ordinary
commit. Raising it means editing the file deliberately and saying why in the
pull request body.

Below, things only decay.

## The roadmap

Do not create ad-hoc `TODO`, `PLAN` or `NOTES` files, and do not leave `TODO`
comments in code. Create a cairn item so the work appears on the board.
Create and claim it inside the feature worktree, not the primary checkout.

```sh
cairn next                 # what is ready
cairn claim --next         # take it
cairn note 0031 "why I did it this way"
cairn close 0031
cairn check                # must pass before you are finished
```

Statuses are the language's own vocabulary: work is `descending` while it is
underway, `starved` when it is waiting on something only somebody else can
supply, and `buried` when it is evaluated as far as the world currently allows.
`unrecorded` is how work gets dropped, and dropping it in public is the point.

Every item carries a **proof**: the observable fact that settles whether it is
done. `cairn list --view unproven` finds items that do not have one. A proof is
not optional here — "the code exists" is not a proof.

## Changing the specification

The specification is canonical as Markdown in `spec/`. The site is baked from
it and committed, so a specification change is not finished until you have run
`site/bake` and committed the result. CI checks this.

Every code sample in the prose is a test fixture. `tests/transcripts/run`
extracts all of them, executes the ones `nether` can answer today, and pins the
rest by hash in `tests/transcripts/MANIFEST.tsv`. Nothing is skipped quietly:
both counts are printed every run.

If you change a sample, the manifest goes stale and the build fails. Re-record
it deliberately:

```sh
tests/transcripts/run --record
```

A transcript that shows only part of a command's output must say so with a line
containing `[...]`. Everything before it must match exactly, and everything
after it must appear in order. An excerpt that does not admit to being one is a
documentation lie, and the harness was written because two of them had already
crept in.

Three things the specification asks of a change:

1. **One claim, one place.** If a rule is already stated in another section,
   link to it rather than restating it. A claim stated twice will eventually be
   stated two different ways.
2. **Say what it cost.** If you close a design question, record the rejected
   alternative in `spec/90-rationale.md`. A design document that lists only
   advantages is marketing.
3. **State the stratum.** If a change touches something that reaches the world,
   say which stratum in the pull request, and set the `stratum` field on the
   item. `cairn list --view deep` is reviewed harder than everything else, and
   that is deliberate.

## Drawing a glyph

One day you will write a sentence with a character nobody has drawn, and the
build will stop:

```
font: the site renders glyphs the regular does not have:
  U+22A5  ⊥  UP TACK

  draw them in site/font.py, or take them out of the page
```

That is `tests/font/run`, and it is correct: the face this site ships is drawn
in this repository, so a character it does not have is a box on somebody's
screen. You do not need to read the TrueType writer to fix it. The drawings
are at the bottom of `site/font.py` and they are eight strings of eight
characters, `#` for ink and `.` for nothing.

```python
g("⊥", "..#.....", "..#.....", "..#.....", "..#.....",
        "..#.....", "..#.....", "#####...", "........")
```

Six things the grid asks of you:

1. **Eight rows of eight.** `g` says so if they are not, and names the glyph.
2. **Six columns of ink**, numbered 0 to 5. The seventh is the blank bearing
   that makes the advance, and the eighth is not yours. Anything drawn past
   column 5 fails the check.
3. **Seven rows tall.** Rows 0 to 6 sit above the baseline and row 7 below it,
   so a capital fills rows 0 to 6, the x-height is rows 2 to 6, and a
   descender drops into row 7.
4. **Name the character, not its number.** `g("⊥", …)`, not `g("\u22a5", …)`.
   The file is UTF-8 and you should be able to see what you are drawing.
5. **Draw it in both weights.** `g` is the regular and `b` is the bold, in a
   section of its own further down. A stem that is one pixel in the regular is
   two in the bold, drawn inward so the advance does not move — and where the
   second pixel would close a counter or fill the gap between two strokes, it
   does not. If a glyph genuinely cannot take more weight in six columns, add
   it to `SAME` and it will be drawn once and used at both.
6. **Look at it.** A font nobody has looked at is a font with a broken glyph
   in it.

```sh
python3 site/font.py --sheet        # every glyph as text
python3 site/font.py --sheet 700    # the same, for the bold
```

Then build what ships and run the checks:

```sh
scripts/task font     # writes site/nether.woff, site/nether-bold.woff and the specimen
scripts/task check    # the whole thing
```

`scripts/task font` writes four files and they are all committed:
`site/nether.woff`, `site/nether-bold.woff` and the two specimen GIFs. A
binary in a diff is a bad review experience and these are in one anyway,
because the thing a reviewer has to read is the bitmaps beside them and the
files are a deterministic function of those — the same drawings give the same
bytes on every machine, and `scripts/task font:check` rebuilds them and fails
if what is committed differs. That is the opposite of
`web/necropolis/nether.wasm`, which is never committed because a Rust release
build is not byte-reproducible and a committed copy could only be checked by
rebuilding it anyway.

`scripts/task font` will refuse a bold that is wider than the regular, lighter
than it, short of a counter it has, no heavier at all without saying so, or
declared the same when it is not. Every one of those is a way a face goes
quietly wrong and a reader finds out before you do.

## Reporting a security issue

See [SECURITY.md](SECURITY.md). Do not open a public issue.
