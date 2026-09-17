// Real Chrome, real module worker, real WASM. CDP over an isolated pipe;
// no driver package, no external server, no user browser profile.
import assert from "node:assert/strict";
import { spawn } from "node:child_process";
import { createServer } from "node:http";
import { access, mkdtemp, readFile, rm, writeFile, mkdir } from "node:fs/promises";
import { tmpdir } from "node:os";
import { delimiter, join, resolve, sep } from "node:path";
import { fileURLToPath } from "node:url";

const root = fileURLToPath(new URL("../../", import.meta.url));
const candidates = process.env.NETHER_CHROME ? [process.env.NETHER_CHROME] : [
  "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
  ...process.env.PATH.split(delimiter).flatMap((p) => ["google-chrome", "chromium", "chromium-browser"].map((n) => join(p, n)))
];
let binary;
for (const candidate of candidates) { try { await access(candidate); binary = candidate; break; } catch {} }
assert.ok(binary, "Install Chrome/Chromium or set NETHER_CHROME to its executable");
const profile = await mkdtemp(join(tmpdir(), "nether-browser-"));
let denyWasm = false;
const server = createServer(async (request, response) => {
  try {
    const url = new URL(request.url, "http://localhost");
    const path = resolve(root, "." + decodeURIComponent(url.pathname));
    if (!path.startsWith(resolve(root) + sep)) throw Error("outside root");
    if (denyWasm && path.endsWith(".wasm")) { response.writeHead(503).end(); return; }
    const file = path.endsWith(sep) || url.pathname.endsWith("/") ? join(path, "index.html") : path;
    const type = file.endsWith(".wasm") ? "application/wasm" : file.endsWith(".js") ? "text/javascript" : file.endsWith(".css") ? "text/css" : file.endsWith(".html") ? "text/html" : "application/octet-stream";
    const bytes = await readFile(file);
    response.writeHead(200, { "Content-Type": type, "Cache-Control": "no-store" });
    response.end(bytes);
  } catch { response.writeHead(404).end(); }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const origin = `http://127.0.0.1:${server.address().port}`;
const chrome = spawn(binary, ["--headless=new", "--remote-debugging-pipe", "--no-first-run",
  "--no-default-browser-check", "--disable-background-networking", "--disable-dev-shm-usage",
  ...(process.env.CI || process.getuid?.() === 0 ? ["--no-sandbox"] : []), `--user-data-dir=${profile}`, "about:blank"],
  { stdio: ["ignore", "ignore", "pipe", "pipe", "pipe"] });
let serial = 0, buffer = "", session;
const pending = new Map(), exceptions = [], requests = [];
let stderr = "";
chrome.stderr.on("data", (b) => { stderr = (stderr + b).slice(-4000); });
chrome.stdio[4].on("data", (chunk) => {
  buffer += chunk.toString();
  let end;
  while ((end = buffer.indexOf("\0")) >= 0) {
    const message = JSON.parse(buffer.slice(0, end)); buffer = buffer.slice(end + 1);
    if (message.id && pending.has(message.id)) {
      const { resolve, reject, timer } = pending.get(message.id);
      clearTimeout(timer); pending.delete(message.id);
      if (message.error) reject(Error(JSON.stringify(message.error))); else resolve(message.result);
    } else if (message.method === "Runtime.exceptionThrown") exceptions.push(message.params);
    else if (message.method === "Network.requestWillBeSent") requests.push(message.params.request);
  }
});
function command(method, params = {}, page = true) {
  return new Promise((resolve, reject) => {
    const id = ++serial;
    const timer = setTimeout(() => { pending.delete(id); reject(Error(`Chrome timeout: ${method}\n${stderr}`)); }, 20000);
    pending.set(id, { resolve, reject, timer });
    chrome.stdio[3].write(JSON.stringify({ id, method, params, ...(page && session ? { sessionId: session } : {}) }) + "\0");
  });
}
async function evaluate(expression) {
  const result = await command("Runtime.evaluate", { expression, awaitPromise: true, returnByValue: true });
  if (result.exceptionDetails) throw Error(JSON.stringify(result.exceptionDetails));
  return result.result.value;
}
async function until(expression) {
  const deadline = Date.now() + 15000;
  while (Date.now() < deadline) {
    try { if (await evaluate(expression)) return; } catch (e) {
      if (!/context|Cannot find/.test(e.message)) throw e;
    }
    await new Promise((resolve) => setTimeout(resolve, 50));
  }
  throw Error(`Page timeout: ${expression}\n${await evaluate("document.body.innerText")}`);
}
const click = (selector) => evaluate(`document.querySelector(${JSON.stringify(selector)}).click()`);
const text = (id) => evaluate(`document.getElementById(${JSON.stringify(id)}).textContent`);
async function bury(source) {
  await evaluate(`document.querySelector('.nc-source').open = true; document.getElementById('source').value = ${JSON.stringify(source)}; document.getElementById('bury').click()`);
  await until("document.getElementById('state').textContent.startsWith('Buried.') || !document.getElementById('diagnostic').hidden");
  assert.equal(await evaluate("document.getElementById('diagnostic').hidden"), true, await text("diagnostic"));
}
async function screenshot(name, selector) {
  if (!process.env.NETHER_SCREENSHOTS) return;
  await mkdir(process.env.NETHER_SCREENSHOTS, { recursive: true });
  const { cssContentSize: size } = await command("Page.getLayoutMetrics");
  const clip = selector ? await evaluate(`(() => {
    const r = document.querySelector(${JSON.stringify(selector)}).getBoundingClientRect();
    return { x: r.x + scrollX, y: r.y + scrollY, width: r.width, height: r.height, scale: 1 };
  })()`) : { x: 0, y: 0, width: size.width, height: Math.min(size.height, 3000), scale: 1 };
  const { data } = await command("Page.captureScreenshot", { format: "png", captureBeyondViewport: true,
    clip });
  await writeFile(join(process.env.NETHER_SCREENSHOTS, name + ".png"), Buffer.from(data, "base64"));
}
try {
  const { targetId } = await command("Target.createTarget", { url: "about:blank" }, false);
  ({ sessionId: session } = await command("Target.attachToTarget", { targetId, flatten: true }, false));
  await command("Page.enable"); await command("Runtime.enable"); await command("Network.enable");
  await command("Page.bringToFront");
  await command("Emulation.setFocusEmulationEnabled", { enabled: true });
  for (const width of [1248, 768, 390, 320]) {
    await command("Emulation.setDeviceMetricsOverride", { width, height: 1000, deviceScaleFactor: 1, mobile: width < 500 });
    for (const path of ["/site/", "/site/spec/00-overview.html"]) {
      await command("Page.navigate", { url: origin + path });
      await until("document.readyState === 'complete' && location.pathname === " + JSON.stringify(path));
      await evaluate("document.fonts.ready");
      assert.equal(await evaluate('document.fonts.check(\'16px "Nether 8"\')'), true);
      assert.ok(await evaluate("document.documentElement.scrollWidth <= innerWidth"), path + " overflows at " + width);
      assert.ok(await evaluate("[...document.images].every(i => i.complete && i.naturalWidth > 0)"), "missing site graphic");
      assert.equal(await evaluate("document.querySelector('.directory-drawer').open"), width > 1050);
      await evaluate("document.querySelector('.directory-drawer summary').focus()");
      await command("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", text: "\r" });
      await command("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter" });
      assert.equal(await evaluate("document.querySelector('.directory-drawer').open"), width <= 1050);
      await click(".directory-drawer summary");
      await click("[data-lamp-toggle]");
      assert.equal(await evaluate("document.documentElement.dataset.lamp"), "lit");
      assert.equal(await evaluate("document.querySelector('[data-lamp-toggle]').getAttribute('aria-pressed')"), "true");
      await until("[...document.images].every(i => i.complete && i.naturalWidth > 0)");
      assert.ok(await evaluate("[...document.querySelectorAll('img[data-lit-src]')].every(i => i.getAttribute('src') === i.dataset.litSrc)"), "graphics follow the lamp");
      if (width === 1248 && path === "/site/") await screenshot("site-lamp-lit");
      await click("[data-lamp-toggle]");
      await until("[...document.images].every(i => i.complete && i.naturalWidth > 0)");
      assert.deepEqual(await evaluate(`(() => {
        const c = document.createElement("canvas").getContext("2d");
        return [8, 16, 24, 32].map(size => {
          c.font = size + 'px "Nether 8"';
          return c.measureText(".").actualBoundingBoxLeft === -size / 4
            && ["W", "i", ".", " ", "■", "▸"].every(ch => c.measureText(ch).width === size * 7 / 8);
        });
      })()`), [true, true, true, true], "fixed advance at every pixel size");
      // 0231: the bold is drawn, so it is a face the browser has rather than
      // one it invents. A synthetic bold is wider, and on a site made of
      // columns a wider bold moves every listing wherever a keyword is.
      assert.equal(await evaluate('document.fonts.check(\'bold 16px "Nether 8"\')'), true, "the bold face is loaded");
      assert.equal(await evaluate("getComputedStyle(document.body).fontSynthesis"), "none", "synthesis is off");
      assert.deepEqual(await evaluate(`(() => {
        const c = document.createElement("canvas").getContext("2d");
        const text = "MUST bury 0x1f — strata@8";
        return [8, 16, 24, 32].map(size => {
          c.font = size + 'px "Nether 8"';
          const plain = c.measureText(text).width;
          c.font = 'bold ' + size + 'px "Nether 8"';
          return c.measureText(text).width === plain && plain === text.length * size * 7 / 8;
        });
      })()`), [true, true, true, true], "a bold run measures what the same text measures unbolded");
      assert.equal(await evaluate(`(() => {
        const one = document.createElement("span"), two = document.createElement("strong");
        one.textContent = two.textContent = "bury the whole of it 0123";
        for (const el of [one, two]) { el.style.position = "absolute"; el.style.whiteSpace = "pre"; document.body.append(el); }
        const same = one.getBoundingClientRect().width === two.getBoundingClientRect().width;
        one.remove(); two.remove();
        return same;
      })()`), true, "a strong run is the same width in the page");
      if (path.includes("/spec/")) {
        assert.equal(await evaluate("document.querySelectorAll('nav.contents li').length"), 12);
        assert.equal(await evaluate("document.querySelectorAll('nav.contents [aria-current=page]').length"), 1);
        assert.equal(await evaluate("document.querySelectorAll('nav.contents a').length"), 11);
      } else {
        assert.equal(await evaluate("document.querySelector('img.hero').naturalWidth"), 600);
        assert.ok(await evaluate("[...document.querySelectorAll('.swatches > div')].every(s => getComputedStyle(s, '::before').height === '32px' && getComputedStyle(s, '::before').backgroundColor !== 'rgba(0, 0, 0, 0)')"), "swatches show the live tokens");
        await command("Emulation.setEmulatedMedia", { features: [{ name: "prefers-reduced-motion", value: "reduce" }] });
        await until("document.querySelector('img.hero').getAttribute('src').endsWith('-still.gif') && [...document.images].every(i => i.complete && i.naturalWidth > 0)");
        assert.match(await evaluate("document.querySelector('img.descent').getAttribute('src')"), /-still.gif$/);
        if (width === 1248) await screenshot("strata-dark", "img.descent");
        await click("[data-lamp-toggle]");
        await until("document.querySelector('img.hero').getAttribute('src').endsWith('-lit-still.gif') && [...document.images].every(i => i.complete && i.naturalWidth > 0)");
        if (width === 1248) {
          await screenshot("strata-light", "img.descent");
          await screenshot("palette-light", ".swatches");
        }
        await click("[data-lamp-toggle]");
        await command("Emulation.setEmulatedMedia", { features: [] });
        await until("document.querySelector('img.hero').getAttribute('src').endsWith('/hero.gif') && [...document.images].every(i => i.complete && i.naturalWidth > 0)");
        assert.ok(await evaluate("[...document.querySelectorAll('.desk-directory a')].every(a => document.getElementById(a.hash.slice(1)))"));
        await click(".complaints summary");
        assert.equal(await evaluate("document.querySelector('.complaints').open"), true);
        assert.match(await evaluate("document.querySelector('.complaints').textContent"), /No complaint was sent/);
        await click(".complaints summary");
      }
      await screenshot((path.includes("/spec/") ? "spec-" : "site-") + width);
      if (path.includes("/spec/")) {
        await evaluate("document.querySelector('.directory-drawer').open = true");
        await click('nav.contents a[href="02-calculus.html"]');
        await until("location.pathname === '/site/spec/02-calculus.html' && document.readyState === 'complete'");
        assert.match(await evaluate("document.querySelector('nav.contents [aria-current]').textContent"), /calculus/);
      } else {
        await click('.arrival-actions a[href="#first-program"]');
        assert.equal(await evaluate("location.hash"), "#first-program");
      }
    }
  }
  await command("Emulation.setScriptExecutionDisabled", { value: true });
  await command("Page.navigate", { url: origin + "/site/spec/00-overview.html" });
  await until("location.pathname === '/site/spec/00-overview.html' && document.readyState === 'complete'");
  assert.equal(await evaluate("document.querySelector('.directory-drawer').open"), true, "directory works without JavaScript");
  await command("Emulation.setScriptExecutionDisabled", { value: false });
  console.log("browser: site graphics, spec contents and fixed font metrics at desktop/mobile sizes");
  await command("Emulation.setDeviceMetricsOverride", { width: 1248, height: 1000, deviceScaleFactor: 1, mobile: false });
  await command("Page.navigate", { url: origin + "/web/necropolis/" });
  await until("document.getElementById('state')?.textContent.startsWith('Buried.')");
  await evaluate("document.fonts.ready");
  assert.match(await text("summary"), /1 hole/);
  assert.equal(await text("selected-title"), "Trace");
  await until("document.querySelectorAll('.nc-wire').length > 0");
  await screenshot("trace-desktop");
  console.log("browser: real worker burial and desktop graph");

  const rootId = await text("identity");
  await click(".nc-root-hole");
  assert.equal(await text("selected-title"), "Hole");
  assert.match(await text("detail"), /read\("main.nc"\)/);
  assert.match(await evaluate("document.querySelector('.nc-excerpt mark').textContent"), /read/);
  const holeId = await text("identity");
  await screenshot("hole-desktop");
  await click('#outgoing [data-kind="Str"]');
  assert.match(await text("detail"), /main.nc/);
  await evaluate("document.getElementById('back').focus()");
  assert.equal(await evaluate("document.activeElement.id"), "back");
  await command("Input.dispatchKeyEvent", { type: "keyDown", key: "Enter", code: "Enter", text: "\r", unmodifiedText: "\r", windowsVirtualKeyCode: 13, nativeVirtualKeyCode: 13 });
  await command("Input.dispatchKeyEvent", { type: "keyUp", key: "Enter", code: "Enter", windowsVirtualKeyCode: 13 });
  await until(`document.getElementById('identity').textContent === ${JSON.stringify(holeId)}`);
  assert.equal(await text("identity"), holeId);
  await click("#forward"); assert.equal(await text("selected-title"), "Str");
  await click("#home"); assert.equal(await text("identity"), rootId);
  await click(".nc-root-deposit"); await click('#outgoing [data-kind="Bytes"]');
  assert.match(await text("detail"), /a deposit that happened/);
  console.log("browser: hole, argument, source span, deposit and keyboard history");

  await command("Emulation.setDeviceMetricsOverride", { width: 390, height: 844, deviceScaleFactor: 1, mobile: true });
  await click(".nc-root-hole");
  assert.ok(await evaluate("document.documentElement.scrollWidth <= innerWidth"), "mobile overflows horizontally");
  assert.equal(await evaluate("getComputedStyle(document.querySelector('.nc-wires')).display"), "none");
  assert.equal(await evaluate("getComputedStyle(document.getElementById('selected')).order"), "-1");
  await screenshot("hole-mobile");
  console.log("browser: 390px layout, same relationships, no horizontal overflow");

  await bury('U0 f() { "é雪"; "<img src=x onerror=alert(1)>"; } demand f();');
  await click(".nc-root-deposit:nth-child(2)");
  await click('#outgoing [data-kind="Str"]');
  assert.match(await text("detail"), /<img src=x/);
  assert.equal(await evaluate("document.querySelectorAll('#detail img').length"), 0);
  console.log("browser: hostile source and values are text, never markup");

  await bury(`U0 f() { ${Array.from({ length: 30 }, (_, i) => `${i};`).join(" ")} } demand f();`);
  assert.equal(await evaluate("document.querySelectorAll('#outgoing .nc-object').length"), 12);
  await click("#outgoing .nc-more");
  assert.equal(await evaluate("document.querySelectorAll('#outgoing .nc-object').length"), 24);
  assert.equal(await evaluate("document.activeElement.className"), "nc-object");
  await click("#outgoing .nc-more");
  assert.equal(await evaluate("document.querySelectorAll('#outgoing .nc-object').length"), 32);
  console.log("browser: large neighbourhoods expand in bounded steps and retain focus");

  const previous = await text("trace-title");
  await evaluate("document.getElementById('source').value = 'demand missing;'; document.getElementById('bury').click()");
  await until("!document.getElementById('diagnostic').hidden");
  assert.equal(await text("trace-title"), previous);
  assert.match(await text("state"), /previous trace/);
  await evaluate("document.getElementById('source').value = 'demand 1;'; document.getElementById('bury').click(); document.getElementById('cancel').click()");
  assert.match(await text("state"), /Cancelled/);
  assert.equal(await text("trace-title"), previous);
  await evaluate("document.getElementById('source').value = 'demand 1;'; document.getElementById('bury').click(); document.getElementById('source').value = 'demand 2;'; document.getElementById('bury').click()");
  await until("document.getElementById('state').textContent.startsWith('Buried.')");
  await click('#outgoing .nc-object');
  assert.match(await text("detail"), /demand 2;/);
  console.log("browser: failure preserves the prior trace; cancel/restart discards stale work");

  await bury("");
  assert.match(await text("summary"), /0 holes/);
  assert.equal(await evaluate("document.querySelectorAll('#roots button').length"), 0);
  denyWasm = true;
  await click("#bury");
  await until("!document.getElementById('diagnostic').hidden");
  assert.match(await text("diagnostic"), /Burial unavailable/);
  denyWasm = false;
  await bury("demand 3;");
  assert.deepEqual(exceptions, [], "uncaught browser exceptions");
  assert.ok(requests.every((r) => r.url.startsWith(origin) || r.url.startsWith("data:")), "page made an external request");
  assert.ok(requests.every((r) => r.method === "GET"), "page sent data");
  console.log("browser: empty trace, module-load failure and recovery; no external requests");
} finally {
  for (const { timer } of pending.values()) clearTimeout(timer);
  chrome.kill();
  await new Promise((resolve) => { if (chrome.exitCode !== null) resolve(); else chrome.once("exit", resolve); });
  await new Promise((resolve) => server.close(resolve));
  // Chrome's helpers can finish profile writes after the parent exits.
  // Retry transient directory races, but still fail if cleanup cannot finish.
  await rm(profile, { recursive: true, force: true, maxRetries: 5, retryDelay: 100 });
}
