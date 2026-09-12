---
section: "90"
title: Rationale
status: non-normative
---

# Rationale

Non-normative throughout. Nothing here constrains an implementation.

This section exists because a design document that lists only advantages is
marketing. For each inversion: what HolyC did, what Nether C does, and what
that cost.

## 90.1 The inversions

### The command line is the compiler → there is no `run`

HolyC compiles and executes each top-level statement as it is read. The shell,
the REPL and the build are one act, and the result is a system with
extraordinary immediacy: you type, and the machine has already done it.

Nether C moves all of it to the other side. Burial is partial evaluation, and
the artifact is the residue.

**What it cost.** Immediacy. There is no REPL and there cannot be a good one,
because the thing a REPL is for — see the effect now — is precisely what this
language refuses to do. The Necropolis playground is the compensation, and it
is a worse experience than a REPL for the things a REPL is best at.

### A bare string prints → a bare expression deposits

The syntax is preserved exactly; the semantics are reversed. `"Hello\n";` is
still a complete statement, and it still means *this value matters*. It simply
no longer reaches anybody.

**What it cost.** `printf` debugging. The first thing every programmer does in
a new language is print something, and in Nether C that takes two commands
instead of one. This is the single largest ergonomic tax in the design, and it
is charged on the first thirty seconds of every newcomer's experience.

The defence is that it buys the strongest property in the language: a program
holds no capability that reaches a log, so it cannot leak to one. Whether that
trade is worth it is genuinely arguable, and if Nether C fails to find users,
this is the most likely reason.

### The Oracle → stratum 7

Terry's random number generator was a channel: entropy arriving from above,
the highest thing in the system. Nether C puts randomness at depth 7 — nearly
the deepest thing there is — because it is the one act that makes a program
impossible to re-derive.

This is the cleanest inversion in the design. The same mechanism sits at
opposite ends of the same axis, for reasons each system states outright.

**What it cost.** Nothing, in engineering terms. Everything, in the sense that
it is the point where Nether C is most obviously arguing with TempleOS rather
than merely differing from it. That is intentional and it is meant respectfully.

### Ring 0 for everyone → nine strata

TempleOS gives every task every privilege over all of memory at all times, and
gets a system that switches tasks in half a microsecond and is a joy to
program. Nether C partitions authority into a lattice and records the
partition in every type.

**What it cost.** Speed of expression. In HolyC you write what you mean. In
Nether C you write what you mean, discover it is at depth 5, and go back to
add a `descend`. Depth inference is what keeps this from being intolerable,
and if inference is not good enough the language is not usable.

### Everything mutable → nothing mutable

Both systems share everything. TempleOS lets every task write to it; Nether C
lets none write to anything.

**What it cost.** Every algorithm that wants an in-place update. §5.4 permits
building an aggregate before it is first read, which covers construction but
not, say, a hash table that is updated a million times. A Nether C program
that needs one will be slow, and the honest answer today is that such a
program should be written somewhere else and called at stratum 8.

### DolDoc → the ledger graph

TempleOS's file is alive: source, documentation, UI and runnable macros in one
illuminated hypertext, with sprites drawn inline. It is the most charming
thing about the system.

Nether C makes the *text* disposable and the *graph* canonical. The source is
one rendering of the program; the trace is what actually exists.

**What it cost.** Charm, so far. The ledger graph is a better substrate for
diffing, provenance and reproducibility, and it is currently much less
pleasant to look at than a DolDoc file. The Necropolis exists to close that
gap and has not yet been built, so this comparison is not yet one Nether C
wins.

### 640×480 forever → the Decay Rule

Terry fixed the dimensions of the system by covenant and never changed them.
Nether C's counterpart is that the trusted core may only ever get smaller —
checked in CI, recorded in `.decay-ceiling`.

A fixed constraint and a monotonically decreasing one are both ways of
refusing to let a system sprawl. The second is more demanding.

## 90.2 Rejected alternatives

### The Orpheus rule: two rejected forms

**Scope re-stain.** `look` raises the depth of the whole enclosing scope to
the shade's origin. Mythologically exact — turning around costs you everything
— and it produces errors nobody can read: one `look` deep inside a function
poisons every caller, and the diagnostic points at the caller rather than the
cause.

**Binding taint.** Only the bound value takes the depth. Tractable, familiar,
and it reduces `shade` to a newtype: if looking is free, nothing has been
carried.

The rule in [§1.6](01-strata.md#16-shade-and-the-orpheus-rule) — *you may only
look by going back down* — was chosen because it is a local check rather than
a propagation, gives an error that names the exact fix, and keeps the myth
intact. Orpheus could have kept her; what he could not do was look on the way
up.

### Stratum 8: refusing foreign code

The purer design has no stratum 8. Every value in every trace would then have
a complete history, unconditionally, and every claim in this specification
could drop its qualifier.

It was rejected because a language that cannot call C cannot be adopted, and
an unadopted language proves nothing about whether its ideas were good. The
quarantine in [§1.7](01-strata.md#17-stratum-8-the-unrecorded) is the
compromise: foreign code is permitted, and a trace that used it is permanently
forbidden from claiming to be replayable.

This remains the weakest part of the design.

### Failure: three rejected forms

**Exceptions.** Rejected outright. Unwinding presumes a stack that exists at a
single moment, and burial has no such moment — an expression may be evaluated
now, residualised, and finished a week later by somebody else's exhumation.
There is nothing to unwind to.

**`rescue e else f`, catching a collapse.** Superficially the most elegant
option: a collapsed node would be a first-class thing in the trace, so expose
it and let a program handle it. Rejected for two reasons. It makes bugs
catchable, and a bug that can be caught is a bug that will be ignored. Worse, it
makes evaluation order observable — whether `rescue` fires depends on how far
burial got before it collapsed, which is exactly the kind of dependence
[§6.2](06-evaluation.md#62-demand) exists to forbid.

**Nothing, and say so.** Declare failure a burial-level diagnostic and put
recovery outside the language. Honest, and fatal: a build system that cannot
say *if this file is missing, generate it* is not a build system, and build
systems are the case that motivates the whole design.

What was chosen instead splits the question in two. A no from the world is an
*answer* and gets a value; a mistake in the program is a *collapse* and gets a
stopped burial. The cost is a check at every world-touching call site, which is
the bargain C has always offered and which this language is in no position to
improve on.

### One word for starving and for collapsing

An earlier draft used **starve** for both: for an expression waiting on a hole
([§6.4](06-evaluation.md#64-starvation-and-fuel)) and for one that can never
produce a value at all
([§9.9](09-prelude.md#99-failure-and-the-difference-between-two-of-them)).
[§10](10-glossary.md) defended the merge — "deliberately given one name
because both stop a burial" — and that reason was not true of the first one.
§6.4 says a starved expression is *residualised and waits*. It does not stop
anything; it is the ordinary outcome, and it is what the whole language is for.

Two other ways out were available.

**Rename §6.4's instead**, and keep `starve` for the fatal case. Rejected
because the word is already spent: `starved` means *waiting on something
somebody else must supply* in this project's vocabulary table and in the
roadmap's own item status, and a term that means one thing in the tooling and
its opposite in the specification is worse than either.

**Leave them merged and rely on context.** Rejected on contact with the
implementation: `nether-bury` has to tell them apart to report either, one is
a `Halt` and the other is not, and a reader of §9.9 who has not read §6.4
comes away believing burial stops whenever anything waits.

So the fatal one is a **collapse**. It reads in the register the rest of the
language is written in — a burial that hits one caves in — and it leaves
`starve` meaning the one thing it means everywhere else.

### A shade's origin: in the value, or only in the type

[§5.3](05-types.md#53-equality) once said both things at once. Equality is
structural — two values are equal exactly when their encodings are — and then,
one paragraph later, that two shades are equal when their *underlying* values
are. [§7.1](07-ledger.md#71-canonical-encoding) encodes a shade as an origin
byte and the cairn of what it holds, so a shade of the same bytes from stratum
3 and from stratum 5 were unequal by the first rule and equal by the second.

The other way out was to drop the origin byte, making `Shadeᵈ⟨T⟩` encode
exactly as the cairn of what it holds and leaving the origin a fact the type
carries. Rejected, and not only because [§7.1](07-ledger.md#71-canonical-encoding)
is frozen. [§1.6](01-strata.md#16-shade-and-the-orpheus-rule) says a shade MAY
be stored, and the only place a Nether C value is stored is the ledger. A shade
read back out of the ledger with no origin has lost the one thing that makes
the Orpheus rule checkable, and `look` on it could not be typed at all. The
origin is not metadata about the shade; it is half of what a shade is.

What that costs is the sentence in [§1.5](01-strata.md#15-seal): `seal` on a
shade no longer reaches through to the value inside. It names the shade. That
is the tighter rule anyway — a shade is opaque, and reaching through an opaque
thing for a name was a small hole in it — and it takes nothing away, because
`seal e` is legal at any depth and can simply be written before the `shade`.

### Normalising text in the ledger

An earlier draft had the ledger encode `Str` over its NFC form, so that two
spellings of one string could not produce two cairns. It was removed while
freezing [§7.1](07-ledger.md#71-canonical-encoding), for two reasons.

The first is a matter of what a content-addressed store is for. If the store
normalises, then `cairn(s)` is not the name of `s`; it is the name of something
the store decided `s` ought to be. A program that writes a decomposed string
and reads back a composed one has been lied to by the one component whose
entire job is not lying about bytes.

The second is that normalisation belongs where the ambiguity is actually a
problem: identifiers. Two spellings of a variable name must resolve to one
binding, and [§3.3](03-lexical.md#33-identifiers) makes the lexer responsible
for that. Nothing below the lexer needs an opinion.

It also removes a dependency — full NFC is a megabyte of Unicode tables — but
that is a consequence of the decision, not the reason for it. Had the argument
gone the other way, the tables would have been the right thing to add.

### Writing our own cryptography

The repository has no third-party dependencies. Not in Rust, not in the site,
not in the graphics: `site/gfx.py` contains a GIF89a encoder, an LZW
compressor, a PNG encoder and a bitmap font rather than importing any of them,
and the argument each time was that the part actually needed was smaller than
the cost of the dependency.

That argument does not survive contact with [§7.2](07-ledger.md#72-cairns),
which specifies blake3.

A hash function is not an encoder. An encoder that is subtly wrong produces a
file somebody notices. A hash function that is subtly wrong produces cairns
that look fine, verify fine, and collide — and the failure surfaces years later
as two different values with the same name, in a store nobody can now audit,
underneath every reproducibility claim in this document. Timing behaviour,
buffer edge cases and the compression-function rounds are exactly the places
where hand-written implementations go wrong quietly.

So the rule is narrower than "no dependencies", and it is worth stating
precisely, because the broad version is the kind of principle that feels good
until it costs something:

> Write your own encoders, parsers, renderers and formats. Do not write your
> own cryptography.

`blake3` is a permitted dependency. Every addition after it needs its own
entry in this section.

This is also consistent with the Decay Rule rather than in tension with it. The
ceiling counts the lines this project is responsible for, and three hundred
lines of hand-rolled hashing would be three hundred lines of exactly the code
nobody should be reviewing here.

### Unicode, and the line the rule is actually drawn on

[§3.3](03-lexical.md#33-identifiers) needs two things Unicode is the authority
for: which code points start and continue an identifier, and NFC. Working out
how many lines the part actually needed would be, as the rule requires: the
`XID` properties are about seven hundred ranges, and NFC is two thousand
decomposition mappings, nine hundred combining classes and an exclusion list.
Two hundred lines of algorithm and forty-five hundred lines of data.

The failure mode is blake3's. Two identifiers that should compare equal and do
not are a program that resolves one name to two bindings, quietly, and §3.3
already says so in as many words.

The Decay Rule settles the rest. `.decay-ceiling` counts the code every
guarantee rests on, and four and a half thousand lines of UCD is not that
code: it is not ours, we would not read it, and it changes once a year when
Unicode publishes. Putting it in the count would mean every future commit
measuring itself against a number that is mostly somebody else's data.
Generating it at build time moves the lines and not the problem.

So `unicode-ident` and `unicode-normalization` are permitted, the language
pins **Unicode 16.0**, and the rule the three of them share is worth stating
because "no dependencies" was never quite it:

> A dependency is admissible when what it holds is data, or a construction,
> that somebody else is the authority for. It is inadmissible when what it
> holds is behaviour this project could write and should understand.

`blake3` is the first kind. Unicode is the first kind. A markdown renderer, a
GIF encoder, an argument parser and a web framework are the second kind, which
is why this repository has written four of those and imported none.

### Fuel and `opaque` as command-line concerns

The obvious cheap answer: make the fuel budget a `--fuel` flag and the barrier a
`--no-inline` list, and keep both out of the language.

Rejected because a trace buried under a different budget is a different trace.
If the artifact depends on it, it is part of the artifact, and a thing that is
part of the artifact has to be visible in the source that produced it. A build
that succeeds on one machine and exhausts on another because somebody's shell
alias differed is precisely the class of failure this language exists to make
impossible.

The cost is a keyword. `opaque` is the only construct in Nether C that exists to
make the compiler do *less*, which is an odd thing to have to teach, and it will
be the first thing a newcomer mistakes for an optimisation hint.

### Depth as a monad stack

The obvious alternative to a lattice. Rejected because effects that compose by
`max` need no lifting, no transformers, and no ordering decisions, and because
`max` is a rule every programmer already holds in their head.

The cost is expressiveness: a lattice cannot distinguish *reads the disk* from
*reads the disk and the network* except by taking the deeper of the two. Nether
C accepts a coarser answer in exchange for one people will actually use.

### Monotonicity, pointing the other way

[§2.4](02-calculus.md#24-metatheory) once required `d′ ≥ d` and explained it
as *evaluation can only ever learn that something is deeper than it looked*.
It reads well and no implementation can satisfy it. A conditional's depth is
the maximum over its arms, and reducing it takes one arm and discards the
other, so a branch whose deep arm is not taken reduces to something shallower
than its own type says. The generative test in 0052 found it in forty-seven
programs.

**Keep `≥` and make the arms not count.** Give a conditional the depth of its
condition only, so reduction never lowers anything. Rejected outright: the arm
that is taken carries its own depth into the result, and a rule that drops it
hands back a disk-derived value typed as pure. That is laundering, and it is
the one thing the lattice is for.

**Keep `≥` and make reduction preserve the bound.** Have burial stamp the
unreduced expression's depth onto whatever the branch reduces to, so `1`
becomes `1@3`. The law holds and the language stops working: pure code that
happens to sit beside a deep arm no longer disappears at burial, and
everything downstream of it inherits a depth nothing ever reached.

**What it cost.** The shorter, more quotable sentence. `d′ ≤ d` is the
direction that carries the guarantee — a value never escapes at a depth its
type did not admit — and it costs the intuition that a depth is something
already reached. It is a bound, and the number that *was* reached lives in the
trace, which is now said explicitly in §2.4 because the two were being read as
one.

### Pretending an implementation has no limits

[§6.4](06-evaluation.md#64-starvation-and-fuel) said exhausting fuel was a
diagnostic and not a crash, and left it there. Fuel bounds steps, so a
recursion that never returns overflows whatever stack the implementation is
using long before the budget runs out, and a stack overflow is a crash. The
guarantee was true of the case it named and silent about the case that
actually happens.

**Bound depth with fuel.** Charge more of the budget per frame, so that a deep
program exhausts sooner. Rejected because it changes a constant and not a
shape: for any per-frame charge there is a budget large enough to reach any
depth, and picking one small enough to be safe makes fuel useless as a bound
on work.

**Say nothing and let implementations differ.** Rejected because §6.4 already
promises no crash, so saying nothing is not neutral — it is a promise no
recursive evaluator can keep.

**What it cost.** An admission: two implementations with different limits can
disagree about whether a program buries at all. The reproducibility claims in
this document survive because none of them was ever about that. They are about
what a trace *is* once it exists, and the sentence that says so is now written
down beside the admission rather than assumed.

### One number on an arrow

An arrow carried a single depth, and [ABS] took it from the body's value
depth. The consequence was that a descent could be written and not named:

```c
Bytes load(Str p) { descend disk { must(read(p)) } }

Bytes src = load("kernel.nc");   // rejected: dƒ was 3, δ was 0
```

while [§1.3](01-strata.md#13-descent) opens by accepting the identical
expression written out at the same ambient depth. There was no way to put a
descent behind a name, and the same rule failed in the other direction too: a
function whose deep work was deposited rather than returned had a body of
value depth 0, and so claimed a latent depth of 0 while touching the disk.

The decisive objection is about burial rather than taste. Burial reduces
applications, so under that rule an application could be rejected where its own
inlining was accepted, and the residue of a burial is supposed to be a program
that checks. Two ways to keep the single number were considered.

**Let the latent depth be what the body reaches, and let a descent inside a
body not relieve the caller.** A signature then reads as an audit: `U0 stamp()
@4` says it writes to the disk, and that is worth something. But a `descend`
inside a function bounds nothing under this rule, so every caller up the chain
holds what the leaf reached, and a program that reads one file writes `descend
disk` at every level between the two. Encapsulating a capability becomes
impossible, which is most of what a capability is for.

**Carry three numbers** — what it asks of the caller, what it hands back, and
what it reaches — the third purely so a signature stays an audit. Rejected
because the audit answer is already recorded somewhere better: a trace lists
every stratum actually reached, with a witness for each, and
`nether strata` reads it. A number in a signature that no rule consumes is a
comment with a syntax.

**What it cost.** The chosen design has two numbers and a signature is no
longer a complete account of what a function touches: `U0 stamp()` can write
to the disk and say nothing about it in its type, because it descends for
itself and hands back `U0`. What a program did is a question for the trace,
and what a program *needs from you* is the question the type answers. That
split is defensible and it is still a real loss for anyone reading signatures
to decide whether to call something.

### [APP]: the premise on all three terms

[APP] once required `max(dƒ, d_f, d_a) ≤ δ`, which reads well — *everything
about this application is within what you hold* — and forbids
[§6.2](06-evaluation.md#62-demand)'s own example, where `compile(src)` is
applied to a depth-3 value at ambient 0 and the comment beside it says the
call is pure and merely starves. [§9.2](09-prelude.md#92-depth-0) does the
same with `given(a)`, and [§2.5](02-calculus.md#25-subsumption-and-its-deliberate-absence)
says in prose that a shallow value combines with a deep one without coercion.

The stricter premise is not unsound; it is unusable. Under it a value carried
out of a descent can be sealed, shaded and compared, and cannot be passed to
`len`, so every use of a deep value has to happen inside the descent that
produced it and staging buys nothing. That is close to deleting
[§1.3](01-strata.md#13-descent)'s one-way-out property.

What replaced it is the narrowest premise that still stops the thing
capabilities exist to stop: `dƒ ≤ δ`. A capability is required to reach a
stratum, and the latent depth is the only term that reaches one.

### Ambient soundness: two ways to keep the simpler statement

[§2.4](02-calculus.md#24-metatheory) once said `d ≤ δ` outright, which is
shorter, easier to check and false: [DESCEND] carries a value out at the depth
it reached, and `Bytes@3 src = descend disk { read("kernel.nc") };` is depth 3
where δ is 0. Two ways to keep the shorter sentence were considered.

**Let a descent forget.** Conclude `descend κ {e} : τ@δ` instead of `τ@d`. The
invariant then holds by construction, and so does nothing else: it lowers the
depth of a value, which [§1.2](01-strata.md#12-the-monotonicity-law) forbids
outright, and it turns the one expression in the language whose purpose is to
reach the world into the one expression that launders what it found.

**Let a descent stain.** Keep δ raised for the remainder of the enclosing
scope, so that anything holding a depth-3 value really is at ambient 3. The
invariant holds, and the cost is that a capability's extent is no longer
visible in the braces: a `descend disk` on line 4 silently grants the disk to
line 90. This is the same re-staining already rejected for `look` earlier in
this section, and it is no better here.

What was kept instead is a longer true statement. The cost is that ambient
soundness now has a term in it that a reader has to evaluate — `g(e)`, the
deepest descent in the expression — where before it had two symbols. A rule
that fits on one page is worth defending; a rule that fits on one page and is
wrong is not.

### One range for all three integer spellings

Every integer literal could hold the same range, which is easier to state and
leaves `−2⁶³` unwritable: `-` is an operator applied to a literal, so the
literal it needs is `9223372036854775808`, which is one past the top of the
range. C has this wart and works around it with implementation-defined
promotion; a language with one integer type and no promotion has nowhere to
put the workaround.

The alternative considered was to let a decimal literal reach `2⁶³` — one
higher than the type — on the grounds that it is only ever going to be read
under a minus sign. Rejected because it makes a bare `9223372036854775808` a
literal that is in range as a literal and out of range as a value, which is a
distinction nothing else in the language makes and which every reader would
have to be told about once.

**What it cost.** Three spellings with two ranges between them, and a sentence
in §3.6 explaining why. The alternative is one range and a value that cannot
be written, and a type with a value nobody can write is a type with a hole in
it.

### Floating point

Excluded from [§3.6](03-lexical.md#36-literals) because IEEE 754 has
platform-observable behaviour, and canonical encoding cannot be built on top of
it. This makes Nether C unsuitable for numerical work today, which is a real
and large exclusion.

## 90.3 Things Nether C is worse at

Stated plainly, because the roadmap requires at least one such statement and
because they are all true.

- **Interactive programs.** A language whose central act happens before the
  program is delivered is the worst possible fit for anything that responds to
  a person. Nether C should not be used for a UI, a game, or a server that
  holds a connection.
- **Numerics.** No floats. See above.
- **Debugging by print.** Two commands instead of one, forever.
- **Storage.** The ledger never shrinks. §7.6 argues that this is affordable;
  it has not yet been measured, and until it is that argument is a hope.
- **Learning curve.** Depth is a genuinely new thing to learn. HolyC's whole
  proposition was that you already knew it — it was C, plus permission.

## 90.4 On TempleOS

Nether C is an inversion of HolyC in the way a photographic negative is an
inversion: every value reversed, every edge in exactly the same place.

Terry Davis built a complete operating system, a compiler, a graphics stack, a
document format and a language, alone, and made every one of them follow from a
single stated idea. Whatever one makes of the idea, the coherence is the
achievement, and it is rarer than the code.

He was ill, and it is in the work. TempleOS is strange because he was strange,
and it is also technically excellent, and both of those are true at once and
neither one cancels the other. Treating the strangeness as the whole story
misses an operating system. Treating it as an embarrassment to be edited out
misses the person who wrote it. This document tries to do neither.

What is borrowed here is the method rather than the belief: build the thing
yourself, keep it small enough to hold in your head, follow one idea all the
way down even when it becomes inconvenient, and say plainly what you think is
true.

An inversion is a form of close reading. You cannot turn something over without
first working out which way up it was.
