// The seam, from the other side.
//
// Every block the module returns is a little-endian u32 length and then that
// many bytes of UTF-8, and the caller frees it. There is no bindings generator
// here because there are four functions.

const encoder = new TextEncoder();
const decoder = new TextDecoder();

export async function open(url = "nether.wasm") {
  const response = await fetch(url);
  if (!response.ok) throw Error(`HTTP ${response.status} fetching ${url}`);
  let instantiated;
  try {
    instantiated = await WebAssembly.instantiateStreaming(response.clone(), {});
  } catch {
    const bytes = await response.arrayBuffer();
    instantiated = await WebAssembly.instantiate(bytes, {});
  }
  return new Necropolis(instantiated.instance.exports);
}

export class Necropolis {
  constructor(exports) {
    this.m = exports;
  }

  // The memory view has to be taken after every call: a growing heap
  // detaches the old ArrayBuffer, and a view held across an allocation is a
  // view of nothing.
  get bytes() {
    return new Uint8Array(this.m.memory.buffer);
  }

  bury(source, fuel = 0) {
    if (!Number.isSafeInteger(fuel) || fuel < 0) throw RangeError("fuel must be a non-negative integer");
    const said = encoder.encode(source);
    const at = this.m.nether_alloc(said.length) >>> 0;
    if (said.length > 0 && at === 0) throw Error("WASM allocator out of memory");
    let block;
    let trapped = false;
    try {
      if (said.length > 0) this.bytes.set(said, at);
      block = this.m.nether_bury(at, said.length, BigInt(fuel)) >>> 0;
    } catch (e) {
      if (e instanceof WebAssembly.RuntimeError) trapped = true;
      throw e;
    } finally {
      if (!trapped) this.m.nether_free(at, said.length);
    }
    if (block === 0) throw Error("WASM burial failed to allocate response block");
    const len = new DataView(this.m.memory.buffer).getUint32(block, true);
    try {
      return JSON.parse(decoder.decode(this.bytes.subarray(block + 4, block + 4 + len)));
    } finally { this.m.nether_free(block, len + 4); }
  }
}
