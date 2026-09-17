// tick.nc — a deposit made on every turn. spec/07-ledger.md §7.3.1.
//
// Four deposits of one value from one span. The `Deposit` is one node — same
// value, same span, so one content address — and the trace's list names it
// four times, because the list is what the program did and the node is what
// it said. `nether lamp` prints four lines.

U0 tick()
{
  for (I64 i = 0; i < 4; i += 1) { "tick\n"; }
}

demand tick();
