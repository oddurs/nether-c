# The trace browser

The page buries source locally with `nether-wasm`. It has no world capabilities
and sends no source to a server. Build with `scripts/task wasm`, then serve the
repository with `scripts/task serve` and open `/web/necropolis/`.

## Data boundary

A successful burial includes `objects`: pairs of full cairn and lowercase hex
canonical encoding, sorted by cairn and deduplicated. These are the objects
evaluation named, plus the source, printed residue and trace envelope. The
encoding is [the ledger format](../../spec/07-ledger.md), not a new JSON value
format. Signed integers, zero bytes and invalid UTF-8 byte values are lossless.
Existing summary fields remain available. A diagnostic has no partial graph.

Only references actually recorded in objects are navigable. The trace is not
an AST or a recording of every arithmetic reduction. Source locations are
UTF-8 byte ranges into the named source, not JavaScript string offsets. A
missing object must remain visibly absent, not become an invented value.

This transport comes directly from the local WASM module. It is not a ledger
import API; accepting untrusted external objects would also require verifying
their cairns with the existing Rust implementation, not handwritten JavaScript
cryptography. Canonical bytes avoid a second Rust serializer for every node and
value variant, at the cost of hex transport overhead and a small browser decoder.

## Navigation and limits

The initial example buries automatically. Open a hole or deposit, follow its
labelled references, and use Back, Forward or Return to trace. Incoming edges
mean “references this object”, not “produced this value”; the provenance walk
is separate work. Shades show their origin and cairn, never an unwrapped value.

Source can be edited in the disclosure above the trace. Each burial uses a
fresh module worker; restarting or cancelling terminates the old worker.
Diagnostics retain and explicitly label the previous successful trace.

Browser limits: 256 KiB source, the default one-million-step fuel budget,
128 evaluation frames, an 8 MiB module stack, and a 30-second worker deadline.
The native burier retains its 2,048 frames and 64 MiB thread stack. Browser
burial cannot spawn native threads: `.cargo/config.toml` supplies the module
stack, and real-WASM tests verify the frame diagnostic instead of only testing
the native build of `nether-wasm`.

The view accepts at most 20,000 objects / 16 MiB canonical bytes. Relations
start with twelve entries per side. Value previews are bounded; compound
decoding stops at 4,096 entries or 128 nesting levels and labels the preview
unavailable. The bytes remain in the local result; these are display limits,
not invented values or modified cairns.

Tests require Node 22 or newer, using only its standard library.
`scripts/task graph:check` checks decoding and navigation; `scripts/task wasm`
also instantiates the actual built module, buries real samples, and exercises
limits and repeated burials. Both are required by `scripts/task check`.

`scripts/task browser:check` uses an installed Chrome or Chromium via its
debugging pipe, with a temporary profile and a loopback-only static server.
Set `NETHER_CHROME` if it is not at a standard location. No driver package is
installed. This check is required, not silently skipped, by `scripts/task check`
after the WASM build. CI's hosted Ubuntu runner supplies Chrome and Node.
`NETHER_SCREENSHOTS` optionally names a directory for desktop and phone PNGs.

Browser checks cover worker burial, hole and deposit navigation, source spans,
keyboard history, a 390px viewport, hostile text, cancel/restart, empty traces,
module-load failure and recovery. Graph links are HTML buttons; the SVG is
decorative and disappears on small screens without removing navigation.

## Human proof still required

Item 75's proof is not an automated test. Give someone unfamiliar with Nether C
the default page, without coaching, and ask them to find what the program is
waiting for, inspect the question, follow an argument and return to the trace.
Record where they hesitate and whether they complete the navigation unaided.
Only that observation can close 75. The five-person comprehension study remains
the later, separate item 78; this browser does not claim to settle it.
