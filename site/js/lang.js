// The language of the site. The first visit to an English page goes to the visitor's language
// when the site has it (navigator.languages, in order); the language links of the footer remember
// the visitor's choice, which wins from then on. Translated pages never move. Without
// JavaScript or storage, nothing moves and the links still work.
(function () {
    var KEY = "vano-lang";
    var SUPPORTED = ["en", "es", "gl", "it"];

    function stored() {
        try {
            return localStorage.getItem(KEY);
        } catch (e) {
            return null;
        }
    }

    function remember(lang) {
        try {
            localStorage.setItem(KEY, lang);
        } catch (e) {
            // Private mode or blocked storage: the choice lasts this page only.
        }
    }

    function preferred() {
        var list = navigator.languages || [navigator.language || ""];
        for (var i = 0; i < list.length; i++) {
            var code = String(list[i]).toLowerCase().split("-")[0];
            if (SUPPORTED.indexOf(code) >= 0) {
                return code;
            }
        }
        return "en";
    }

    var page = document.documentElement.lang;
    var choice = stored();
    if (page === "en" && choice === null) {
        var lang = preferred();
        if (lang !== "en") {
            remember(lang);
            location.replace("/" + lang + location.pathname + location.search + location.hash);
            return;
        }
    }

    document.addEventListener("click", function (event) {
        var link = event.target.closest && event.target.closest("a[data-lang]");
        if (link) {
            remember(link.getAttribute("data-lang"));
        }
    });
})();
