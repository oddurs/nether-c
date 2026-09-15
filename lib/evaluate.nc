// Demand evaluator. Joined after interpreter.nc as one Nether C compilation
// unit. The host supplies bytes and answers; parsing and checking happen here.
//
// Values and environments use length-prefixed Bytes records because the
// bootstrap burier cannot yet reduce aggregate construction/projection.
// Value = packet(type), packet(depth), payload. Result = packet(cursor), value.
// A Shade holds an inner value, preserving its origin independently of ambient
// scope. An Answer holds either the given value or a Refusal value.

Bytes decimal(I64 n)
{
  if (n < 0) { return concat(b"-", decimal(-n)); }
  if (n < 10) { return slice(b"0123456789", n, n + 1); }
  return concat(decimal(n / 10), slice(b"0123456789", n % 10, n % 10 + 1));
}

I64 number_from(Bytes s, I64 p, I64 n)
{
  if (p == len(s)) { return n; }
  I64 d = seek(b"0123456789", 0, slice(s, p, p + 1));
  if (d == 10) { return malformed_source(s); }
  return number_from(s, p + 1, n * 10 + d);
}

I64 number(Bytes s)
{
  if (starts_with(s, b"-")) { return -number_from(s, 1, 0); }
  return number_from(s, 0, 0);
}

Bytes packet(Bytes s) { concat(decimal(len(s)), concat(b":", s)) }
I64 packet_start(Bytes s) { seek(s, 0, b":") + 1 }
I64 packet_end(Bytes s) { packet_start(s) + number(slice(s, 0, packet_start(s) - 1)) }
Bytes head(Bytes s) { slice(s, packet_start(s), packet_end(s)) }
Bytes rest(Bytes s) { slice(s, packet_end(s), len(s)) }

Bytes value(Bytes t, I64 d, Bytes payload)
{
  return concat(packet(t), concat(packet(decimal(d)), payload));
}
Bytes kind(Bytes v) { head(v) }
I64 depth(Bytes v) { number(head(rest(v))) }
Bytes data(Bytes v) { rest(rest(v)) }
Bytes unit_value() { value(b"U0", 0, b"") }
Bytes problem(Bytes message, I64 p)
{
  return value(b"Error", 0, concat(message, concat(b" at byte ", decimal(p))));
}
Bytes result(I64 p, Bytes v) { concat(packet(decimal(p)), v) }
I64 cursor(Bytes r) { number(head(r)) }
Bytes found(Bytes r) { rest(r) }

Bytes bind(Bytes env, Bytes name, Bytes v)
{
  return concat(packet(name), concat(packet(v), env));
}
Bytes lookup(Bytes env, Bytes name)
{
  if (len(env) == 0) { return b""; }
  if (head(env) == name) { return head(rest(env)); }
  return lookup(rest(rest(env)), name);
}

// Structural scans skip complete tokens, including strings and comments.
I64 group_end(Bytes s, I64 p, Bytes close)
{
  Bytes t = token(s, p);
  I64 next = token_end(s, p);
  if (t == close) { return next; }
  if (t == b"") { return malformed_source(s); }
  if (t == b"(") { return group_end(s, group_end(s, next, b")"), close); }
  if (t == b"{") { return group_end(s, group_end(s, next, b"}"), close); }
  if (t == b"[") { return group_end(s, group_end(s, next, b"]"), close); }
  return group_end(s, next, close);
}

I64 type_end(Bytes s, I64 p)
{
  I64 next = name_end(s, p);
  if (token(s, next) == b"<") { return type_suffix(s, expect(s, type_end(s, token_end(s, next)), b">")); }
  return type_suffix(s, next);
}
I64 type_suffix(Bytes s, I64 p)
{
  if (token(s, p) == b"@") { return token_end(s, token_end(s, p)); }
  return p;
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
I64 declaration(Bytes s, I64 p, Bytes name)
{
  if (skip(s, p) >= len(s)) { return -1; }
  if (token(s, p) != b"demand" && token(s, type_end(s, p)) == name) { return p; }
  return declaration(s, declaration_end(s, p), name);
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

Bytes declared_value(Bytes s, I64 p)
{
  Bytes t = token(s, p);
  if (t == b"Shade" || t == b"Answer") {
    I64 inner = expect(s, token_end(s, p), b"<");
    return value(t, 0, declared_value(s, inner));
  }
  if (t != b"U0" && t != b"Bytes" && t != b"Str" && t != b"I64" && t != b"Bool" && t != b"Cairn") {
    return problem(b"unsupported type", p);
  }
  I64 next = token_end(s, p);
  if (token(s, next) == b"@") { return value(t, number(token(s, token_end(s, next))), b""); }
  return value(t, 0, b"");
}
Bool same_type(Bytes a, Bytes b)
{
  if (kind(a) != kind(b)) { return false; }
  if (kind(a) == b"Shade" || kind(a) == b"Answer") { return same_type(data(a), data(b)); }
  return true;
}
Bytes check_binding(Bytes s, I64 p, Bytes v)
{
  if (kind(v) == b"Error") { return v; }
  Bytes want = declared_value(s, p);
  if (!same_type(want, v)) { return problem(b"binding type mismatch", p); }
  I64 end = type_end(s, p);
  // The outer annotation is the last two tokens of a type, never its inner one.
  I64 ann = annotation(s, p, end, 0);
  if (ann >= 0 && ann != depth(v)) { return problem(b"depth assertion mismatch", p); }
  return v;
}
I64 annotation(Bytes s, I64 p, I64 end, I64 nesting)
{
  if (skip(s, p) >= skip(s, end)) { return -1; }
  Bytes t = token(s, p);
  if (t == b"@" && nesting == 0) { return number(token(s, token_end(s, p))); }
  if (t == b"<") { return annotation(s, token_end(s, p), end, nesting + 1); }
  if (t == b">") { return annotation(s, token_end(s, p), end, nesting - 1); }
  return annotation(s, token_end(s, p), end, nesting);
}

// checking=true computes types/depths with placeholder values. No prelude
// request or deposit may happen in that pass, even for unused globals.
Bytes expression(Bytes s, I64 p, Bytes env, I64 ambient, Bool checking)
{
  I64 start = skip(s, p);
  I64 next = token_end(s, start);
  Bytes t = token(s, start);
  if (starts_with(t, b"b\"")) { return result(next, value(b"Bytes", 0, text(s, start + 1))); }
  if (starts_with(t, b"\"")) { return result(next, value(b"Str", 0, text(s, start))); }
  if (t == b"(") {
    Bytes r = expression(s, next, env, ambient, checking);
    return result(expect(s, cursor(r), b")"), found(r));
  }
  if (t == b"descend") {
    I64 d = capability(token(s, next));
    if (d < 0) { return result(next, problem(b"unknown capability", next)); }
    return block_value(s, expect(s, token_end(s, next), b"{"), env, max(ambient, d), checking);
  }
  if (t == b"shade" || t == b"look" || t == b"seal") {
    Bytes r = expression(s, next, env, ambient, checking);
    Bytes v = found(r);
    if (kind(v) == b"Error") { return r; }
    if (t == b"shade") { return result(cursor(r), value(b"Shade", 0, v)); }
    if (t == b"look") {
      if (kind(v) != b"Shade") { return result(cursor(r), problem(b"look requires a Shade", start)); }
      if (depth(data(v)) > ambient) {
        return result(cursor(r), problem(concat(b"Orpheus: origin ", concat(decimal(depth(data(v))), concat(b", ambient ", decimal(ambient)))), start));
      }
      return result(cursor(r), value(kind(data(v)), max(depth(v), depth(data(v))), data(data(v))));
    }
    if (checking) { return result(cursor(r), value(b"Cairn", 0, b"")); }
    return result(cursor(r), problem(b"runtime seal is not supported by this bootstrap", start));
  }
  if (token(s, next) == b"(") {
    Bytes args = arguments(s, token_end(s, next), env, ambient, checking);
    return result(cursor(args), invoke(s, t, found(args), ambient, checking, start));
  }
  Bytes local = lookup(env, t);
  if (len(local) > 0) { return result(next, local); }
  I64 global = declaration(s, 0, t);
  if (global < 0) { return result(next, problem(b"unknown binding", start)); }
  I64 initializer = name_end(s, type_end(s, global));
  if (token(s, initializer) != b"=") { return result(next, problem(b"function requires a call", start)); }
  Bytes r = expression(s, token_end(s, initializer), b"", 0, checking);
  return result(next, check_binding(s, global, found(r)));
}

Bytes arguments(Bytes s, I64 p, Bytes env, I64 ambient, Bool checking)
{
  if (token(s, p) == b")") { return result(token_end(s, p), b""); }
  Bytes r = expression(s, p, env, ambient, checking);
  if (token(s, cursor(r)) == b")") { return result(token_end(s, cursor(r)), packet(found(r))); }
  Bytes tail = arguments(s, expect(s, cursor(r), b","), env, ambient, checking);
  return result(cursor(tail), concat(packet(found(r)), found(tail)));
}

Bytes argument_error(Bytes args)
{
  if (len(args) == 0) { return b""; }
  if (kind(head(args)) == b"Error") { return head(args); }
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

Bytes parameters(Bytes s, I64 p, Bytes args, Bytes env, Bool checking)
{
  if (token(s, p) == b")") {
    if (len(args) != 0) { return result(p, problem(b"too many arguments", p)); }
    return result(token_end(s, p), value(b"Environment", 0, env));
  }
  if (len(args) == 0) { return result(p, problem(b"too few arguments", p)); }
  Bytes v = check_binding(s, p, head(args));
  if (kind(v) == b"Error") { return result(p, v); }
  I64 name = type_end(s, p);
  I64 next = name_end(s, name);
  Bytes bound = bind(env, token(s, name), v);
  if (token(s, next) == b")") { return parameters(s, next, rest(args), bound, checking); }
  return parameters(s, expect(s, next, b","), rest(args), bound, checking);
}

Bytes invoke(Bytes s, Bytes name, Bytes args, I64 ambient, Bool checking, I64 p)
{
  Bytes bad = argument_error(args);
  if (len(bad) > 0) { return bad; }
  I64 f = declaration(s, 0, name);
  if (f >= 0) {
    I64 start = expect(s, name_end(s, type_end(s, f)), b"(");
    Bytes bound = parameters(s, start, args, b"", checking);
    if (kind(found(bound)) == b"Error") { return found(bound); }
    I64 after = cursor(bound);
    if (token(s, after) == b"@" && number(token(s, token_end(s, after))) > ambient) {
      return problem(b"call requires a deeper ambient stratum", p);
    }
    Bytes r = block_value(s, expect(s, type_suffix(s, after), b"{"), data(found(bound)), ambient, checking);
    Bytes v = check_binding(s, f, found(r));
    if (kind(v) == b"Error") { return v; }
    return value(kind(v), max(depth(v), argument_depth(args)), data(v));
  }
  I64 count = argument_count(args);
  if (name == b"concat") {
    if (count != 2) { return problem(b"concat arity", p); }
    Bytes a = head(args);
    Bytes b = head(rest(args));
    if (kind(a) != b"Bytes" || kind(b) != b"Bytes") { return problem(b"concat requires Bytes", p); }
    return value(b"Bytes", max(depth(a), depth(b)), concat(data(a), data(b)));
  }
  if (count != 1) { return problem(b"unknown call or wrong arity", p); }
  Bytes a = head(args);
  if (name == b"must") {
    if (kind(a) != b"Answer") { return problem(b"must requires Answer", p); }
    if (kind(data(a)) == b"Refusal") { return problem(b"must: world refused", p); }
    return value(kind(data(a)), max(depth(a), depth(data(a))), data(data(a)));
  }
  if (name == b"len") {
    if (kind(a) != b"Bytes" && kind(a) != b"Str") { return problem(b"len requires Bytes or Str", p); }
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
    if (checking) { return value(b"Answer", max(d, depth(a)), value(b"Bytes", d, b"")); }
    return world_request(name, data(a));
  }
  return problem(b"unsupported prelude function", p);
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
    if (given(a)) { return value(b"Answer", 3, value(b"Bytes", 3, must(a))); }
    return value(b"Answer", 3, value(b"Refusal", 3, b"refused"));
  }
  Answer<Bytes> a = descend net { get(must(utf8(path))) };
  if (given(a)) { return value(b"Answer", 5, value(b"Bytes", 5, must(a))); }
  return value(b"Answer", 5, value(b"Refusal", 5, b"refused"));
}

U0 deposit_value(Bytes v)
{
  if (kind(v) == b"Str") { must(utf8(data(v))); }
  else if (kind(v) == b"Bytes") { data(v); }
  else if (kind(v) == b"I64") { number(data(v)); }
}

Bool type_word(Bytes t)
{
  return t == b"U0" || t == b"Bytes" || t == b"Str" || t == b"I64" || t == b"Shade" || t == b"Answer" || t == b"Cairn" || t == b"Bool";
}

Bytes block_value(Bytes s, I64 p, Bytes env, I64 ambient, Bool checking)
{
  if (token(s, p) == b"}") { return result(token_end(s, p), unit_value()); }
  if (type_word(token(s, p))) {
    I64 name = type_end(s, p);
    Bytes r = expression(s, expect(s, name_end(s, name), b"="), env, ambient, checking);
    Bytes v = check_binding(s, p, found(r));
    if (kind(v) == b"Error") { return result(group_end(s, cursor(r), b"}"), v); }
    return block_value(s, expect(s, cursor(r), b";"), bind(env, token(s, name), v), ambient, checking);
  }
  if (token(s, p) == b"return") {
    if (token(s, token_end(s, p)) == b";") {
      I64 next = token_end(s, token_end(s, p));
      if (checking) {
        Bytes tail = block_value(s, next, env, ambient, checking);
        if (kind(found(tail)) == b"Error") { return tail; }
        return result(cursor(tail), unit_value());
      }
      return result(group_end(s, next, b"}"), unit_value());
    }
    Bytes r = expression(s, token_end(s, p), env, ambient, checking);
    I64 next = expect(s, cursor(r), b";");
    if (checking) {
      Bytes tail = block_value(s, next, env, ambient, checking);
      if (kind(found(tail)) == b"Error") { return tail; }
      return result(cursor(tail), found(r));
    }
    return result(group_end(s, next, b"}"), found(r));
  }
  Bytes r = expression(s, p, env, ambient, checking);
  if (kind(found(r)) == b"Error") { return result(group_end(s, cursor(r), b"}"), found(r)); }
  if (token(s, cursor(r)) == b"}") { return result(token_end(s, cursor(r)), found(r)); }
  I64 next = expect(s, cursor(r), b";");
  Bytes t = kind(found(r));
  if (t != b"U0" && t != b"Str" && t != b"Bytes" && t != b"I64") {
    return result(group_end(s, next, b"}"), problem(b"unsupported deposit type", p));
  }
  if (!checking) { deposit_value(found(r)); }
  return block_value(s, next, env, ambient, checking);
}

Bytes placeholder_parameters(Bytes s, I64 p)
{
  if (token(s, p) == b")") { return b""; }
  Bytes v = declared_value(s, p);
  I64 next = name_end(s, type_end(s, p));
  if (token(s, next) == b")") { return packet(v); }
  return concat(packet(v), placeholder_parameters(s, expect(s, next, b",")));
}

Bytes validate_program(Bytes s, I64 p)
{
  if (skip(s, p) >= len(s)) { return unit_value(); }
  if (token(s, p) == b"demand") {
    Bytes r = expression(s, token_end(s, p), b"", 0, true);
    if (kind(found(r)) == b"Error") { return found(r); }
    return validate_program(s, expect(s, cursor(r), b";"));
  }
  I64 name = type_end(s, p);
  I64 after = name_end(s, name);
  if (declaration(s, declaration_end(s, p), token(s, name)) >= 0) { return problem(b"duplicate declaration", name); }
  if (token(s, after) == b"(") {
    Bytes args = placeholder_parameters(s, token_end(s, after));
    I64 signature_end = group_end(s, token_end(s, after), b")");
    // An explicit latent annotation supplies the function's ambient context.
    I64 ambient = function_ambient(s, signature_end);
    Bytes v = invoke(s, token(s, name), args, ambient, true, p);
    if (kind(v) == b"Error") { return v; }
    return validate_program(s, declaration_end(s, p));
  }
  Bytes r = expression(s, expect(s, after, b"="), b"", 0, true);
  Bytes v = check_binding(s, p, found(r));
  if (kind(v) == b"Error") { return v; }
  return validate_program(s, expect(s, cursor(r), b";"));
}
I64 function_ambient(Bytes s, I64 p)
{
  if (token(s, p) == b"@") { return number(token(s, token_end(s, p))); }
  return 0;
}

Bytes evaluate_program(Bytes s, I64 p)
{
  if (skip(s, p) >= len(s)) { return b""; }
  if (token(s, p) == b"demand") {
    Bytes r = expression(s, token_end(s, p), b"", 0, false);
    if (kind(found(r)) == b"Error") { return packet(found(r)); }
    return concat(packet(found(r)), evaluate_program(s, expect(s, cursor(r), b";")));
  }
  return evaluate_program(s, declaration_end(s, p));
}

// The result is a packet per demand, each containing a typed value. A checking
// error is a single Error value. Expression statements deposit native values;
// demanded values remain in this return record and are not extra deposits.
Bytes interpret(Bytes source)
{
  Bytes checked = validate_program(source, 0);
  if (kind(checked) == b"Error") { return packet(checked); }
  return evaluate_program(source, 0);
}
