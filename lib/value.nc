// The interpreter's records. No language decisions belong in this file.
// A packet is decimal byte length, ':', then that many uninterpreted bytes.
// The bootstrap uses Bytes until the burier can reduce aggregate projections.

Bytes decimal(I64 n)
{
  if (n < -9) {
    I64 last = -(n % 10);
    return concat(decimal(n / 10), slice(b"0123456789", last, last + 1));
  }
  if (n < 0) { return concat(b"-", decimal(-n)); }
  if (n < 10) { return slice(b"0123456789", n, n + 1); }
  return concat(decimal(n / 10), slice(b"0123456789", n % 10, n % 10 + 1));
}

// Accumulate negatively: I64's minimum has no positive counterpart.
// Check before multiplying, so wrapping arithmetic cannot forge a length.
I64 number_from(Bytes s, I64 p, I64 n)
{
  if (p == len(s)) { return n; }
  I64 d = seek(b"0123456789", 0, slice(s, p, p + 1));
  if (d == 10) { return malformed_source(s); }
  if (n < -922337203685477580 || (n == -922337203685477580 && d > 8)) {
    return malformed_source(s);
  }
  return number_from(s, p + 1, n * 10 - d);
}
I64 number(Bytes s)
{
  Bool negative = starts_with(s, b"-");
  I64 start = 0;
  if (negative) { start = 1; }
  if (start == len(s)) { return malformed_source(s); }
  I64 n = number_from(s, start, 0);
  if (negative) { return n; }
  if (n < -9223372036854775807) { return malformed_source(s); }
  return -n;
}

Bytes packet(Bytes s) { concat(decimal(len(s)), concat(b":", s)) }
I64 packet_start(Bytes s) { seek(s, 0, b":") + 1 }
I64 packet_end(Bytes s) { packet_start(s) + number(slice(s, 0, packet_start(s) - 1)) }
Bytes head(Bytes s) { slice(s, packet_start(s), packet_end(s)) }
Bytes rest(Bytes s) { slice(s, packet_end(s), len(s)) }

// Value = packet(type), packet(depth), payload. Only wrappers have an inner
// value: a Shade preserves its origin; an Answer contains a value or Refusal.
Bytes value(Bytes t, I64 d, Bytes payload)
{
  return concat(packet(t), concat(packet(decimal(d)), payload));
}
Bytes kind(Bytes v) { head(v) }
I64 depth(Bytes v) { number(head(rest(v))) }
Bytes data(Bytes v) { rest(rest(v)) }
Bool failed(Bytes v) { kind(v) == b"Error" }
Bytes unit_value() { value(b"U0", 0, b"") }
Bytes deepen(Bytes v, I64 d) { value(kind(v), max(depth(v), d), data(v)) }
Bytes problem(Bytes message, I64 p)
{
  return value(b"Error", 0, concat(message, concat(b" at byte ", decimal(p))));
}

// Tables are alternating name/value packets, newest binding first. The empty
// byte string means absent; every stored value, including U0, has an encoding.
Bytes bind(Bytes table, Bytes name, Bytes v)
{
  return concat(packet(name), concat(packet(v), table));
}
Bytes lookup(Bytes table, Bytes name)
{
  if (len(table) == 0) { return b""; }
  if (head(table) == name) { return head(rest(table)); }
  return lookup(rest(rest(table)), name);
}

// Memory = packet(declaration table), global-value table. Each pass starts
// with the same declarations and its own empty cache. Locals never go here.
Bytes remember(Bytes memory, Bytes name, Bytes v)
{
  return concat(packet(head(memory)), bind(rest(memory), name, v));
}
I64 location(Bytes memory, Bytes name)
{
  Bytes offset = lookup(head(memory), name);
  if (len(offset) == 0) { return -1; }
  return number(offset);
}

// Result = packet(cursor), packet(value), memory. Every evaluation returns
// its updated memory; a caller cannot lose a completed global by leaving scope.
Bytes result(I64 p, Bytes v, Bytes memory)
{
  return concat(packet(decimal(p)), concat(packet(v), memory));
}
I64 cursor(Bytes r) { number(head(r)) }
Bytes found(Bytes r) { head(rest(r)) }
Bytes memory_of(Bytes r) { rest(rest(r)) }
Bytes relocate(Bytes r, I64 p) { result(p, found(r), memory_of(r)) }
Bytes replace_value(Bytes r, Bytes v) { result(cursor(r), v, memory_of(r)) }
