# The interpreter bootstrap

`interpreter.nc` and `evaluate.nc`, joined in that order, form one Nether C
compilation unit. Append a demand of `interpret(source_bytes)`. The source can
itself be supplied by `must(descend disk { read("program.nc") })`. The host
parses the interpreter; the guest source remains Bytes throughout.

The bootstrap settles 0079's sample-program proof:

- Unchanged `hello.nc` resolves its demanded function and deposits the greeting
  as a native Str.
- Unchanged `build.nc` resolves globals and parameters, leaves a native
  `read("main.nc")` hole, and returns `obj:` followed by the answer. Unused
  bindings do not ask the world anything.
- Unchanged `stamp.nc` fails checking at its illegal `look`, with origin 5,
  ambient 0 and the guest byte offset. Checking unused declarations does not
  evaluate them. Its undemanded `seal` is legal.

`crates/nether-bury/tests/interpreter.rs` proves these facts and exercises
renaming, forward declarations, comments, type/arity/depth errors, UTF-8,
refusals and legal looks. A source hole can be answered before the guest's
file hole; the resulting residue prints, parses and resumes to the same
result as burial with both answers supplied from the start.

## Representation

The bootstrap burier does not yet reduce aggregate projections. Interpreter
values therefore use length-prefixed Bytes, not host data structures. A packet
is its payload length in decimal, a colon, then those exact bytes. A value is
`packet(type) + packet(depth) + payload`; Shade and Answer payloads contain a
value. Environments contain alternating name and value packets. Payloads can
contain colons, zero bytes and arbitrary answer bytes without ambiguity.

`interpret` returns a packet per demanded value in source order. A semantic
failure is an Error value carrying a diagnostic and guest byte offset. Syntax
failures collapse through an invalid slice. Expression statements deposit
native Str, Bytes or I64 values; U0 deposits nothing. A refused `must` becomes
an Error, never an empty successful value.

Checking is a separate pass using typed placeholder answers. It validates all
declarations before evaluation and never makes a prelude request or deposit.
Evaluation dispatches read/get through the real prelude. The host supplies
fuel, records the actual questions and answers, and residualises starvation.
It does not parse guest syntax or choose guest bindings.

## Boundary

This is the sample-program bootstrap, not a complete replacement for the Rust
frontend or an interpreter capable of interpreting itself. Its grammar covers
ASCII identifiers, comments, Str/Bytes literals, named calls with typed
parameters, globals, local initialisers, expression statements, returns, block
tails, demands, descend, shade and look. It checks U0, Str, Bytes, I64, Bool,
Cairn, Shade and Answer types and explicit outer depth assertions. Its prelude
dispatch covers concat, len, raw, must, read and get.

General operators, control flow, aggregates, Unicode identifiers, numeric
literals, full escape syntax, recursive checking, inferred latent signatures,
memoised guest globals and guest-level fuel accounting are not implemented.
Latent signatures must currently be explicit when a function calls the world
without descending itself. Seal is checked but a demanded seal returns an
explicit unsupported-operation Error; the bootstrap never invents a cairn.
Only the listed native deposit types are supported. These boundaries must not
be confused with completion of the later self-burial projections.

The proof harness grants 100 million host expression steps per burial. Host
call-frame and allocation limits apply too. Exhausting either is a diagnostic,
not evidence that a guest program is valid or has completed.
