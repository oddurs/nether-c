import { Graph, Navigation, PAGE_SIZE, hex, plural, preview, references, short, sourceExcerpt, utf8, utf8Prefix } from "./graph.js";

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
let graph, navigation, worker, timer, copyTimer;
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
      relations(target, edges);
      $("neighbourhood").dispatchEvent(new Event("graphchange"));
      container.querySelectorAll(".nc-object")[firstNew]?.focus();
    }, "nc-more"));
}

function renderBytesInspector(detail, note, object) {
  note(`${object.data.length} bytes. Preview limited to the first 2,048 bytes.`);
  const body = element("pre");
  const sample = object.data.subarray(0, 2048);
  const textSample = utf8Prefix(object.data, 2048);
  let textBtn, hexBtn;
  const showText = () => {
    textBtn.setAttribute("aria-pressed", "true");
    hexBtn.setAttribute("aria-pressed", "false");
    try { body.textContent = utf8.decode(textSample); }
    catch { body.textContent = "This preview is not valid UTF-8. Use hex to inspect the exact bytes."; }
  };
  const showHex = () => {
    textBtn.setAttribute("aria-pressed", "false");
    hexBtn.setAttribute("aria-pressed", "true");
    body.textContent = hex(sample).match(/.{1,32}/g)?.map((line) => line.match(/../g).join(" ")).join("\n") || "(empty)";
  };
  textBtn = button("Text", showText);
  hexBtn = button("Hex", showHex);
  detail.append(textBtn, hexBtn, body);
  showText();
}

function renderSpanExcerpt(detail, note, span) {
  const excerpt = sourceExcerpt(graph, span);
  detail.append(element("h3", "Where this happened"));
  note(`Source ${short(span.source)} · bytes ${span.start}–${span.end}${excerpt ? ` · line ${excerpt.line}` : ""}`);
  if (excerpt) {
    const code = element("pre", undefined, "nc-excerpt");
    code.append(document.createTextNode(excerpt.before), element("mark", excerpt.selected), document.createTextNode(excerpt.after));
    detail.append(code);
  } else note("Source bytes unavailable, or the recorded span is outside them.");
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
  if (object.kind === "Bytes") renderBytesInspector(detail, note, object);
  else if (object.kind === "Str") detail.append(element("pre", object.data.slice(0, 2048) + (object.data.length > 2048 ? "…" : "")));
  else if (object.kind === "Array" || object.kind === "Struct") {
    note(`${object.data.length} entries in declaration order. First 32 shown.`);
    object.data.slice(0, 32).forEach((v, i) => detail.append(element("pre", `${i + 1}. ${v.kind}: ${preview(v, 256)}`)));
  } else if (!object.call) detail.append(element("pre", preview(object, 2048)));
  if (object.span) renderSpanExcerpt(detail, note, object.span);
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
  $("home").disabled = id === graph.root;
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

function commitBurial(data) {
  const next = new Graph(data);
  const nextNav = new Navigation(next.root);
  const summary = `depth ${data.depth} · ${plural(data.holes.length, "hole")} · ${next.encoded.size} objects · ${data.fuel_spent.toLocaleString()} steps`;
  const root = next.get(next.root);
  const prevGraph = graph;
  const prevNav = navigation;
  const prevShown = shown;
  const prevTitle = $("trace-title").textContent;
  const prevSummary = $("summary").textContent;
  const prevRoots = Array.from($("roots").childNodes);
  const prevHidden = $("explorer").hidden;
  graph = next;
  navigation = nextNav;
  shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE };
  try {
    $("trace-title").textContent = `Trace ${short(graph.root)}`;
    $("summary").textContent = summary;
    const frag = document.createDocumentFragment();
    for (const [label, ids] of [["hole", root.holes], ["deposit", root.deposits]]) {
      ids.slice(0, 6).forEach((id, i) => frag.append(button(`${label} ${i + 1}`, () => visit(id), `nc-root-${label}`)));
      if (ids.length > 6) frag.append(button(`All ${ids.length} ${label}s`, () => {
        navigation.visit(graph.root);
        shown = { incoming: PAGE_SIZE, outgoing: Math.min(references(root).length, PAGE_SIZE * 4) };
        render();
        $("selected-title").focus({ preventScroll: true });
      }));
    }
    $("roots").replaceChildren(frag);
    $("explorer").hidden = false;
    render();
  } catch (inner) {
    graph = prevGraph;
    navigation = prevNav;
    shown = prevShown;
    $("trace-title").textContent = prevTitle;
    $("summary").textContent = prevSummary;
    $("roots").replaceChildren(...prevRoots);
    $("explorer").hidden = prevHidden;
    if (prevGraph) render();
    throw inner;
  }
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
    try {
      if (!data || typeof data !== "object") throw new Error("The burial worker returned an invalid response.");
      if (data.error) { failed(data.note ? `${data.error}\n${data.note}` : data.error); return; }
      commitBurial(data);
      stop();
      $("state").textContent = "Buried. Choose a hole, deposit, or reference to explore.";
    } catch (e) { failed(e.message); }
  };
  active.onerror = () => { if (worker === active) failed("The burial worker stopped. Retry or reduce the source."); };
  active.onmessageerror = () => { if (worker === active) failed("The burial worker returned an unreadable message."); };
  timer = setTimeout(() => { if (worker === active) failed("Burial exceeded the browser's 30-second limit. Reduce the source and retry."); }, 30000);
  active.postMessage({ source: $("source").value });
}

function move(delta) {
  if (!navigation) return;
  const before = navigation.position;
  navigation.move(delta);
  if (navigation.position === before) return;
  shown = { incoming: PAGE_SIZE, outgoing: PAGE_SIZE };
  render();
  $("selected-title").focus({ preventScroll: true });
}

$("bury").addEventListener("click", bury);
$("source").addEventListener("keydown", (event) => {
  if ((event.ctrlKey || event.metaKey) && event.key === "Enter") {
    event.preventDefault();
    bury();
  }
});
$("cancel").addEventListener("click", () => { stop(); $("state").textContent = graph ? "Cancelled. The previous trace remains below." : "Cancelled. Bury source to try again."; });
for (const [id, delta] of [["back", -1], ["forward", 1]]) $(id).addEventListener("click", () => move(delta));
document.addEventListener("keydown", (event) => {
  if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
  if (event.target === $("source") || event.target?.closest?.("textarea, input, [contenteditable]")) return;
  if (event.key === "ArrowLeft") { event.preventDefault(); move(-1); }
  else if (event.key === "ArrowRight") { event.preventDefault(); move(1); }
});
$("home").addEventListener("click", () => { if (graph) visit(graph.root); });
$("copy").addEventListener("click", async () => {
  if (!navigation) return;
  try {
    await navigator.clipboard.writeText(navigation.current);
    $("state").textContent = "Full cairn copied.";
    $("copy").textContent = "Copied";
    clearTimeout(copyTimer);
    copyTimer = setTimeout(() => { $("copy").textContent = "Copy full cairn"; }, 1500);
  } catch { $("state").textContent = "Clipboard unavailable. Select and copy the full cairn shown below."; }
});
bury();
