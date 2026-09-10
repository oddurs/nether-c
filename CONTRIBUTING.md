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
the Decay Rule, and `cairn check`. `scripts/agent pr` runs it for you and
refuses to push if it fails.

CI runs exactly the same command, so the two cannot drift.

Approvals are not required to merge — this is a one-maintainer project and
requiring a review would deadlock it. The status check still is. That changes to
one approval the moment there is a second maintainer.

## The Decay Rule

TempleOS was fixed at a size given by covenant and never grew. Nether C inverts
that: **the trusted core may only ever get smaller.**

`.decay-ceiling` records the current line count of `crates/`. `scripts/decay`
fails the build if the count exceeds it. Lowering the ceiling is an ordinary
commit. Raising it means editing the file deliberately and saying why in the
pull request body.

Below, things only decay.

## The roadmap

Do not create ad-hoc `TODO`, `PLAN` or `NOTES` files, and do not leave `TODO`
comments in code. Create a cairn item so the work appears on the board.

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

## Reporting a security issue

See [SECURITY.md](SECURITY.md). Do not open a public issue.
