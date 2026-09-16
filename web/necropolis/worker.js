import { open } from "./necropolis.js";

self.onmessage = async ({ data: { source } }) => {
  try {
    if (new TextEncoder().encode(source).length > 256 * 1024) throw Error("Source limit: 256 KiB");
    const nether = await open(new URL("nether.wasm", import.meta.url));
    self.postMessage(nether.bury(source));
  } catch (e) {
    self.postMessage({ error: `Burial unavailable: ${e.message}. Build with scripts/task wasm and serve this page over HTTP.` });
  }
};
