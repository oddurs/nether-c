// interpreter.nc — the first stratum of the interpreter, written in Nether C.
//
// The source is Bytes because the interpreter must be able to receive the
// program from a disk hole. There is deliberately no host parser hiding behind
// this file: every inspection below is a slice and a comparison the language
// itself can bury. value.nc owns records; evaluate.nc owns language semantics.

Bool at(Bytes source, I64 from, Bytes needle)
{
  if (from < 0 || from > len(source) - len(needle)) { return false; }
  return slice(source, from, from + len(needle)) == needle;
}

Bool white(Bytes source, I64 from)
{
  at(source, from, b" ") || at(source, from, b"\t")
    || at(source, from, b"\n") || at(source, from, b"\r")
}

// Skip the whitespace and comments the lexical specification permits.
I64 skip(Bytes source, I64 from)
{
  I64 p = from;
  // Pass `p + 0` rather than `p` so §5.4 does not freeze mutable loop cursors.
  while (p < len(source)) {
    if (white(source, p + 0)) {
      p += 1;
    } else if (at(source, p + 0, b"//")) {
      p = skip_line(source, p + 2);
    } else if (at(source, p + 0, b"/*")) {
      p = skip_block(source, p + 2);
    } else {
      return p + 0;
    }
  }
  return p + 0;
}

I64 skip_line(Bytes source, I64 from)
{
  I64 p = from;
  while (p < len(source)) {
    if (at(source, p + 0, b"\n")) { return p + 1; }
    p += 1;
  }
  return len(source);
}

// Malformed syntax collapses through an invalid slice. Semantic errors have
// typed records in evaluate.nc; a syntax failure must not become a valid EOF.
Bytes malformed_bytes(Bytes source)
{
  return slice(source, -1, 0);
}
I64 malformed_source(Bytes source)
{
  return len(malformed_bytes(source));
}

I64 skip_block(Bytes source, I64 from)
{
  I64 p = from;
  while (p + 1 < len(source)) {
    if (at(source, p + 0, b"*/")) { return p + 2; }
    p += 1;
  }
  return malformed_source(source);
}

// The first occurrence of needle at or after from, or len(source) when it is
// absent. It is the only search primitive the parser needs.
I64 seek(Bytes source, I64 from, Bytes needle)
{
  I64 p = from;
  I64 limit = len(source) - len(needle);
  while (p <= limit) {
    if (at(source, p + 0, needle)) { return p + 0; }
    p += 1;
  }
  return len(source);
}

// Advance across a contiguous non-escape chunk inside a quoted literal.
I64 plain_end(Bytes source, I64 from, I64 until)
{
  I64 p = from;
  while (p < until) {
    if (at(source, p + 0, b"\\")) { return p + 0; }
    p += 1;
  }
  return until;
}

// Count backslash escapes in [from, until) using only integer locals.
I64 escape_count(Bytes source, I64 from, I64 until)
{
  I64 n = 0;
  I64 cur = from;
  while (cur < until) {
    I64 cut = plain_end(source, cur + 0, until);
    if (cut >= until) { return n + 0; }
    n += 1;
    cur = cut + 2;
  }
  return n + 0;
}

// Byte offset of the `target`-th backslash escape (0-indexed) in [from, until).
I64 nth_escape(Bytes source, I64 from, I64 until, I64 target)
{
  I64 n = 0;
  I64 cur = from;
  while (cur < until) {
    I64 cut = plain_end(source, cur + 0, until);
    if (cut >= until) { return until; }
    if (n == target) { return cut; }
    n += 1;
    cur = cut + 2;
  }
  return until;
}

// Decode the escape forms used by the sample programs. Uses balanced
// divide-and-conquer so literals with thousands of escapes use O(log N)
// call frames and stay within WASM's frame limit.
Bytes unescape(Bytes source, I64 from, I64 until)
{
  if (from >= until) { return b""; }
  I64 cut = plain_end(source, from, until);
  if (cut >= until) { return slice(source, from, until); }
  I64 n = escape_count(source, cut, until);
  // Split once a literal has > 8 escapes to keep recursion depth O(log N).
  if (n > 8) {
    I64 mid = nth_escape(source, cut, until, n / 2);
    return concat(unescape(source, from, mid), unescape(source, mid, until));
  }
  Bytes head_chunk = slice(source, from, cut);
  Bytes esc = b"";
  if (at(source, cut, b"\\n")) { esc = b"\n"; }
  else if (at(source, cut, b"\\r")) { esc = b"\r"; }
  else if (at(source, cut, b"\\t")) { esc = b"\t"; }
  else if (at(source, cut, b"\\\"")) { esc = b"\""; }
  else if (at(source, cut, b"\\\\")) { esc = b"\\"; }
  else if (at(source, cut, b"\\0")) { esc = b"\0"; }
  else { return malformed_bytes(source); }
  return concat(head_chunk, concat(esc, unescape(source, cut + 2, until)));
}

// The closing quote is not the first quote after the opener: a quoted escape
// belongs to the literal. This is kept beside unescape so the two walks make
// the same decision about a backslash.
I64 quote_end(Bytes source, I64 from)
{
  I64 p = from;
  while (p < len(source)) {
    if (at(source, p + 0, b"\n") || at(source, p + 0, b"\r")) {
      return malformed_source(source);
    } else if (at(source, p + 0, b"\\")) {
      if (p + 1 >= len(source) || at(source, p + 1, b"\n") || at(source, p + 1, b"\r")) {
        return malformed_source(source);
      }
      p += 2;
    } else if (at(source, p + 0, b"\"")) {
      return p + 0;
    } else {
      p += 1;
    }
  }
  return malformed_source(source);
}

// A quoted text literal at `from`, returned as Bytes so that the evaluator can
// carry it before deciding whether the program asked to deposit or return it.
Bytes text(Bytes source, I64 from)
{
  I64 start = skip(source, from);
  if (!at(source, start, b"\"")) {
    return malformed_bytes(source);
  }
  I64 end = quote_end(source, start + 1);
  return unescape(source, start + 1, end);
}

// Lexical entry point, shared by clients inspecting the source cursor.
I64 source_start(Bytes source)
{
  return skip(source, 0);
}

// Identifiers in the bootstrap grammar use ASCII letters, digits and '_'.
Bool letter(Bytes source, I64 p)
{
  if (p >= len(source)) { return false; }
  Bytes alphabet = b"abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ_";
  return seek(alphabet, 0, slice(source, p, p + 1)) < len(alphabet);
}

Bool digit(Bytes source, I64 p)
{
  if (p >= len(source)) { return false; }
  return seek(b"0123456789", 0, slice(source, p, p + 1)) < 10;
}

I64 word_end(Bytes source, I64 p)
{
  I64 q = p;
  while (letter(source, q + 0) || digit(source, q + 0)) { q += 1; }
  return q + 0;
}

I64 token_end(Bytes source, I64 p)
{
  I64 start = skip(source, p);
  if (start >= len(source)) { return start; }
  if (at(source, start, b"b\"")) { return quote_end(source, start + 2) + 1; }
  if (at(source, start, b"\"")) { return quote_end(source, start + 1) + 1; }
  if (letter(source, start)) { return word_end(source, start + 1); }
  return start + 1;
}

Bytes token(Bytes source, I64 p)
{
  return slice(source, skip(source, p), token_end(source, p));
}

I64 expect(Bytes source, I64 p, Bytes wanted)
{
  if (token(source, p) != wanted) { return malformed_source(source); }
  return token_end(source, p);
}

I64 name_end(Bytes source, I64 p)
{
  I64 start = skip(source, p);
  if (!letter(source, start)) { return malformed_source(source); }
  return word_end(source, start + 1);
}
