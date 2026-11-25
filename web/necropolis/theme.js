/* Before paint, so nothing flashes. Matches site/template.html without an inline script. */
(function () {
  var root = document.documentElement;
  try {
    if (localStorage.getItem("nc-lamp") === "lit") {
      root.setAttribute("data-lamp", "lit");
    }
  } catch (e) {}

  function bind() {
    var btn = document.querySelector("[data-lamp-toggle]");
    var label = document.querySelector("[data-lamp-state]");
    function paint() {
      var lit = root.getAttribute("data-lamp") === "lit";
      if (label) label.textContent = lit ? "lit" : "out";
      if (btn) btn.setAttribute("aria-pressed", lit ? "true" : "false");
    }
    if (btn) {
      btn.addEventListener("click", function () {
        var next = root.getAttribute("data-lamp") === "lit" ? "out" : "lit";
        root.setAttribute("data-lamp", next);
        try { localStorage.setItem("nc-lamp", next); } catch (e) {}
        paint();
      });
    }
    paint();
  }

  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", bind);
  } else {
    bind();
  }
})();
