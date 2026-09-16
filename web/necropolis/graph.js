// Read the frozen ledger format (§7). This is a local WASM view, not an importer.
const utf8 = new TextDecoder("utf-8", { fatal: true });
const HEX = /^[0-9a-f]*$/;
export const PAGE_SIZE = 12;
export const short = (id) => id.slice(0, 8);
export const hex = (bytes) => Array.from(bytes, (b) => b.toString(16).padStart(2, "0")).join("");

export function decode(encoded) {
  if (encoded.length % 2 || !HEX.test(encoded)) throw Error("Invalid object encoding");
  const bytes = Uint8Array.from(encoded.match(/../g) || [], (b) => parseInt(b, 16));
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
  const list = (read) => {
    const n = count();
    if (n > 4096) throw Error("Preview limit: more than 4096 entries");
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
  const call = () => ({ function: name(), args: list(id) });
  let object;
  if (bytes[0] !== 32) object = value();
  else {
    byte();
    switch (byte()) {
      case 0: object = { kind: "Literal", value: value() }; break;
      case 1: object = { kind: "Apply", function: id(), args: list(id), result: id() }; break;
      case 2: object = { kind: "Hole", call: call(), stratum: stratum(), span: span() }; break;
      case 3: object = { kind: "Deposit", value: id(), span: span() }; break;
      case 4: object = { kind: "Witness", stratum: stratum(), call: call(), answer: id(), span: span() }; break;
      case 5: object = { kind: "Trace", residue: id(), holes: list(id), witnesses: list(id),
        deposits: list(id), source: id(), depth: stratum(), unrecorded: bool() }; break;
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
  if (o.span) add("source", o.span.source);
  return edges;
}

export function preview(o, limit = 160) {
  let s;
  if (o.call) return `${o.call.function}(${o.call.args.length} argument${o.call.args.length === 1 ? "" : "s"})`;
  if (o.kind === "Bytes") {
    const sample = o.data.subarray(0, limit);
    try { s = `b${JSON.stringify(utf8.decode(sample))}`; }
    catch { s = `hex ${hex(sample)}`; }
    if (o.data.length > limit) s += `… (${o.data.length} bytes)`;
  } else if (o.kind === "Str") s = JSON.stringify(o.data.slice(0, limit));
  else if (o.kind === "Shade") s = `opaque · origin ${o.origin} · ${short(o.data)}`;
  else if (o.kind === "Answer") s = `${o.data.kind === "Refusal" ? "refused" : "given"}: ${preview(o.data, limit)}`;
  else if (o.kind === "Literal") s = preview(o.value, limit);
  else if (o.kind === "Array" || o.kind === "Struct") s = `${o.name || "Array"} · ${o.data.length} entries`;
  else if (o.kind === "Trace") s = `depth ${o.depth} · ${o.holes.length} holes · ${o.deposits.length} deposits`;
  else s = String(o.data ?? o.kind);
  return s.length > limit ? s.slice(0, limit) + "…" : s;
}

export class Graph {
  constructor(burial) {
    if (!Array.isArray(burial.objects) || burial.objects.length > 20000) throw Error("Graph limit: 20,000 objects");
    this.root = burial.cairn;
    this.encoded = new Map();
    this.cache = new Map();
    this.incoming = new Map();
    let size = 0;
    for (const [id, encoded] of burial.objects) {
      if (!/^[0-9a-f]{64}$/.test(id) || typeof encoded !== "string" || this.encoded.has(id)) throw Error("Invalid object table");
      size += encoded.length;
      if (size > 32 * 1024 * 1024) throw Error("Graph limit: 16 MiB of canonical objects");
      this.encoded.set(id, encoded);
    }
    if (this.get(this.root).kind !== "Trace") throw Error("The trace root is missing or unreadable");
    for (const [id, encoded] of this.encoded) {
      if (!encoded.startsWith("20")) continue;
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
  if (span.end > BigInt(o.data.length) || span.start > span.end) return null;
  const start = Number(span.start), end = Number(span.end);
  // Slice bytes before decoding: JS string indices are not source offsets.
  const from = Math.max(0, start - 160), to = Math.min(o.data.length, end + 160);
  const display = new TextDecoder();
  return { before: display.decode(o.data.subarray(from, start)),
    selected: display.decode(o.data.subarray(start, Math.min(end, start + 2048))),
    after: end - start > 2048 ? "… (selection truncated)" : display.decode(o.data.subarray(end, to)),
    line: o.data.subarray(0, start).reduce((n, b) => n + (b === 10), 1) };
}
