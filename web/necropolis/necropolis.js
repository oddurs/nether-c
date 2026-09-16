// The seam, from the other side.
//
// Every block the module returns is a little-endian u32 length and then that
// many bytes of UTF-8, and the caller frees it. There is no bindings generator
// here because there are four functions.

const PAGE = 65536;

export async function open(url = "nether.wasm") {
  const { instance } = await WebAssembly.instantiateStreaming(fetch(url), {});
  return new Necropolis(instance.exports);
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
    const said = new TextEncoder().encode(source);
    const at = this.m.nether_alloc(said.length);
    let block;
    try {
      this.bytes.set(said, at);
      block = this.m.nether_bury(at, said.length, BigInt(fuel));
    } finally { this.m.nether_free(at, said.length); }
    const len = new DataView(this.m.memory.buffer).getUint32(block, true);
    try {
      return JSON.parse(new TextDecoder().decode(this.bytes.subarray(block + 4, block + 4 + len)));
    } finally { this.m.nether_free(block, len + 4); }
  }
}

export { PAGE };
