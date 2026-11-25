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
  Bytes head_t = token(s, p);
  I64 next = name_end(s, p);
  if (token(s, next) == b"<") {
    if (head_t != b"Shade" && head_t != b"Answer") {
      return malformed_source(s);
    }
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
I64 latent_suffix(Bytes s, I64 p)
{
  if (token(s, p) != b"@") { return p; }
  I64 start = skip(s, token_end(s, p));
  if (token(s, start) == b"{") {
    I64 cur = token_end(s, start);
    while (true) {
      I64 dpos = skip(s, cur + 0);
      if (!digit(s, dpos) || number(token(s, dpos)) > 8) {
        return malformed_source(s);
      }
      I64 after_d = token_end(s, dpos);
      if (token(s, after_d) == b",") { cur = token_end(s, after_d); }
      else { return expect(s, after_d, b"}"); }
    }
  }
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
    I64 body = expect(s, latent_suffix(s, after_params), b"{");
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
  if (t == b"disk!") { return 4; }
  if (t == b"net") { return 5; }
  if (t == b"net!") { return 6; }
  if (t == b"entropy") { return 7; }
  if (t == b"unrecorded") { return 8; }
  return -1;
}
I64 deepest_held(I64 mask)
{
  I64 d = 8;
  while (d > 0) {
    if ((mask & (1 << d)) != 0) { return d + 0; }
    d -= 1;
  }
  return 0;
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
  I64 d = annotation(s, p);
  if (t == b"Shade" || t == b"Answer") {
    Bytes inner = declared_value(s, expect(s, token_end(s, p), b"<"));
    if (failed(inner)) { return inner; }
    return value(t, d, inner);
  }
  return value(t, d, b"");
}
Bytes placeholder_value(Bytes s, I64 p)
{
  Bytes v = declared_value(s, p);
  if (failed(v)) { return v; }
  return check_param_origins(v, p);
}
Bytes check_param_origins(Bytes v, I64 p)
{
  I64 d = max(0, depth(v));
  Bytes t = kind(v);
  if (t == b"Shade") {
    if (depth(data(v)) < 0) {
      return problem(b"shade parameter requires explicit origin depth", p);
    }
    Bytes inner = check_param_origins(data(v), p);
    if (failed(inner)) { return inner; }
    return value(t, d, inner);
  }
  if (t == b"Answer") {
    Bytes inner = check_param_origins(data(v), p);
    if (failed(inner)) { return inner; }
    return value(t, d, inner);
  }
  return value(t, d, data(v));
}
Bool same_type(Bytes a, Bytes b)
{
  if (kind(a) != kind(b)) { return false; }
  // Refusal is a valid inhabitant of every Answer<T>, not a new result type.
  if (kind(a) == b"Answer" && kind(data(b)) == b"Refusal") { return true; }
  if (kind(a) == b"Shade") {
    I64 want_origin = depth(data(a));
    if (want_origin >= 0 && want_origin != depth(data(b))) { return false; }
    return same_type(data(a), data(b));
  }
  if (kind(a) == b"Answer") {
    return same_type(data(a), data(b));
  }
  return true;
}
Bytes check_binding(Bytes s, I64 p, Bytes v, Bool check_asserts)
{
  if (failed(v)) { return v; }
  Bytes want = declared_value(s, p);
  if (failed(want)) { return want; }
  if (!same_type(want, v)) { return problem(b"binding type mismatch", p); }
  I64 asserted = annotation(s, p);
  if (check_asserts && asserted >= 0 && asserted != depth(v)) {
    return problem(b"depth assertion mismatch", p);
  }
  return v;
}

// A global moves exactly once: absent -> Pending -> value. Pending is a
// black hole, not a guessed value; a dependency cycle names the binding.
// Global initializers have no caller locals and always begin at ambient {0}.
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
  Bytes r = expression(s, token_end(s, after), b"", 1, pending, checking);
  if (!failed(found(r)) && token(s, cursor(r)) != b";") {
    return result(p, problem(b"expected ;", cursor(r)), memory_of(r));
  }
  Bytes v = check_binding(s, global, found(r), true);
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
    if (failed(found(r)) || kind(found(r)) == b"Return") {
      return relocate(r, group_end(s, cursor(r), b")"));
    }
    return relocate(r, expect(s, cursor(r), b")"));
  }
  if (t == b"descend") {
    Bytes raw_cap = token(s, next);
    I64 raw_end = token_end(s, next);
    Bool has_bang = raw_end < len(s) && at(s, raw_end, b"!");
    Bytes cap_name = raw_cap;
    I64 cap_end = raw_end;
    if (has_bang) {
      cap_name = concat(raw_cap, b"!");
      cap_end = raw_end + 1;
    }
    I64 d = capability(cap_name);
    if (d < 0) { return result(next, problem(b"unknown capability", next), memory); }
    I64 body = expect(s, cap_end, b"{");
    return block_value(s, body, env, b"", ambient | (1 << d), memory, checking);
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
  if (failed(v) || kind(v) == b"Return") { return v; }
  if (rite == b"shade") { return value(b"Shade", 0, v); }
  if (rite == b"seal") {
    if (checking) { return value(b"Cairn", 0, b""); }
    return problem(b"runtime seal is not supported by this bootstrap", p);
  }
  if (kind(v) != b"Shade") { return problem(b"look requires a Shade", p); }
  I64 origin = depth(data(v));
  if ((ambient & (1 << origin)) == 0) {
    Bytes message = concat(b"Orpheus: origin ", decimal(origin));
    return problem(concat(message, concat(b", ambient ", decimal(deepest_held(ambient)))), p);
  }
  return deepen(data(v), depth(v));
}

Bytes arguments(Bytes s, I64 p, Bytes env, I64 ambient, Bytes memory, Bool checking)
{
  if (token(s, p) == b")") { return result(token_end(s, p), b"", memory); }
  Bytes r = expression(s, p, env, ambient, memory, checking);
  if (failed(found(r)) || kind(found(r)) == b"Return") {
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
  if (failed(head(args)) || kind(head(args)) == b"Return") { return head(args); }
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
  Bytes v = check_binding(s, p, head(args), true);
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

Bytes unwrap_return(Bytes v)
{
  if (kind(v) == b"Return") { return data(v); }
  return v;
}
Bytes wrap_return(Bytes v)
{
  if (kind(v) == b"Return") { return v; }
  return value(b"Return", depth(v), v);
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
  I64 callee_ambient = function_ambient(s, after);
  if ((callee_ambient & ambient) != callee_ambient) {
    return result(p, problem(b"call requires a deeper ambient stratum", p), memory);
  }
  I64 arg_d = argument_depth(args);
  I64 body_ambient = callee_ambient | 131072;
  if (arg_d > 0) { body_ambient = body_ambient | 65536; }
  I64 body = expect(s, latent_suffix(s, after), b"{");
  Bytes r = block_value(s, body, data(found(bound)), b"", body_ambient, memory, checking);
  Bytes v = check_binding(s, f, unwrap_return(found(r)), arg_d == 0);
  if (failed(v)) { return replace_value(r, v); }
  return replace_value(r, deepen(v, arg_d));
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
    if (kind(a) != b"Bytes") {
      return problem(b"len requires Bytes", p);
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
    if ((ambient & (1 << d)) == 0) { return problem(b"request requires descend", p); }
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

Bytes block_value(Bytes s, I64 p, Bytes env, Bytes locals, I64 ambient, Bytes memory, Bool checking)
{
  Bytes t = token(s, p);
  if (t == b"}") { return result(token_end(s, p), unit_value(), memory); }
  if (type_word(t)) {
    I64 name = type_end(s, p);
    Bytes lname = token(s, name);
    if (len(lookup(locals, lname)) > 0) {
      return result(group_end(s, p, b"}"), problem(concat(b"duplicate local: ", lname), name), memory);
    }
    I64 start = expect(s, name_end(s, name), b"=");
    Bytes r = expression(s, start, env, ambient, memory, checking);
    if (failed(found(r))) { return relocate(r, group_end(s, cursor(r), b"}")); }
    if (kind(found(r)) == b"Return") {
      Bytes check_env = env;
      if (checking) {
        Bytes ph = placeholder_value(s, p);
        if (failed(ph)) { return result(group_end(s, cursor(r), b"}"), ph, memory_of(r)); }
        check_env = bind(env, lname, ph);
      }
      return finish_return(s, relocate(r, expect(s, cursor(r), b";")),
                           check_env, bind(locals, lname, unit_value()), ambient, checking);
    }
    Bytes v = check_binding(s, p, found(r), (ambient & 65536) == 0);
    if (failed(v)) { return result(group_end(s, cursor(r), b"}"), v, memory_of(r)); }
    I64 next = expect(s, cursor(r), b";");
    return block_value(s, next, bind(env, lname, v), bind(locals, lname, unit_value()),
                       ambient, memory_of(r), checking);
  }
  if (t == b"return") {
    if ((ambient & 131072) == 0) {
      return result(group_end(s, p, b"}"), problem(b"return outside function", p), memory);
    }
    I64 next = token_end(s, p);
    if (token(s, next) == b";") {
      Bytes rv = value(b"Return", 0, unit_value());
      return finish_return(s, result(token_end(s, next), rv, memory), env, locals, ambient, checking);
    }
    Bytes r = expression(s, next, env, ambient, memory, checking);
    if (failed(found(r))) { return relocate(r, group_end(s, cursor(r), b"}")); }
    Bytes rv = wrap_return(found(r));
    return finish_return(s, result(expect(s, cursor(r), b";"), rv, memory_of(r)),
                         env, locals, ambient, checking);
  }
  Bytes r = expression(s, p, env, ambient, memory, checking);
  if (failed(found(r))) { return relocate(r, group_end(s, cursor(r), b"}")); }
  if (kind(found(r)) == b"Return") {
    return finish_return(s, r, env, locals, ambient, checking);
  }
  if (token(s, cursor(r)) == b"}") { return relocate(r, token_end(s, cursor(r))); }
  I64 next = expect(s, cursor(r), b";");
  if (!depositable(found(r))) {
    return result(group_end(s, next, b"}"), problem(b"unsupported deposit type", p), memory_of(r));
  }
  if (!checking) { deposit_value(found(r)); }
  return block_value(s, next, env, locals, ambient, memory_of(r), checking);
}

// Checking still visits unreachable syntax. Its memory cannot contaminate
// evaluation: an early return skips that syntax and keeps only work it reached.
Bytes finish_return(Bytes s, Bytes r, Bytes env, Bytes locals, I64 ambient, Bool checking)
{
  if (checking && !failed(found(r))) {
    if (token(s, cursor(r)) == b"}") {
      return relocate(r, token_end(s, cursor(r)));
    }
    Bytes tail = block_value(s, cursor(r), env, locals, ambient, memory_of(r), checking);
    if (failed(found(tail))) { return tail; }
    Bytes tf = found(tail);
    if (kind(tf) == b"Return" || kind(tf) != b"U0") {
      Bytes rv = unwrap_return(found(r));
      Bytes tv = unwrap_return(tf);
      if (!same_type(rv, tv) && !same_type(tv, rv)) {
        return replace_value(tail, problem(b"unreachable return type mismatch", cursor(r)));
      }
      return replace_value(tail, wrap_return(deepen(rv, depth(tv))));
    }
    return replace_value(tail, found(r));
  }
  return relocate(r, group_end(s, cursor(r), b"}"));
}

Bytes placeholder_parameters(Bytes s, I64 p)
{
  if (token(s, p) == b")") { return b""; }
  Bytes v = placeholder_value(s, p);
  I64 next = name_end(s, type_end(s, p));
  if (token(s, next) == b")") { return packet(v); }
  I64 following = expect(s, next, b",");
  if (token(s, following) == b")") { return slice(s, -1, 0); }
  return concat(packet(v), placeholder_parameters(s, following));
}
I64 function_ambient(Bytes s, I64 p)
{
  I64 mask = 1;
  if (token(s, p) != b"@") { return mask + 0; }
  I64 start = skip(s, token_end(s, p));
  if (token(s, start) == b"{") {
    I64 cur = token_end(s, start);
    while (true) {
      I64 dpos = skip(s, cur + 0);
      I64 d = number(token(s, dpos));
      mask = mask | (1 << d);
      I64 after_d = token_end(s, dpos);
      if (token(s, after_d) == b",") { cur = token_end(s, after_d); }
      else { return mask + 0; }
    }
  }
  return mask | (1 << number(token(s, start)));
}

Bytes check_latent_exact(Bytes s, I64 f, Bytes bound_env, I64 body, I64 declared, Bytes memory)
{
  I64 d = 1;
  while (d <= 8) {
    I64 bit = 1 << d;
    if ((declared & bit) != 0) {
      I64 without = (declared ^ bit) | 131072;
      Bytes probe = block_value(s, body, bound_env, b"", without, memory, true);
      if (!failed(found(probe))) {
        return problem(b"latent depth assertion mismatch", f);
      }
    }
    d += 1;
  }
  return unit_value();
}

Bytes validate_program(Bytes s, I64 p, Bytes memory)
{
  if (skip(s, p) >= len(s)) { return result(p, unit_value(), memory); }
  if (token(s, p) == b"demand") {
    Bytes r = expression(s, token_end(s, p), b"", 1, memory, true);
    if (failed(found(r))) { return r; }
    return validate_program(s, expect(s, cursor(r), b";"), memory_of(r));
  }
  I64 at_name = type_end(s, p);
  Bytes name = token(s, at_name);
  I64 after = name_end(s, at_name);
  if (token(s, after) == b"(") {
    Bytes args = placeholder_parameters(s, token_end(s, after));
    I64 signature_end = group_end(s, token_end(s, after), b")");
    I64 declared_ambient = function_ambient(s, signature_end);
    Bytes r = invoke(s, name, args, declared_ambient, memory, true, p);
    if (failed(found(r))) { return r; }
    if (declared_ambient > 1) {
      Bytes bound = parameters(s, token_end(s, after), args, b"", memory);
      I64 body = expect(s, latent_suffix(s, signature_end), b"{");
      Bytes exact = check_latent_exact(s, p, data(found(bound)), body, declared_ambient, memory);
      if (failed(exact)) { return result(p, exact, memory_of(r)); }
    }
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
    Bytes r = expression(s, token_end(s, p), b"", 1, memory, false);
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
