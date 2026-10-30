// owed.nc — a trace that owes the network. spec/07-ledger.md §7.3.2.
//
// The fetch is demanded only through a seal, and §1.5 makes a seal pure, so
// the residue is at depth 0 and the hole is at 5. The trace's depth is the
// join of both.
Bytes@5 page = must(descend net { get("https://example.invalid/x") });
Cairn   c    = seal page;
demand c;
