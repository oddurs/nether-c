import assert from "node:assert/strict";
import { test } from "node:test";
import { decode, Graph, Navigation, hex, preview, references, sourceExcerpt } from "../../web/necropolis/graph.js";

const u64 = (n) => BigInt.asUintN(64, BigInt(n)).toString(16).padStart(16, "0");
const text = (s) => { const b = new TextEncoder().encode(s); return u64(b.length) + hex(b); };
const id = (n) => n.toString(16).padStart(64, "0");
const ids = (...values) => u64(values.length) + values.join("");
const span = id(1) + u64(2) + u64(5);
const call = text("read") + ids(id(2));
const trace = "2005" + id(3) + ids(id(4)) + ids() + ids(id(5)) + id(1) + "0300";

test("every value tag keeps its exact payload", () => {
  assert.equal(decode("00").kind, "U0");
  assert.equal(decode("0101").data, true);
  for (const n of [0n, -1n, -(2n ** 63n), 2n ** 63n - 1n]) assert.equal(decode("02" + u64(n)).data, String(n));
  assert.deepEqual([...decode("03" + u64(3) + "00ff80").data], [0, 255, 128]);
  assert.equal(decode("04" + text("雪\0</script>")).data, "雪\0</script>");
  assert.equal(decode("05" + id(3)).data, id(3));
  assert.deepEqual(decode("0605" + id(3)), { kind: "Shade", origin: 5, data: id(3) });
  assert.equal(decode("070002" + u64(-5)).data.data, "-5");
  assert.equal(decode("070100").data.data, "absent");
  assert.equal(decode("0805").data, "conflict");
  assert.equal(decode("10" + text("A") + u64(1) + "00").name, "A");
  assert.equal(decode("11" + u64(2) + "0000").data.length, 2);
});

test("every node tag exposes only its labelled recorded edges", () => {
  assert.equal(decode("200000").value.kind, "U0");
  const apply = decode("2001" + id(1) + ids(id(2), id(2)) + id(3));
  assert.deepEqual(references(apply).map((e) => e.label), ["function", "argument 1", "argument 2", "result"]);
  const hole = decode("2002" + call + "03" + span);
  assert.equal(hole.call.function, "read");
  assert.deepEqual(references(hole), [{ label: "argument 1", id: id(2) }, { label: "source", id: id(1) }]);
  assert.equal(decode("2003" + id(2) + span).kind, "Deposit");
  const witness = decode("200403" + call + id(3) + span);
  assert.equal(references(witness)[1].label, "answer");
  assert.deepEqual(references(decode(trace)).map((e) => e.label), ["residue", "hole 1", "deposit 1", "source"]);
  assert.deepEqual(references(decode("0605" + id(3))), [], "shades stay opaque");
});

test("malformed canonical objects fail closed", () => {
  for (const encoded of ["", "0", "zz", "ff", "0102", "0000", "03" + u64(10),
    "04" + u64(1) + "ff", "0609" + id(1), "0702", "0806", "10" + text(""), "20ff",
    "2002" + text("") + ids() + "00" + span,
    "2003" + id(1) + id(1) + u64(5) + u64(2),
    trace.slice(0, -2) + "02", "11" + u64(2 ** 53),
    "0700".repeat(130) + "00"]) {
    assert.throws(() => decode(encoded), undefined, encoded);
  }
});

test("shared references point to one object and missing objects remain missing", () => {
  const g = new Graph({ cairn: id(9), objects: [[id(9), trace], [id(4), "2002" + call + "03" + span]] });
  assert.equal(g.get(id(1)).kind, "Missing");
  assert.equal(g.incoming.get(id(1)).length, 2);
  assert.equal(g.get(id(4)), g.get(id(4)), "decoded objects are cached");
  assert.throws(() => new Graph({ cairn: id(9), objects: [[id(9), trace], [id(9), trace]] }));
  assert.throws(() => new Graph({ cairn: id(9), objects: [] }));
});

test("source spans use byte offsets, not UTF-16 positions", () => {
  const bytes = new TextEncoder().encode("é\n雪 target end");
  const g = { get: () => ({ kind: "Bytes", data: bytes }) };
  const excerpt = sourceExcerpt(g, { source: id(1), start: 7n, end: 13n });
  assert.equal(excerpt.selected, "target");
  assert.equal(excerpt.line, 2);
  assert.equal(sourceExcerpt(g, { source: id(1), start: 0n, end: 100n }), null);
});

test("navigation branches discard forward history without duplicating the current object", () => {
  const n = new Navigation("root");
  n.visit("hole"); n.visit("argument"); n.move(-1); n.visit("source");
  assert.deepEqual(n.entries, ["root", "hole", "source"]);
  n.visit("source"); n.move(20); assert.equal(n.current, "source");
  n.move(-20); assert.equal(n.current, "root");
});

test("previews are bounded and binary bytes are never silently repaired", () => {
  assert.match(preview(decode("03" + u64(2) + "ff00")), /hex ff00/);
  assert.ok(preview(decode("04" + text("a".repeat(10000)))).length <= 161);
});
