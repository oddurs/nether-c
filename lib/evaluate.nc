// The language layer. The cursor reads syntax; value.nc owns records.
// Checking and evaluation share traversal, but never memory or world effects.

// Structural scans consume balanced groups, treating strings and comments as
// indivisible tokens. An unexpected closing delimiter is never skipped.
I64 group_end(Bytes s, I64 p, Bytes close)
{
  Bytes t = token(s, p);
  I64 next = token_end(s, p);
  if (t == close) { return next; }
  if (t == b"" || t == b")" || t == b"}" || t == b"]") {
    return malformed_source(s);
  }
  if (t == b"(") { return group_end(s, group_end(s, next, b")"), close); }
  if (t == b"{") { return group_end(s, group_end(s, next, b"}"), close); }
  if (t == b"[") { return group_end(s, group_end(s, next, b"]"), close); }
  return group_end(s, next, close);
}

I64 type_end(Bytes s, I64 p)
{
  return type_suffix(s, type_body_end(s, p));
}
I64 type_body_end(Bytes s, I64 p)
{
  I64 next = name_end(s, p);
  if (token(s, next) == b"<") {
    I64 inner = type_end(s, token_end(s, next));
    return expect(s, inner, b">");
  }
  return next;
}
I64 type_suffix(Bytes s, I64 p)
{
  if (token(s, p) != b"@") { return p; }
  I64 start = skip(s, token_end(s, p));
  if (!digit(s, start) || number(token(s, start)) > 8) {
    return malformed_source(s);
  }
  return token_end(s, start);
}
I64 declaration_end(Bytes s, I64 p)
{
  if (token(s, p) == b"demand") { return group_end(s, token_end(s, p), b";"); }
  I64 after_name = name_end(s, type_end(s, p));
  if (token(s, after_name) == b"(") {
    I64 after_params = group_end(s, token_end(s, after_name), b")");
    I64 body = expect(s, type_suffix(s, after_params), b"{");
    return group_end(s, body, b"}");
  }
  return group_end(s, expect(s, after_name, b"="), b";");
}

// Build the name index before checking: forward references and duplicate
// declarations are properties of the unit, not accidents of demand order.
Bytes index_program(Bytes s, I64 p, Bytes names)
{
  I64 start = skip(s, p);
  if (start >= len(s)) { return value(b"Index", 0, names); }
  I64 end = declaration_end(s, start);
  if (token(s, start) == b"demand") { return index_program(s, end, names); }
  I64 at_name = type_end(s, start);
  Bytes name = token(s, at_name);
  if (len(lookup(names, name)) > 0) {
    return problem(concat(b"duplicate declaration: ", name), skip(s, at_name));
  }
  return index_program(s, end, bind(names, name, decimal(start)));
}

I64 capability(Bytes t)
{
  if (t == b"store") { return 1; }
  if (t == b"env") { return 2; }
  if (t == b"disk") { return 3; }
  if (t == b"net") { return 5; }
  if (t == b"entropy") { return 7; }
  if (t == b"unrecorded") { return 8; }
  return -1;
}
Bool type_word(Bytes t)
{
  return t == b"U0" || t == b"Bytes" || t == b"Str" || t == b"I64"
    || t == b"Shade" || t == b"Answer" || t == b"Cairn" || t == b"Bool";
}
I64 annotation(Bytes s, I64 p)
{
  I64 next = type_body_end(s, p);
  if (token(s, next) == b"@") { return number(token(s, token_end(s, next))); }
  return -1;
}
Bytes declared_value(Bytes s, I64 p)
{
  Bytes t = token(s, p);
  if (!type_word(t)) { return problem(b"unsupported type", p); }
  I64 d = max(0, annotation(s, p));
  if (t == b"Shade" || t == b"Answer") {
    Bytes inner = declared_value(s, expect(s, token_end(s, p), b"<"));
    if (failed(inner)) { return inner; }
    return value(t, d, inner);
  }
  return value(t, d, b"");
}
Bool same_type(Bytes a, Bytes b)
{
  if (kind(a) != kind(b)) { return false; }
  // Refusal is a valid inhabitant of every Answer<T>, not a new result type.
  if (kind(a) == b"Answer" && kind(data(b)) == b"Refusal") { return true; }
  if (kind(a) == b"Shade" || kind(a) == b"Answer") {
    return same_type(data(a), data(b));
  }
  return true;
}
Bytes check_binding(Bytes s, I64 p, Bytes v)
{
  if (failed(v)) { return v; }
  Bytes want = declared_value(s, p);
  if (failed(want)) { return want; }
  if (!same_type(want, v)) { return problem(b"binding type mismatch", p); }
  I64 asserted = annotation(s, p);
  if (asserted >= 0 && asserted != depth(v)) {
    return problem(b"depth assertion mismatch", p);
  }
  return v;
}

// A global moves exactly once: absent -> Pending -> value. Pending is a
// black hole, not a guessed value; a dependency cycle names the binding.
// Global initializers have no caller locals and always begin at ambient zero.
Bytes global_value(Bytes s, Bytes name, I64 p, Bytes memory, Bool checking)
{
  Bytes cached = lookup(rest(memory), name);
  if (len(cached) > 0) {
    if (kind(cached) == b"Pending") {
      return result(p, problem(concat(b"cyclic global: ", name), p), memory);
    }
    return result(p, cached, memory);
  }
  I64 global = location(memory, name);
  if (global < 0) {
    return result(p, problem(concat(b"unknown binding: ", name), p), memory);
  }
  I64 after = name_end(s, type_end(s, global));
  if (token(s, after) != b"=") {
    return result(p, problem(b"function requires a call", p), memory);
  }
  Bytes pending = remember(memory, name, value(b"Pending", 0, b""));
  Bytes r = expression(s, token_end(s, after), b"", 0, pending, checking);
  Bytes v = check_binding(s, global, found(r));
  return result(p, v, remember(memory_of(r), name, v));
}

// Every expression takes a lexical environment and the latest memory, and
// returns its cursor, value and successor memory. Only block statements may
// deposit; only prelude dispatch may ask the world.
Bytes expression(Bytes s, I64 p, Bytes env, I64 ambient, Bytes memory, Bool checking)
{
  I64 start = skip(s, p);
  I64 next = token_end(s, start);
  Bytes t = token(s, start);
  if (starts_with(t, b"b\"")) {
    return result(next, value(b"Bytes", 0, text(s, start + 1)), memory);
  }
  if (starts_with(t, b"\"")) {
    return result(next, value(b"Str", 0, text(s, start)), memory);
  }
  if (t == b"(") {
    Bytes r = expression(s, next, env, ambient, memory, checking);
    return relocate(r, expect(s, cursor(r), b")"));
  }
  if (t == b"descend") {
    I64 d = capability(token(s, next));
    if (d < 0) { return result(next, problem(b"unknown capability", next), memory); }
    I64 body = expect(s, token_end(s, next), b"{");
    return block_value(s, body, env, max(ambient, d), memory, checking);
  }
  if (t == b"shade" || t == b"look" || t == b"seal") {
    Bytes r = expression(s, next, env, ambient, memory, checking);
    return replace_value(r, interpret_rite(t, found(r), ambient, checking, start));
  }
  if (token(s, next) == b"(") {
    Bytes args = arguments(s, token_end(s, next), env, ambient, memory, checking);
    if (len(lookup(env, t)) > 0) {
      return replace_value(args, problem(b"binding is not callable", start));
    }
    Bytes r = invoke(s, t, found(args), ambient, memory_of(args), checking, start);
    return relocate(r, cursor(args));
  }
  Bytes local = lookup(env, t);
  if (len(local) > 0) { return result(next, local, memory); }
  return relocate(global_value(s, t, start, memory, checking), next);
}

Bytes interpret_rite(Bytes rite, Bytes v, I64 ambient, Bool checking, I64 p)
{
  if (failed(v)) { return v; }
  if (rite == b"shade") { return value(b"Shade", 0, v); }
  if (rite == b"seal") {
    if (checking) { return value(b"Cairn", 0, b""); }
    return problem(b"runtime seal is not supported by this bootstrap", p);
  }
  if (kind(v) != b"Shade") { return problem(b"look requires a Shade", p); }
  I64 origin = depth(data(v));
  if (origin > ambient) {
    Bytes message = concat(b"Orpheus: origin ", decimal(origin));
    return problem(concat(message, concat(b", ambient ", decimal(ambient))), p);
  }
  return deepen(data(v), depth(v));
}

Bytes arguments(Bytes s, I64 p, Bytes env, I64 ambient, Bytes memory, Bool checking)
{
  if (token(s, p) == b")") { return result(token_end(s, p), b"", memory); }
  Bytes r = expression(s, p, env, ambient, memory, checking);
  if (failed(found(r))) {
    return result(group_end(s, cursor(r), b")"), packet(found(r)), memory_of(r));
  }
  if (token(s, cursor(r)) == b")") {
    return result(token_end(s, cursor(r)), packet(found(r)), memory_of(r));
  }
  I64 next = expect(s, cursor(r), b",");
  if (token(s, next) == b")") { return result(malformed_source(s), found(r), memory_of(r)); }
  Bytes tail = arguments(s, next, env, ambient, memory_of(r), checking);
  return replace_value(tail, concat(packet(found(r)), found(tail)));
}
Bytes argument_error(Bytes args)
{
  if (len(args) == 0) { return b""; }
  if (failed(head(args))) { return head(args); }
  return argument_error(rest(args));
}
I64 argument_count(Bytes args)
{
  if (len(args) == 0) { return 0; }
  return 1 + argument_count(rest(args));
}
I64 argument_depth(Bytes args)
{
  if (len(args) == 0) { return 0; }
  return max(depth(head(args)), argument_depth(rest(args)));
}

Bytes parameters(Bytes s, I64 p, Bytes args, Bytes env, Bytes memory)
{
  if (token(s, p) == b")") {
    if (len(args) != 0) { return result(p, problem(b"too many arguments", p), memory); }
    return result(token_end(s, p), value(b"Environment", 0, env), memory);
  }
  if (len(args) == 0) { return result(p, problem(b"too few arguments", p), memory); }
  Bytes v = check_binding(s, p, head(args));
  if (failed(v)) { return result(p, v, memory); }
  I64 at_name = type_end(s, p);
  Bytes name = token(s, at_name);
  if (len(lookup(env, name)) > 0) {
    return result(p, problem(concat(b"duplicate parameter: ", name), p), memory);
  }
  I64 next = name_end(s, at_name);
  Bytes bound = bind(env, name, v);
  if (token(s, next) == b")") { return parameters(s, next, rest(args), bound, memory); }
  return parameters(s, expect(s, next, b","), rest(args), bound, memory);
}

Bytes invoke(Bytes s, Bytes name, Bytes args, I64 ambient,
             Bytes memory, Bool checking, I64 p)
{
  Bytes bad = argument_error(args);
  if (len(bad) > 0) { return result(p, bad, memory); }
  I64 f = location(memory, name);
  if (f < 0) { return result(p, prelude(name, args, ambient, checking, p), memory); }
  I64 after_name = name_end(s, type_end(s, f));
  if (token(s, after_name) != b"(") {
    return result(p, problem(b"binding is not callable", p), memory);
  }
  I64 start = token_end(s, after_name);
  Bytes bound = parameters(s, start, args, b"", memory);
  if (failed(found(bound))) { return bound; }
  I64 after = cursor(bound);
  if (function_ambient(s, after) > ambient) {
    return result(p, problem(b"call requires a deeper ambient stratum", p), memory);
  }
  I64 body = expect(s, type_suffix(s, after), b"{");
  Bytes r = block_value(s, body, data(found(bound)), ambient, memory, checking);
  Bytes v = check_binding(s, f, found(r));
  if (failed(v)) { return replace_value(r, v); }
  return replace_value(r, deepen(v, argument_depth(args)));
}

// Prelude operations have values, not source cursors or lexical environments.
// read/get are the only world boundary; their arguments are already values.
Bytes prelude(Bytes name, Bytes args, I64 ambient, Bool checking, I64 p)
{
  I64 count = argument_count(args);
  if (name == b"concat") {
    if (count != 2) { return problem(b"concat arity", p); }
    Bytes a = head(args);
    Bytes b = head(rest(args));
    if (kind(a) != b"Bytes" || kind(b) != b"Bytes") {
      return problem(b"concat requires Bytes", p);
    }
    return value(b"Bytes", max(depth(a), depth(b)), concat(data(a), data(b)));
  }
  if (count != 1) { return problem(b"unknown call or wrong arity", p); }
  Bytes a = head(args);
  if (name == b"must") {
    if (kind(a) != b"Answer") { return problem(b"must requires Answer", p); }
    if (kind(data(a)) == b"Refusal") { return problem(b"must: world refused", p); }
    return deepen(data(a), depth(a));
  }
  if (name == b"len") {
    if (kind(a) != b"Bytes" && kind(a) != b"Str") {
      return problem(b"len requires Bytes or Str", p);
    }
    return value(b"I64", depth(a), decimal(len(data(a))));
  }
  if (name == b"raw") {
    if (kind(a) != b"Str") { return problem(b"raw requires Str", p); }
    return value(b"Bytes", depth(a), data(a));
  }
  if (name == b"read" || name == b"get") {
    if (kind(a) != b"Str") { return problem(b"request requires Str", p); }
    I64 d = request_depth(name);
    if (ambient < d) { return problem(b"request requires descend", p); }
    if (checking) {
      return value(b"Answer", max(d, depth(a)), value(b"Bytes", d, b""));
    }
    return deepen(world_request(name, data(a)), depth(a));
  }
  return problem(concat(b"unsupported prelude function: ", name), p);
}
I64 request_depth(Bytes name)
{
  if (name == b"read") { return 3; }
  return 5;
}
Bytes world_request(Bytes name, Bytes path)
{
  if (name == b"read") {
    Answer<Bytes> a = descend disk { read(must(utf8(path))) };
    return answered_value(a, 3);
  }
  Answer<Bytes> a = descend net { get(must(utf8(path))) };
  return answered_value(a, 5);
}
Bytes answered_value(Answer<Bytes> a, I64 d)
{
  if (given(a)) { return value(b"Answer", d, value(b"Bytes", d, must(a))); }
  return value(b"Answer", d, value(b"Refusal", d, b"refused"));
}
Bool depositable(Bytes v)
{
  Bytes t = kind(v);
  return t == b"U0" || t == b"Str" || t == b"Bytes" || t == b"I64";
}
U0 deposit_value(Bytes v)
{
  if (kind(v) == b"Str") { must(utf8(data(v))); }
  else if (kind(v) == b"Bytes") { data(v); }
  else if (kind(v) == b"I64") { number(data(v)); }
}

Bytes block_value(Bytes s, I64 p, Bytes env, I64 ambient, Bytes memory, Bool checking)
{
  Bytes t = token(s, p);
  if (t == b"}") { return result(token_end(s, p), unit_value(), memory); }
  if (type_word(t)) {
    I64 name = type_end(s, p);
    I64 start = expect(s, name_end(s, name), b"=");
    Bytes r = expression(s, start, env, ambient, memory, checking);
    Bytes v = check_binding(s, p, found(r));
    if (failed(v)) { return result(group_end(s, cursor(r), b"}"), v, memory_of(r)); }
    I64 next = expect(s, cursor(r), b";");
    return block_value(s, next, bind(env, token(s, name), v), ambient, memory_of(r), checking);
  }
  if (t == b"return") {
    I64 next = token_end(s, p);
    if (token(s, next) == b";") {
      return finish_return(s, result(token_end(s, next), unit_value(), memory),
                           env, ambient, checking);
    }
    Bytes r = expression(s, next, env, ambient, memory, checking);
    return finish_return(s, relocate(r, expect(s, cursor(r), b";")), env, ambient, checking);
  }
  Bytes r = expression(s, p, env, ambient, memory, checking);
  if (failed(found(r))) { return relocate(r, group_end(s, cursor(r), b"}")); }
  if (token(s, cursor(r)) == b"}") { return relocate(r, token_end(s, cursor(r))); }
  I64 next = expect(s, cursor(r), b";");
  if (!depositable(found(r))) {
    return result(group_end(s, next, b"}"), problem(b"unsupported deposit type", p), memory_of(r));
  }
  if (!checking) { deposit_value(found(r)); }
  return block_value(s, next, env, ambient, memory_of(r), checking);
}

// Checking still visits unreachable syntax. Its memory cannot contaminate
// evaluation: an early return skips that syntax and keeps only work it reached.
Bytes finish_return(Bytes s, Bytes r, Bytes env, I64 ambient, Bool checking)
{
  if (checking && !failed(found(r))) {
    Bytes tail = block_value(s, cursor(r), env, ambient, memory_of(r), checking);
    if (failed(found(tail))) { return tail; }
    return replace_value(tail, found(r));
  }
  return relocate(r, group_end(s, cursor(r), b"}"));
}

Bytes placeholder_parameters(Bytes s, I64 p)
{
  if (token(s, p) == b")") { return b""; }
  Bytes v = declared_value(s, p);
  I64 next = name_end(s, type_end(s, p));
  if (token(s, next) == b")") { return packet(v); }
  I64 following = expect(s, next, b",");
  if (token(s, following) == b")") { return slice(s, -1, 0); }
  return concat(packet(v), placeholder_parameters(s, following));
}
I64 function_ambient(Bytes s, I64 p)
{
  if (token(s, p) == b"@") { return number(token(s, token_end(s, p))); }
  return 0;
}

Bytes validate_program(Bytes s, I64 p, Bytes memory)
{
  if (skip(s, p) >= len(s)) { return result(p, unit_value(), memory); }
  if (token(s, p) == b"demand") {
    Bytes r = expression(s, token_end(s, p), b"", 0, memory, true);
    if (failed(found(r))) { return r; }
    return validate_program(s, expect(s, cursor(r), b";"), memory_of(r));
  }
  I64 at_name = type_end(s, p);
  Bytes name = token(s, at_name);
  I64 after = name_end(s, at_name);
  if (token(s, after) == b"(") {
    Bytes args = placeholder_parameters(s, token_end(s, after));
    I64 signature_end = group_end(s, token_end(s, after), b")");
    Bytes r = invoke(s, name, args, function_ambient(s, signature_end), memory, true, p);
    if (failed(found(r))) { return r; }
    return validate_program(s, declaration_end(s, p), memory_of(r));
  }
  Bytes r = global_value(s, name, skip(s, at_name), memory, true);
  if (failed(found(r))) { return r; }
  return validate_program(s, declaration_end(s, p), memory_of(r));
}
Bytes evaluate_program(Bytes s, I64 p, Bytes memory)
{
  if (skip(s, p) >= len(s)) { return b""; }
  if (token(s, p) == b"demand") {
    Bytes r = expression(s, token_end(s, p), b"", 0, memory, false);
    if (failed(found(r))) { return packet(found(r)); }
    I64 next = expect(s, cursor(r), b";");
    return concat(packet(found(r)), evaluate_program(s, next, memory_of(r)));
  }
  return evaluate_program(s, declaration_end(s, p), memory);
}

// External result encoding is unchanged: one typed-value packet per demand,
// or an Error packet. Checking never lends placeholder values to evaluation.
Bytes interpret(Bytes source)
{
  if (!given(utf8(source))) { return packet(problem(b"source is not UTF-8", 0)); }
  Bytes indexed = index_program(source, 0, b"");
  if (failed(indexed)) { return packet(indexed); }
  Bytes memory = packet(data(indexed));
  Bytes checked = validate_program(source, 0, memory);
  if (failed(found(checked))) { return packet(found(checked)); }
  return evaluate_program(source, 0, memory);
}
