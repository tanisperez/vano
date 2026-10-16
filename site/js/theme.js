// Light and dark theme. The page follows the system until the visitor picks a theme with the
// header button. The choice is kept in localStorage, and dropped again when it matches the
// system. Loaded from <head>, before the body renders, so the page never flashes.
(function () {
    var root = document.documentElement;
    var system = window.matchMedia("(prefers-color-scheme: dark)");
    var choice = null;

    try {
        choice = localStorage.getItem("theme");
    } catch (e) {}

    function apply() {
        var theme = choice || (system.matches ? "dark" : "light");
        root.setAttribute("data-theme", theme);

        var color = theme === "dark" ? "#15161a" : "#fcfcfd";
        document.querySelectorAll('meta[name="theme-color"]').forEach(function (meta) {
            meta.setAttribute("content", color);
        });
        document.querySelectorAll(".theme-toggle").forEach(function (button) {
            button.setAttribute("aria-pressed", theme === "dark" ? "true" : "false");
        });
    }

    apply();
    system.addEventListener("change", apply);
    document.addEventListener("DOMContentLoaded", apply);

    document.addEventListener("click", function (event) {
        if (!event.target.closest(".theme-toggle")) {
            return;
        }
        var next = root.getAttribute("data-theme") === "dark" ? "light" : "dark";
        choice = next === (system.matches ? "dark" : "light") ? null : next;
        try {
            if (choice) {
                localStorage.setItem("theme", choice);
            } else {
                localStorage.removeItem("theme");
            }
        } catch (e) {}
        apply();
    });
})();
