// interpreter.nc — the first stratum of the interpreter, written in Nether C.
//
// The source is Bytes because the interpreter must be able to receive the
// program from a disk hole. There is deliberately no host parser hiding behind
// this file: every inspection below is a slice and a comparison the language
// itself can bury. evaluate.nc supplies values, checking and demand evaluation.

Bool at(Bytes source, I64 from, Bytes needle)
{
  if (from < 0 || from + len(needle) > len(source)) {
    return false;
  } else {
    return starts_with(slice(source, from, len(source)), needle);
  }
}

Bool white(Bytes source, I64 from)
{
  at(source, from, b" ") || at(source, from, b"\t")
    || at(source, from, b"\n") || at(source, from, b"\r")
}

// Skip the whitespace and comments the lexical specification permits.
I64 skip(Bytes source, I64 from)
{
  if (from >= len(source)) {
    return from;
  } else if (white(source, from)) {
    return skip(source, from + 1);
  } else if (at(source, from, b"//")) {
    return skip_line(source, from + 2);
  } else if (at(source, from, b"/*")) {
    return skip_block(source, from + 2);
  } else {
    return from;
  }
}

I64 skip_line(Bytes source, I64 from)
{
  if (from >= len(source)) {
    return len(source);
  } else if (at(source, from, b"\n")) {
    return skip(source, from + 1);
  } else {
    return skip_line(source, from + 1);
  }
}

// Malformed syntax collapses through an invalid slice. Semantic errors have
// typed records in evaluate.nc; a syntax failure must not become a valid EOF.
I64 malformed_source(Bytes source)
{
  return len(slice(source, -1, 0));
}

I64 skip_block(Bytes source, I64 from)
{
  if (from + 1 >= len(source)) {
    return malformed_source(source);
  } else if (at(source, from, b"*/")) {
    return skip(source, from + 2);
  } else {
    return skip_block(source, from + 1);
  }
}

// The first occurrence of needle at or after from, or len(source) when it is
// absent. It is the only search primitive the parser needs.
I64 seek(Bytes source, I64 from, Bytes needle)
{
  if (from + len(needle) > len(source)) {
    return len(source);
  } else if (at(source, from, needle)) {
    return from;
  } else {
    return seek(source, from + 1, needle);
  }
}

// Decode the escape forms used by the sample programs. The lexer has already
// found both quote positions; an unclosed literal therefore remains a parser
// failure rather than silently becoming an empty value.
Bytes unescape(Bytes source, I64 from, I64 until)
{
  if (from >= until) {
    return b"";
  } else if (at(source, from, b"\\n")) {
    return concat(b"\n", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\r")) {
    return concat(b"\r", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\t")) {
    return concat(b"\t", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\\"")) {
    return concat(b"\"", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\\\")) {
    return concat(b"\\", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\0")) {
    return concat(b"\0", unescape(source, from + 2, until));
  } else if (at(source, from, b"\\")) {
    return slice(source, -1, 0);
  } else {
    return concat(slice(source, from, from + 1), unescape(source, from + 1, until));
  }
}

// The closing quote is not the first quote after the opener: a quoted escape
// belongs to the literal. This is kept beside unescape so the two walks make
// the same decision about a backslash.
I64 quote_end(Bytes source, I64 from)
{
  if (from >= len(source)) {
    return malformed_source(source);
  } else if (at(source, from, b"\n")) {
    return malformed_source(source);
  } else if (at(source, from, b"\\")) {
    return quote_end(source, from + 2);
  } else if (at(source, from, b"\"")) {
    return from;
  } else {
    return quote_end(source, from + 1);
  }
}

// A quoted text literal at `from`, returned as Bytes so that the evaluator can
// carry it before deciding whether the program asked to deposit or return it.
Bytes text(Bytes source, I64 from)
{
  I64 start = skip(source, from);
  if (!at(source, start, b"\"")) {
    return slice(source, -1, 0);
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
  if (letter(source, p) || digit(source, p)) {
    return word_end(source, p + 1);
  }
  return p;
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
