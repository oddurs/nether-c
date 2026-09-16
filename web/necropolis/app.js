import { Graph, Navigation, PAGE_SIZE, hex, preview, references, short, sourceExcerpt } from "./graph.js";

const $ = (id) => document.getElementById(id);
const element = (tag, text, className) => {
  const e = document.createElement(tag);
  if (text !== undefined) e.textContent = text;
  if (className) e.className = className;
  return e;
};
const button = (text, action, className) => {
  const b = element("button", text, className);
  b.type = "button";
  b.addEventListener("click", action);
  return b;
};
let graph, navigation, worker, timer;
let shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE };

function visit(id) {
  navigation.visit(id);
  shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE };
  render();
  $("selected-title").focus({ preventScroll: true });
  if (matchMedia("(max-width: 760px)").matches) $("selected").scrollIntoView({ block: "nearest" });
}

function relations(target, edges) {
  const container = $(target);
  container.replaceChildren();
  if (!edges.length) container.append(element("p", "None recorded.", "nc-empty"));
  for (const edge of edges.slice(0, shown[target])) {
    const object = graph.get(edge.id);
    const b = button("", () => visit(edge.id), "nc-object");
    b.dataset.id = edge.id;
    b.dataset.kind = object.kind;
    b.append(element("span", edge.label, "nc-edge-label"),
      element("strong", object.kind), element("code", short(edge.id)),
      element("span", preview(object, 80), "nc-preview"));
    container.append(b);
  }
  if (edges.length > shown[target]) container.append(button(
    `Show ${Math.min(PAGE_SIZE, edges.length - shown[target])} more (${edges.length} total)`,
    () => {
      const firstNew = shown[target];
      shown[target] += PAGE_SIZE;
      render();
      container.querySelectorAll(".nc-object")[firstNew]?.focus();
    }, "nc-more"));
}

function inspect(object) {
  const detail = $("detail");
  detail.replaceChildren();
  const note = (text) => detail.append(element("p", text));
  if (object.kind === "Hole") note(`Unanswered question. ${object.call.function} needs stratum ${object.stratum}. No answer is recorded; this is not an error.`);
  if (object.kind === "Trace") {
    note(`Depth ${object.depth}. ${object.holes.length ? "Unanswered questions remain." : "No holes remain."} ${object.unrecorded ? "UNRECORDED: not replayable." : "No stratum-8 mark."}`);
    note("Open a hole to see what the world still owes, or a deposit to see what happened already.");
  }
  if (object.kind === "Deposit") note("A value the program left behind. Follow its value reference to read it.");
  if (object.call) {
    const args = object.call.args.map((id) => preview(graph.get(id), 120));
    detail.append(element("pre", `${object.call.function}(${args.join(", ")})`));
  }
  if (object.kind === "Bytes") {
    note(`${object.data.length} bytes. Preview limited to the first 2,048 bytes.`);
    const body = element("pre");
    const sample = object.data.subarray(0, 2048);
    const showText = () => {
      try { body.textContent = new TextDecoder("utf-8", { fatal: true }).decode(sample); }
      catch { body.textContent = "This preview is not valid UTF-8. Use hex to inspect the exact bytes."; }
    };
    detail.append(button("Text", showText), button("Hex", () => {
      body.textContent = hex(sample).match(/.{1,32}/g)?.map((line) => line.match(/../g).join(" ")).join("\n") || "(empty)";
    }), body);
    showText();
  } else if (object.kind === "Str") detail.append(element("pre", object.data.slice(0, 2048) + (object.data.length > 2048 ? "…" : "")));
  else if (object.kind === "Array" || object.kind === "Struct") {
    note(`${object.data.length} entries in declaration order. First 32 shown.`);
    object.data.slice(0, 32).forEach((v, i) => detail.append(element("pre", `${i + 1}. ${v.kind}: ${preview(v, 256)}`)));
  } else if (!object.call) detail.append(element("pre", preview(object, 2048)));
  if (object.span) {
    const s = object.span;
    const excerpt = sourceExcerpt(graph, s);
    detail.append(element("h3", "Where this happened"));
    note(`Source ${short(s.source)} · bytes ${s.start}–${s.end}${excerpt ? ` · line ${excerpt.line}` : ""}`);
    if (excerpt) {
      const code = element("pre", undefined, "nc-excerpt");
      code.append(document.createTextNode(excerpt.before), element("mark", excerpt.selected), document.createTextNode(excerpt.after));
      detail.append(code);
    } else note("Source bytes unavailable, or the recorded span is outside them.");
  }
}

function render() {
  const id = navigation.current, object = graph.get(id);
  $("selected").dataset.kind = object.kind;
  $("selected-title").textContent = object.kind;
  $("selected-preview").textContent = preview(object);
  $("identity").textContent = id;
  $("position").textContent = `${navigation.position + 1} / ${navigation.entries.length}`;
  $("back").disabled = navigation.position === 0;
  $("forward").disabled = navigation.position === navigation.entries.length - 1;
  relations("incoming", graph.incoming.get(id) || []);
  relations("outgoing", references(object));
  inspect(object);
  $("neighbourhood").dispatchEvent(new Event("graphchange"));
}

function stop() {
  worker?.terminate(); worker = null;
  clearTimeout(timer);
  $("cancel").hidden = true;
  $("bury").textContent = "Bury source";
}

function failed(message) {
  stop();
  $("diagnostic").textContent = message;
  $("diagnostic").hidden = false;
  $("state").textContent = graph ? "Burial did not finish. The previous trace is still shown below." : "Burial did not finish. Edit the source or retry.";
}

function bury() {
  stop();
  $("diagnostic").hidden = true;
  $("state").textContent = graph ? "Burying… previous trace remains available." : "Burying locally…";
  $("bury").textContent = "Restart burial";
  $("cancel").hidden = false;
  let active;
  try { active = new Worker(new URL("worker.js", import.meta.url), { type: "module" }); }
  catch (e) { failed(e.message); return; }
  worker = active;
  active.onmessage = ({ data }) => {
    if (worker !== active) return;
    if (data.error) { failed(data.error); return; }
    try {
      const next = new Graph(data);
      graph = next;
      navigation = new Navigation(graph.root);
      shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE };
      $("trace-title").textContent = `Trace ${short(graph.root)}`;
      $("summary").textContent = `depth ${data.depth} · ${data.holes.length} hole${data.holes.length === 1 ? "" : "s"} · ${graph.encoded.size} objects · ${data.fuel_spent.toLocaleString()} steps`;
      const root = graph.get(graph.root);
      $("roots").replaceChildren();
      for (const [label, ids] of [["hole", root.holes], ["deposit", root.deposits]]) {
        ids.slice(0, 6).forEach((id, i) => $("roots").append(button(`${label} ${i + 1}`, () => visit(id), `nc-root-${label}`)));
        if (ids.length > 6) $("roots").append(button(`All ${ids.length} ${label}s`, () => visit(graph.root)));
      }
      $("explorer").hidden = false;
      render();
      stop();
      $("state").textContent = "Buried. Choose a hole, deposit, or reference to explore.";
    } catch (e) { failed(e.message); }
  };
  active.onerror = () => { if (worker === active) failed("The burial worker stopped. Retry or reduce the source."); };
  timer = setTimeout(() => { if (worker === active) failed("Burial exceeded the browser's 30-second limit. Reduce the source and retry."); }, 30000);
  active.postMessage({ source: $("source").value });
}

$("bury").addEventListener("click", bury);
$("cancel").addEventListener("click", () => { stop(); $("state").textContent = graph ? "Cancelled. The previous trace remains below." : "Cancelled. Bury source to try again."; });
for (const [id, delta] of [["back", -1], ["forward", 1]]) $(id).addEventListener("click", () => {
  navigation.move(delta); shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE }; render();
});
$("home").addEventListener("click", () => visit(graph.root));
$("copy").addEventListener("click", async () => {
  try { await navigator.clipboard.writeText(navigation.current); $("state").textContent = "Full cairn copied."; }
  catch { $("state").textContent = "Clipboard unavailable. Select and copy the full cairn shown below."; }
});
bury();
