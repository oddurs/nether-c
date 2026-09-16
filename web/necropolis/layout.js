// Draw only after the HTML has laid itself out. The controls own navigation;
// removing this file removes no labels, focus targets or relationships.
const stage = document.getElementById("neighbourhood");
const ns = "http://www.w3.org/2000/svg";
const wires = document.createElementNS(ns, "svg");
wires.classList.add("nc-wires");
wires.setAttribute("aria-hidden", "true");
stage.prepend(wires);
let frame;
function draw() {
  cancelAnimationFrame(frame);
  frame = requestAnimationFrame(() => {
    wires.replaceChildren();
    if (matchMedia("(max-width: 760px)").matches) return;
    const bounds = stage.getBoundingClientRect();
    if (!bounds.width) return;
    const selected = document.getElementById("selected").getBoundingClientRect();
    wires.setAttribute("viewBox", `0 0 ${bounds.width} ${bounds.height}`);
    for (const side of ["incoming", "outgoing"]) {
      for (const card of document.querySelectorAll(`#${side} .nc-object`)) {
        const r = card.getBoundingClientRect();
        const left = side === "incoming";
        const x1 = (left ? r.right : selected.right) - bounds.left;
        const y1 = (left ? r.top + r.height / 2 : selected.top + selected.height / 2) - bounds.top;
        const x2 = (left ? selected.left : r.left) - bounds.left;
        const y2 = (left ? selected.top + selected.height / 2 : r.top + r.height / 2) - bounds.top;
        const mid = (x1 + x2) / 2;
        const path = document.createElementNS(ns, "path");
        path.classList.add("nc-wire");
        path.dataset.kind = card.dataset.kind;
        path.setAttribute("d", `M ${x1} ${y1} C ${mid} ${y1}, ${mid} ${y2}, ${x2} ${y2}`);
        wires.append(path);
      }
    }
  });
}
new ResizeObserver(draw).observe(stage);
stage.addEventListener("graphchange", draw);
document.fonts.ready.then(draw);
