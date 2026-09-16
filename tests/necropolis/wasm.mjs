import assert from "node:assert/strict";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import { Necropolis } from "../../web/necropolis/necropolis.js";
import { Graph, references } from "../../web/necropolis/graph.js";

const { instance } = await WebAssembly.instantiate(await readFile(new URL("../../web/necropolis/nether.wasm", import.meta.url)), {});
const nether = new Necropolis(instance.exports);

test("the actual module buries samples and exports a traversable graph", async () => {
  for (const name of ["hello", "build"]) {
    const source = await readFile(new URL(`../programs/${name}.nc`, import.meta.url), "utf8");
    const burial = nether.bury(source);
    assert.equal(burial.error, undefined, burial.error);
    const graph = new Graph(burial);
    for (const [id] of graph.encoded) {
      const object = graph.get(id);
      assert.notEqual(object.kind, "Unavailable", object.data);
      for (const edge of references(object)) assert.notEqual(graph.get(edge.id).kind, "Missing");
    }
    assert.equal(graph.get(graph.root).kind, "Trace");
  }
});

test("WASM call and fuel limits are diagnostics, not traps", () => {
  const recursive = nether.bury("I64 f(I64 n) { f(n + 1) } demand f(0);", 1000000);
  assert.match(recursive.error, /128 calls deep/);
  assert.match(nether.bury("demand 1 + 2;", 1).error, /fuel/);
  assert.equal(nether.bury("demand 1;").error, undefined, "module remains usable after a diagnostic");
});

test("repeated module burials retain exact integers and hostile text as values", () => {
  const source = 'U0 f() { -9223372036854775807 - 1; 9223372036854775807; "</script><img src=x onerror=alert(1)>"; } demand f();';
  let previous;
  for (let i = 0; i < 5; i++) {
    const burial = nether.bury(source);
    assert.equal(burial.error, undefined, burial.error);
    if (previous) assert.equal(burial.cairn, previous);
    previous = burial.cairn;
    const graph = new Graph(burial);
    const values = [...graph.encoded.keys()].map((id) => graph.get(id));
    assert.ok(values.some((v) => v.data === "-9223372036854775808"));
    assert.ok(values.some((v) => v.data === "9223372036854775807"));
    assert.ok(values.some((v) => v.kind === "Str" && v.data.startsWith("</script>")));
  }
});
