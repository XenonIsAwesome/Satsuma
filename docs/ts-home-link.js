// Imported on every TypeDoc page via `customJs` (see typedoc.json).

// Fixes up the "<- Satsuma Docs" navigationLinks entry's href, which
// TypeDoc renders as the same literal string on every page regardless of
// nesting depth (unlike its own internal links, which it does adjust per
// page). `data-base` on <html> ("./" at the site root, "../" one level
// down, etc.) is TypeDoc's own per-page depth hint, so reusing it here
// keeps the link correct without hardcoding a depth that only some pages
// have.
(function () {
  var base = document.documentElement.getAttribute("data-base") || "./";
  var links = document.querySelectorAll("a.tsd-nav-link");
  for (var i = 0; i < links.length; i++) {
    if (links[i].textContent.indexOf("Satsuma Docs") !== -1) {
      links[i].setAttribute("href", base + "../index.html");
    }
  }
})();

// Keeps TypeDoc's own theme in sync with the other two docs sub-sites
// (rustdoc, the landing page) via one shared localStorage key,
// "satsuma-docs-theme" ("light" | "dark") - see docs/rust-theme-sync.html
// for the rustdoc side of this and why it watches a public attribute
// rather than internal storage keys/functions.
//
// TypeDoc's own theme-init (an inline script at the very top of <body>,
// `document.documentElement.dataset.theme = localStorage.getItem(
// "tsd-theme") || "os"`) has already run by the time this deferred
// script executes, and the page stays hidden (`body{display:none}`,
// TypeDoc's own FOUC guard) until shortly after - so correcting the
// theme here still happens before the page is shown.
(function () {
  try {
    var canonical = localStorage.getItem("satsuma-docs-theme");
    if (canonical === "light" || canonical === "dark") {
      document.documentElement.dataset.theme = canonical;
      localStorage.setItem("tsd-theme", canonical);
    }
  } catch (e) {}

  try {
    new MutationObserver(function () {
      var theme = document.documentElement.dataset.theme;
      if (theme === "light" || theme === "dark") {
        localStorage.setItem("satsuma-docs-theme", theme);
      } else if (theme === "os") {
        localStorage.removeItem("satsuma-docs-theme");
      }
    }).observe(document.documentElement, { attributes: true, attributeFilter: ["data-theme"] });
  } catch (e) {}
})();
