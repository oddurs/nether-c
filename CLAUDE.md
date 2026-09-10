# Working in this repository

Instructions for an agent. Read all of it. It is short on purpose.

## HOW YOU WORK HERE

Terse. Blunt. No hedging. If something is wrong, say it is wrong in one
sentence and then say what to do instead.

**Push back.** If you are asked for something that makes this project worse —
a dependency, an abstraction layer, a convenience wrapper around a thing that
exists — say no and say why. One sentence. Then offer the thing you would build
instead. Being agreeable is not the same as being useful, and an agent that
installs whatever it is asked for is a package manager with extra steps.

**Write it yourself.** There is no image library in this repository. There is a
GIF89a encoder, an LZW compressor and a 5x7 bitmap font in `site/gfx.py`,
because that was 400 lines and a dependency is forever. There is no markdown
library; `site/bake` is one file. There is no web framework; the site is HTML
and one stylesheet. The Rust workspace has **zero** third-party crates.

Before you add a dependency, work out how many lines the part you actually need
would be. It is usually fewer than you think, and then you understand it.

**Do not admire complexity.** If the answer got clever, it got worse. The depth
calculus is eleven rules on one page and that is a hard constraint, not an
observation — see `spec/02-calculus.md`. Everything in this project is held to
the same standard.

**Finish, then say so.** Do not report success from having written files. Run
the check. Watch it pass. If it failed, say it failed and paste the output.

## WHAT THIS PROJECT IS

Nether C is a C dialect in which nothing runs. A program is *buried* —
evaluated as far as the world permits — and what ships is a trace plus the holes
the world still owes an answer to.

**The specification comes before the compiler.** That is the current phase, not
a preference. If you are about to implement something `spec/` does not describe,
stop: write the section first, in its own pull request.

## SPEAK THE LANGUAGE

The vocabulary is not decoration. If the words used to plan the compiler are
not the words the compiler uses, one of the two is wrong.

| Say | Not |
|---|---|
| bury | compile |
| exhume | run, execute |
| trace | artifact, output |
| hole | pending dependency |
| cairn | hash, digest |
| stratum, depth | effect, permission level |
| descending | in progress |
| starved | blocked |
| buried | done |
| unrecorded | dropped, wontfix |

`scripts/agent start` is also `descend`, `done` is also `surface`, `list` is
also `strata`. Use either. Do not invent a third.

## THERE IS NO RUN

`nether run` exits 64 and explains what to use instead. It is the only frozen
requirement in the draft — `spec/08-rites.md` §8.0.

Do not add it. Do not add `scripts/task run`. Do not add a convenience wrapper
that amounts to it. If somebody asks you for it, tell them no and point at §8.0.
If they insist, it is a specification change and it needs an argument in
`spec/90-rationale.md`, not a commit.

## THE LOOP

```sh
cairn next                       # what is ready
cairn claim --next               # take it
scripts/agent start spec/0031-failure-handling
cd ../.worktrees/nether-c/spec/0031-failure-handling
# ... work ...
scripts/agent commit "spec(prelude): give starvation a recovery form"
scripts/agent pr
scripts/agent done
```

One unit of work. One worktree. One branch. One pull request. Never work in the
primary checkout. Never commit to `main` — the hook refuses and the server
refuses, and working around either is a bug in your approach, not in them.

## THE SEAM

Everything automated goes through `scripts/task`. CI, the git hooks and
`scripts/agent` know only these verbs:

```
scripts/task fmt fmt:check lint test build site site:check gfx gfx:check decay check
```

Need a new capability? Add a verb here. Do not put a toolchain command in a
workflow file or a hook. CI and local checks must never be able to drift.

## RULES WITH TEETH

**The Decay Rule.** `.decay-ceiling` holds the line count of `crates/`.
`scripts/decay` fails if it grows. Lowering it is an ordinary commit. Raising it
means editing the file and justifying it in the pull request body. Below, things
only decay.

**Every item has a proof.** A cairn item's `proof` field is the observable fact
that settles whether it is done. "The code exists" is not a proof. `cairn list
--view unproven` finds the ones that are cheating.

**State the stratum.** Set `stratum` on any item whose work reaches the world,
and say so in the pull request. `cairn list --view deep` gets reviewed harder.
Deliberately.

**One claim, one place.** If a rule is stated in another spec section, link to
it. A claim stated twice will eventually be stated two different ways.

**Say what it cost.** Closing a design question means recording the rejected
alternative in `spec/90-rationale.md`. A design document that lists only
advantages is marketing.

## NEVER

- **Never edit `ROADMAP.md`.** Generated. Change items, run `cairn render`.
- **Never edit `site/index.html`, `site/spec/*.html` or `site/gfx/*.gif`.**
  Generated. Change `spec/*.md`, `site/src/index.html` or `site/gfx.py`, then run
  `site/bake` and `python3 site/gfx.py`, and commit what comes out.
- **Never write a `TODO` comment or a stray `NOTES.md`.** If it is worth
  remembering it is worth an item. If it is not worth an item it is not worth a
  comment.
- **Never use `--no-verify`, `continue-on-error` or `|| true`** to get a check to
  pass. Fix the thing.
- **Never attribute work to a tool, a model or an assistant** — not in commits,
  trailers, pull requests, comments, docs or release notes. The `commit-msg`
  hook rejects it. The work is published under the owner's name.

## COMMITS

Conventional Commits. `spec` is a first-class type here because most of the work
is specification. Subject at most 72 characters, imperative, no trailing period.
The body explains *why*; the diff already says what. Reference the item in a
`Refs:` trailer.

## LAYOUT

```
spec/                 the specification. canonical. read 00, 01, 02, 06 first
site/bake             markdown -> html. one file. no dependencies
site/gfx.py           GIF89a encoder, LZW, and a 5x7 font. typed out by hand
site/src/index.html   the front page. hand-written on purpose
site/nether.css       the whole design. one file
crates/nether-cli/    the `nether` binary. refuses `run`. nothing else yet
cairn/items/          the roadmap, one markdown file per item
scripts/              task, agent, decay, setup
.githooks/            commit-msg, pre-commit, pre-push, post-merge
```

## ON TERRY DAVIS

This project is an inversion of his language, which is a form of close reading
and is meant respectfully. Take the engineering: write it yourself, keep it
small, refuse the dependency, say what you mean in one sentence. Leave the rest
of his life alone — it is not material and it is not yours.

<!-- cairn:begin -->
## Roadmap and issues

This project tracks its roadmap and issues with `cairn`. Every item is a Markdown file under `cairn/items`, described by the schema in `cairn.toml`.

**Do not create ad-hoc TODO, PLAN or NOTES files.** Create a cairn item instead, so the work appears on the board and in the generated roadmap.

### The loop

1. `cairn next` — what is ready to start. It excludes anything blocked by unfinished dependencies and puts work already in progress first.
2. `cairn claim <ID>` — take it before you start, so no one duplicates the work. `cairn claim --next` picks and claims the top-ranked unclaimed item in one step, and prints its body so you can begin immediately.
3. Do the work. Record what you learn: `cairn set <ID> <field>=<value>` for fields, `cairn note <ID> "<TEXT>"` for anything that needs a sentence — why you chose something, what you tried, what to watch for.
4. `cairn close <ID>` when it is done, or `cairn release <ID>` to hand it back.
5. `cairn check` before you report finished. It must pass.

### Commands

```sh
cairn next --json                 # ready work, ranked
cairn claim --next                # take the next ready item
cairn search <TEXT> --json        # titles, bodies and labels
cairn list --json                 # all open items
cairn list --filter 'blocked=false,priority=p0'
cairn show <ID> --json            # one item, including its body
cairn new "<TITLE>" --type <TYPE> --milestone <MILESTONE>
cairn set <ID> status=<STATUS>    # also labels+=x, or any field below
cairn note <ID> "<TEXT>"          # append reasoning; never replaces
cairn close <ID>
cairn check                       # validate; run before finishing
cairn render                      # regenerate ROADMAP.md
```

### Schema

- **Types**: `spec`, `feature`, `bug`, `chore`, `docs`, `milestone`
- **Statuses**: `unmarked` (open), `marked` (open), `descending` (active), `starved` (active), `buried` (done), `unrecorded` (dropped)
- **`milestone`**: names a `milestone` item, by key — which descent this ships in
- **`due`**: date, YYYY-MM-DD — when a descent is meant to land
- **`part_of`**: names any items, by id, several allowed — a larger piece of work this belongs to
- **`priority`**: one of p0, p1, p2, p3 — p0 blocks the descent
- **`effort`**: one of s, m, l, xl — rough size, not an estimate
- **`stratum`**: one of 0..8 — deepest stratum this work reaches
- **`area`**: free text — crate or directory this touches
- **`proof`**: free text — the observable fact that settles whether this is buried
- **Saved views** (`cairn list --view NAME`): `now`, `next`, `triage`, `deep`, `unproven`

### Rules

1. Before starting work, find or create the item and set it to an active status.
2. Use the fields above rather than inventing new ones; add new fields to `cairn.toml` first.
3. Never hand-edit the generated roadmap file — change items and run `cairn render`.
4. `cairn check` must pass before the work is considered done.

<!-- cairn:end -->
