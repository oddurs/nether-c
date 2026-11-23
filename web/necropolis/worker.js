import { open } from "./necropolis.js";

const encoder = new TextEncoder();

self.onmessage = async (event) => {
  const source = event?.data?.source;
  if (typeof source !== "string") {
    self.postMessage({ error: "No source text was provided." });
    return;
  }
  if (encoder.encode(source).length > 256 * 1024) {
    self.postMessage({ error: "Source limit: 256 KiB" });
    return;
  }
  let nether;
  try {
    nether = await open(new URL("nether.wasm", import.meta.url));
  } catch (e) {
    self.postMessage({ error: `Burial unavailable: ${e.message}. Build with scripts/task wasm and serve this page over HTTP.` });
    return;
  }
  try {
    self.postMessage(nether.bury(source));
  } catch (e) {
    self.postMessage({ error: `Burial failed: ${e.message}` });
  }
};
