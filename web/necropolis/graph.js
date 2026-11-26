// Read the frozen ledger format (§7). This is a local WASM view, not an importer.
export const utf8 = new TextDecoder("utf-8", { fatal: true, ignoreBOM: true });
const displayUtf8 = new TextDecoder("utf-8", { ignoreBOM: true });
const HEX = /^[0-9a-f]*$/;
const CAIRN = /^[0-9a-f]{64}$/;
export const PAGE_SIZE = 12;
export const short = (id) => id.slice(0, 8);
export const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");
export const plural = (n, word) => `${n} ${word}${n === 1 ? "" : "s"}`;

// Slice at most `limit` bytes without cutting inside a multi-byte UTF-8 sequence.
export function utf8Prefix(bytes, limit) {
  let end = Math.min(bytes.length, limit);
  if (end === bytes.length) return bytes;
  // Walk back past continuation bytes (10xxxxxx) to the leading byte.
  let start = end;
  while (start > 0 && (bytes[start] & 0xC0) === 0x80) start--;
  if (start < end) {
    const lead = bytes[start];
    const width = lead < 0x80 ? 1 : (lead & 0xE0) === 0xC0 ? 2 : (lead & 0xF0) === 0xE0 ? 3 : (lead & 0xF8) === 0xF0 ? 4 : 1;
    if (start + width > end) end = start;
  }
  return bytes.subarray(0, end);
}

export function decode(encoded) {
  if (encoded.length % 2 || !HEX.test(encoded)) throw Error("Invalid object encoding");
  const bytes = new Uint8Array(encoded.length / 2);
  const nibble = (c) => c <= 57 ? c - 48 : c - 87;
  for (let i = 0; i < bytes.length; i++) {
    bytes[i] = (nibble(encoded.charCodeAt(i * 2)) << 4) | nibble(encoded.charCodeAt(i * 2 + 1));
  }
  let p = 0;
  const take = (n) => {
    if (!Number.isSafeInteger(n) || n < 0 || n > bytes.length - p) throw Error("Truncated object");
    const out = bytes.subarray(p, p + n);
    p += n;
    return out;
  };
  const byte = () => take(1)[0];
  const uint = () => { const b = take(8); return new DataView(b.buffer, b.byteOffset, 8).getBigUint64(0); };
  const count = () => {
    const n = uint();
    if (n > BigInt(bytes.length - p)) throw Error("Length exceeds object");
    return Number(n);
  };
  const text = () => utf8.decode(take(count()));
  const name = () => { const s = text(); if (!s) throw Error("Empty name"); return s; };
  const id = () => hex(take(32));
  const stratum = () => { const d = byte(); if (d > 8) throw Error("Invalid stratum"); return d; };
  const bool = () => { const b = byte(); if (b > 1) throw Error("Invalid boolean"); return b === 1; };
  const list = (read, max = 4096, itemBytes = 1) => {
    const n = count();
    if (BigInt(n) * BigInt(itemBytes) > BigInt(bytes.length - p)) throw Error("Length exceeds object");
    if (n > max) throw Error(`Preview limit: more than ${max} entries`);
    return Array.from({ length: n }, read);
  };
  const refusal = () => {
    const names = ["absent", "denied", "malformed", "unreachable", "exhausted", "conflict"];
    const data = names[byte()];
    if (!data) throw Error("Invalid refusal");
    return { kind: "Refusal", data };
  };
  const value = (level = 0) => {
    if (level > 128) throw Error("Value nesting limit");
    switch (byte()) {
      case 0: return { kind: "U0", data: "unit" };
      case 1: return { kind: "Bool", data: bool() };
      case 2: return { kind: "I64", data: BigInt.asIntN(64, uint()).toString() };
      case 3: return { kind: "Bytes", data: take(count()) };
      case 4: return { kind: "Str", data: text() };
      case 5: return { kind: "Cairn", data: id() };
      case 6: return { kind: "Shade", origin: stratum(), data: id() };
      case 7: {
        const tag = byte();
        if (tag > 1) throw Error("Invalid answer");
        return { kind: "Answer", data: tag ? refusal() : value(level + 1) };
      }
      case 8: return refusal();
      case 16: return { kind: "Struct", name: name(), data: list(() => value(level + 1)) };
      case 17: return { kind: "Array", data: list(() => value(level + 1)) };
      default: throw Error("Unknown value tag");
    }
  };
  const span = () => {
    const source = id(), start = uint(), end = uint();
    if (end < start) throw Error("Backwards source span");
    return { source, start, end };
  };
  const call = () => ({ function: name(), args: list(id, 4096, 32) });
  let object;
  if (bytes[0] !== 32) object = value();
  else {
    byte();
    switch (byte()) {
      case 0: object = { kind: "Literal", value: value() }; break;
      case 1: object = { kind: "Apply", function: id(), args: list(id, 4096, 32), result: id() }; break;
      case 2: object = { kind: "Hole", call: call(), stratum: stratum(), span: span() }; break;
      case 3: object = { kind: "Deposit", value: id(), span: span() }; break;
      case 4: object = { kind: "Witness", stratum: stratum(), call: call(), answer: id(), span: span() }; break;
      case 5: object = { kind: "Trace", residue: id(), holes: list(id, 20000, 32), witnesses: list(id, 20000, 32),
        deposits: list(id, 20000, 32), source: id(), depth: stratum(), unrecorded: bool() }; break;
      default: throw Error("Unknown node kind");
    }
  }
  if (p !== bytes.length) throw Error("Trailing object bytes");
  return object;
}

// Edge labels describe stored fields, never inferred causality.
export function references(o) {
  const edges = [];
  const add = (label, id) => edges.push({ label, id });
  const many = (label, ids) => ids.forEach((id, i) => add(`${label} ${i + 1}`, id));
  const walkValue = (v, prefix) => {
    if (!v || typeof v !== "object") return;
    if (v.kind === "Shade") add(prefix ? `${prefix} shaded` : "shaded", v.data);
    else if (v.kind === "Cairn") add(prefix ? `${prefix} target` : "target", v.data);
    else if (v.kind === "Answer" && v.data && v.data.kind !== "Refusal") walkValue(v.data, prefix ? `${prefix} given` : "given");
    else if (v.kind === "Array" || v.kind === "Struct") {
      v.data.forEach((item, i) => walkValue(item, prefix ? `${prefix} #${i + 1}` : `entry ${i + 1}`));
    }
  };
  if (o.kind === "Trace") {
    add("residue", o.residue);
    many("hole", o.holes); many("witness", o.witnesses); many("deposit", o.deposits);
    add("source", o.source);
  } else if (o.kind === "Apply") {
    add("function", o.function); many("argument", o.args); add("result", o.result);
  } else if (o.kind === "Hole" || o.kind === "Witness") {
    many("argument", o.call.args);
    if (o.kind === "Witness") add("answer", o.answer);
  } else if (o.kind === "Deposit") add("value", o.value);
  else if (o.kind === "Literal") walkValue(o.value, "value");
  else walkValue(o, "");
  if (o.span) add("source", o.span.source);
  return edges;
}

export function preview(o, limit = 160) {
  let s;
  if (o.call) return `${o.call.function}(${plural(o.call.args.length, "argument")})`;
  if (o.kind === "Bytes") {
    const sample = utf8Prefix(o.data, limit);
    const suffix = `… (${o.data.length} bytes)`;
    try {
      const body = `b${JSON.stringify(utf8.decode(sample))}`;
      if (o.data.length > sample.length || body.length > limit) {
        const keep = Math.max(1, limit - suffix.length);
        return body.slice(0, keep) + suffix;
      }
      return body;
    } catch {
      const maxBytes = Math.max(1, Math.floor((limit - 4 - suffix.length) / 2));
      if (o.data.length > maxBytes) return `hex ${hex(o.data.subarray(0, maxBytes))}${suffix}`;
      return `hex ${hex(o.data)}`;
    }
  } else if (o.kind === "Str") s = JSON.stringify(o.data.slice(0, limit));
  else if (o.kind === "Shade") s = `opaque · origin ${o.origin} · ${short(o.data)}`;
  else if (o.kind === "Answer") s = `${o.data.kind === "Refusal" ? "refused" : "given"}: ${preview(o.data, limit)}`;
  else if (o.kind === "Literal") s = preview(o.value, limit);
  else if (o.kind === "Array" || o.kind === "Struct") s = `${o.name || "Array"} · ${o.data.length} entries`;
  else if (o.kind === "Trace") s = `depth ${o.depth} · ${plural(o.holes.length, "hole")} · ${plural(o.deposits.length, "deposit")}`;
  else s = String(o.data ?? o.kind);
  return s.length > limit ? s.slice(0, limit) + "…" : s;
}

export class Graph {
  constructor(burial) {
    if (!burial || !Array.isArray(burial.objects) || burial.objects.length > 20000) throw Error("Graph limit: 20,000 objects");
    this.root = burial.cairn;
    this.encoded = new Map();
    this.cache = new Map();
    this.incoming = new Map();
    let size = 0;
    for (const pair of burial.objects) {
      if (!Array.isArray(pair) || pair.length !== 2) throw Error("Invalid object table");
      const [id, encoded] = pair;
      if (!CAIRN.test(id) || typeof encoded !== "string" || this.encoded.has(id)) throw Error("Invalid object table");
      size += encoded.length;
      if (size > 32 * 1024 * 1024) throw Error("Graph limit: 16 MiB of canonical objects");
      this.encoded.set(id, encoded);
    }
    const root = this.get(this.root);
    if (root.kind !== "Trace") throw Error(root.kind === "Unavailable" ? root.data : "The trace root is missing or unreadable");
    for (const [id] of this.encoded) {
      for (const edge of references(this.get(id))) {
        if (!this.incoming.has(edge.id)) this.incoming.set(edge.id, []);
        this.incoming.get(edge.id).push({ id, label: edge.label });
      }
    }
  }
  get(id) {
    if (!this.encoded.has(id)) return { kind: "Missing", data: "Not present in this burial" };
    if (!this.cache.has(id)) {
      try { this.cache.set(id, decode(this.encoded.get(id))); }
      catch (e) { this.cache.set(id, { kind: "Unavailable", data: e.message }); }
    }
    return this.cache.get(id);
  }
}

export class Navigation {
  constructor(root) { this.entries = [root]; this.position = 0; }
  get current() { return this.entries[this.position]; }
  visit(id) {
    if (id === this.current) return;
    this.entries.splice(this.position + 1);
    this.entries.push(id);
    this.position++;
  }
  move(delta) {
    this.position = Math.max(0, Math.min(this.entries.length - 1, this.position + delta));
  }
}

export function sourceExcerpt(graph, span) {
  const o = graph.get(span.source);
  if (o.kind !== "Bytes") return null;
  if (span.start < 0n || span.end > BigInt(o.data.length) || span.start > span.end) return null;
  const start = Number(span.start), end = Number(span.end);
  // Slice bytes before decoding: JS string indices are not source offsets.
  let from = Math.max(0, start - 160);
  while (from < start && (o.data[from] & 0xC0) === 0x80) from++;
  const to = end + utf8Prefix(o.data.subarray(end), 160).length;
  const selectedBytes = utf8Prefix(o.data.subarray(start, end), 2048);
  return { before: displayUtf8.decode(o.data.subarray(from, start)),
    selected: displayUtf8.decode(selectedBytes),
    after: end - start > selectedBytes.length ? "… (selection truncated)" : displayUtf8.decode(o.data.subarray(end, to)),
    line: o.data.subarray(0, start).reduce((n, b) => n + (b === 10), 1) };
}
