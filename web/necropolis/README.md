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
