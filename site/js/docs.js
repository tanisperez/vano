// Documentation pages: search, the "On this page" list, heading anchors, copy buttons, syntax
// highlighting and the mobile navigation toggle. Plain JavaScript, no dependencies. Every page
// still reads fine without it: the navigation stays open and code stays plain text.
(function () {
    "use strict";

    var article = document.querySelector(".article");

    // Mobile navigation

    function setupMenu() {
        var docs = document.querySelector(".docs");
        var button = document.querySelector(".docs-menu");
        if (!docs || !button) {
            return;
        }
        button.addEventListener("click", function () {
            var open = docs.classList.toggle("nav-open");
            button.setAttribute("aria-expanded", open ? "true" : "false");
        });
    }

    // Heading anchors and the "On this page" list

    function setupHeadings() {
        if (!article) {
            return;
        }
        var headings = Array.prototype.slice.call(article.querySelectorAll("h2[id], h3[id]"));
        headings.concat(Array.prototype.slice.call(article.querySelectorAll("h4[id]")))
            .forEach(function (heading) {
                var anchor = document.createElement("a");
                anchor.className = "anchor";
                anchor.href = "#" + heading.id;
                anchor.setAttribute("aria-label", "Link to this section");
                anchor.textContent = "#";
                heading.appendChild(anchor);
            });

        var toc = document.querySelector(".docs-toc");
        if (!toc || headings.length < 2) {
            return;
        }
        var title = document.createElement("h2");
        title.textContent = "On this page";
        var list = document.createElement("ul");
        var links = headings.map(function (heading) {
            var item = document.createElement("li");
            item.className = heading.tagName.toLowerCase();
            var link = document.createElement("a");
            link.href = "#" + heading.id;
            link.textContent = headingText(heading);
            item.appendChild(link);
            list.appendChild(item);
            return link;
        });
        toc.appendChild(title);
        toc.appendChild(list);

        // Highlights the last heading that scrolled past the top. Runs only on scroll events, so
        // an idle page does no work.
        var pending = false;
        function update() {
            pending = false;
            var current = 0;
            for (var i = 0; i < headings.length; i++) {
                if (headings[i].getBoundingClientRect().top < 120) {
                    current = i;
                } else {
                    break;
                }
            }
            links.forEach(function (link, i) {
                link.classList.toggle("active", i === current);
            });
        }
        window.addEventListener(
            "scroll",
            function () {
                if (!pending) {
                    pending = true;
                    window.requestAnimationFrame(update);
                }
            },
            { passive: true }
        );
        update();
    }

    // The text of a heading without its "#" anchor.
    function headingText(heading) {
        var text = "";
        heading.childNodes.forEach(function (node) {
            if (!(node.classList && node.classList.contains("anchor"))) {
                text += node.textContent;
            }
        });
        return text.trim();
    }

    // Copy buttons

    function setupCopy() {
        if (!article || !navigator.clipboard) {
            return;
        }
        article.querySelectorAll(".code").forEach(function (block) {
            var pre = block.querySelector("pre");
            if (!pre) {
                return;
            }
            var button = document.createElement("button");
            button.type = "button";
            button.className = "copy-code";
            button.textContent = "Copy";
            button.addEventListener("click", function () {
                // Shell prompts are not part of the command.
                var clone = pre.cloneNode(true);
                clone.querySelectorAll(".prompt").forEach(function (prompt) {
                    prompt.remove();
                });
                navigator.clipboard.writeText(clone.textContent).then(function () {
                    button.textContent = "Copied";
                    setTimeout(function () {
                        button.textContent = "Copy";
                    }, 1500);
                });
            });
            block.appendChild(button);
        });
    }

    // Syntax highlighting. A small tokenizer per language, good enough for the examples in these
    // pages. Blocks opt in with data-lang on the <pre>.

    function escapeHtml(text) {
        return text.replace(/[&<>"]/g, function (c) {
            return { "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" }[c];
        });
    }

    // Splits `text` with an ordered list of [class, regex source] rules and wraps every match in
    // a span of that class. Text no rule matches is escaped and kept as is.
    function tokenize(text, rules) {
        var pattern = new RegExp(
            rules
                .map(function (rule) {
                    return "(" + rule[1] + ")";
                })
                .join("|"),
            "g"
        );
        var out = "";
        var last = 0;
        var match;
        while ((match = pattern.exec(text)) !== null) {
            if (match[0] === "") {
                pattern.lastIndex++;
                continue;
            }
            out += escapeHtml(text.slice(last, match.index));
            for (var i = 0; i < rules.length; i++) {
                if (match[i + 1] !== undefined) {
                    var cls = rules[i][0];
                    var html = escapeHtml(match[0]);
                    out += cls ? '<span class="' + cls + '">' + html + "</span>" : html;
                    break;
                }
            }
            last = pattern.lastIndex;
        }
        return out + escapeHtml(text.slice(last));
    }

    var VAR = "\\{\\{[^{}\\n]*\\}\\}";

    var JS_RULES = [
        ["cm", "\\/\\/[^\\n]*|\\/\\*[\\s\\S]*?\\*\\/"],
        ["str", "\"(?:[^\"\\\\\\n]|\\\\.)*\"|'(?:[^'\\\\\\n]|\\\\.)*'|`(?:[^`\\\\]|\\\\.)*`"],
        ["kw", "\\b(?:const|let|var|function|return|if|else|for|while|of|in|new|throw|try|catch|typeof|true|false|null|undefined)\\b"],
        ["num", "\\b\\d+(?:\\.\\d+)?\\b"],
        ["fn", "\\b[A-Za-z_$][\\w$]*(?=\\s*\\()"],
    ];

    var JSON_RULES = [
        ["var", VAR],
        ["hd", "\"(?:[^\"\\\\\\n]|\\\\.)*\"(?=\\s*:)"],
        ["str", "\"(?:[^\"\\\\\\n]|\\\\.)*\""],
        ["num", "-?\\b\\d+(?:\\.\\d+)?(?:[eE][+-]?\\d+)?\\b"],
        ["kw", "\\b(?:true|false|null)\\b"],
    ];

    var LANGS = {
        js: function (text) {
            return tokenize(text, JS_RULES);
        },
        json: function (text) {
            return tokenize(text, JSON_RULES);
        },
        env: function (text) {
            return text
                .split("\n")
                .map(function (line) {
                    if (/^\s*#/.test(line)) {
                        return '<span class="cm">' + escapeHtml(line) + "</span>";
                    }
                    var eq = line.indexOf("=");
                    if (eq < 0) {
                        return escapeHtml(line);
                    }
                    return '<span class="hd">' + escapeHtml(line.slice(0, eq)) + "</span>=" +
                        tokenize(line.slice(eq + 1), [["var", VAR]]);
                })
                .join("\n");
        },
        toml: function (text) {
            return text
                .split("\n")
                .map(function (line) {
                    if (/^\s*#/.test(line)) {
                        return '<span class="cm">' + escapeHtml(line) + "</span>";
                    }
                    if (/^\s*\[/.test(line)) {
                        return '<span class="sec">' + escapeHtml(line) + "</span>";
                    }
                    var eq = line.indexOf("=");
                    if (eq < 0) {
                        return escapeHtml(line);
                    }
                    return '<span class="hd">' + escapeHtml(line.slice(0, eq)) + "</span>" +
                        tokenize(line.slice(eq), [
                            ["str", "\"(?:[^\"\\\\\\n]|\\\\.)*\""],
                            ["num", "\\b\\d+(?:\\.\\d+)?\\b"],
                            ["kw", "\\b(?:true|false)\\b"],
                        ]);
                })
                .join("\n");
        },
        shell: function (text) {
            return text
                .split("\n")
                .map(function (line) {
                    if (/^\s*#/.test(line)) {
                        return '<span class="cm">' + escapeHtml(line) + "</span>";
                    }
                    var prompt = /^(\$|>) /.exec(line);
                    if (prompt) {
                        return '<span class="prompt">' + escapeHtml(prompt[0]) + "</span>" +
                            tokenize(line.slice(2), [["str", "\"[^\"\\n]*\"|'[^'\\n]*'"], ["var", "\\$[A-Z_]+|%[A-Z_]+%"]]);
                    }
                    return escapeHtml(line);
                })
                .join("\n");
        },
    };

    function setupHighlighting() {
        document.querySelectorAll("pre[data-lang]").forEach(function (pre) {
            var highlight = LANGS[pre.getAttribute("data-lang")];
            if (highlight) {
                pre.innerHTML = highlight(pre.textContent);
            }
        });
    }

    // Search. The index is built in the browser the first time the dialog opens: every page in
    // the navigation is fetched once and split into sections at its h2 and h3 headings.

    var dialog = null;
    var input = null;
    var results = null;
    var index = null;
    var indexing = null;
    var selected = -1;

    function pagesFromNav() {
        var seen = {};
        var pages = [];
        document.querySelectorAll(".docs-nav a[href]").forEach(function (link) {
            var url = link.href.split("#")[0];
            if (!seen[url]) {
                seen[url] = true;
                pages.push(url);
            }
        });
        return pages;
    }

    // The text of an element with a space after every cell, item and paragraph, so table cells
    // and list items do not run into each other.
    function spacedText(element) {
        var clone = element.cloneNode(true);
        clone.querySelectorAll("td, th, li, dt, dd, p").forEach(function (block) {
            block.appendChild(block.ownerDocument.createTextNode(" "));
        });
        return clone.textContent;
    }

    function sectionsOf(doc, url) {
        var root = doc.querySelector(".article");
        if (!root) {
            return [];
        }
        var h1 = root.querySelector("h1");
        var pageTitle = h1 ? h1.textContent.trim() : doc.title;
        var sections = [];
        var current = { page: pageTitle, url: url, heading: pageTitle, text: "", top: true };
        sections.push(current);

        function walk(node) {
            node.childNodes.forEach(function (child) {
                if (child.nodeType === 1) {
                    if (/^H[23]$/.test(child.tagName) && child.id) {
                        current = {
                            page: pageTitle,
                            url: url + "#" + child.id,
                            heading: child.textContent.trim(),
                            text: "",
                            top: false,
                        };
                        sections.push(current);
                        return;
                    }
                    if (child.tagName === "H1" || child.classList.contains("pager") ||
                        child.classList.contains("breadcrumb") || child.classList.contains("page-meta")) {
                        return;
                    }
                    if (/^(SECTION|DIV|ARTICLE|FIGURE)$/.test(child.tagName) &&
                        !child.classList.contains("code") && !child.classList.contains("table")) {
                        walk(child);
                        return;
                    }
                    current.text += " " + spacedText(child);
                }
            });
        }
        walk(root);
        sections.forEach(function (section) {
            section.text = section.text.replace(/\s+/g, " ").trim();
            section.lower = (section.heading + " " + section.text).toLowerCase();
        });
        return sections;
    }

    function buildIndex() {
        if (indexing) {
            return indexing;
        }
        var parser = new DOMParser();
        indexing = Promise.all(
            pagesFromNav().map(function (url) {
                return fetch(url)
                    .then(function (response) {
                        return response.ok ? response.text() : "";
                    })
                    .then(function (html) {
                        return html ? sectionsOf(parser.parseFromString(html, "text/html"), url) : [];
                    })
                    .catch(function () {
                        return [];
                    });
            })
        ).then(function (lists) {
            index = [].concat.apply([], lists);
            return index;
        });
        return indexing;
    }

    function search(query) {
        var terms = query.toLowerCase().split(/\s+/).filter(Boolean);
        if (terms.length === 0 || !index) {
            return [];
        }
        var phrase = terms.join(" ");
        var hits = [];
        index.forEach(function (section, order) {
            var heading = section.heading.toLowerCase();
            var page = section.page.toLowerCase();
            var score = 0;
            for (var i = 0; i < terms.length; i++) {
                var term = terms[i];
                if (section.lower.indexOf(term) < 0 && page.indexOf(term) < 0) {
                    return;
                }
                if (heading.indexOf(term) >= 0) {
                    score += heading.indexOf(term) === 0 ? 14 : 10;
                }
                if (page.indexOf(term) >= 0) {
                    score += 4;
                }
                score += Math.min(count(section.lower, term), 5);
            }
            if (heading.indexOf(phrase) >= 0) {
                score += 20;
            }
            if (section.top && page.indexOf(phrase) >= 0) {
                score += 10;
            }
            hits.push({ section: section, score: score, order: order });
        });
        hits.sort(function (a, b) {
            return b.score - a.score || a.order - b.order;
        });
        return hits.slice(0, 40).map(function (hit) {
            return hit.section;
        });
    }

    function count(text, term) {
        var n = 0;
        var at = text.indexOf(term);
        while (at >= 0) {
            n++;
            at = text.indexOf(term, at + term.length);
        }
        return n;
    }

    function escapeRegExp(text) {
        return text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
    }

    function mark(text, terms) {
        if (terms.length === 0) {
            return escapeHtml(text);
        }
        var pattern = new RegExp("(" + terms.map(escapeRegExp).join("|") + ")", "gi");
        return text
            .split(pattern)
            .map(function (part, i) {
                return i % 2 === 1 ? "<mark>" + escapeHtml(part) + "</mark>" : escapeHtml(part);
            })
            .join("");
    }

    // About 140 characters of the section around the first term that appears in its text.
    function excerpt(section, terms) {
        var text = section.text;
        var lower = text.toLowerCase();
        var at = -1;
        for (var i = 0; i < terms.length && at < 0; i++) {
            at = lower.indexOf(terms[i]);
        }
        if (at < 0) {
            return text.slice(0, 140);
        }
        var start = Math.max(0, at - 40);
        var snippet = text.slice(start, start + 140);
        return (start > 0 ? "…" : "") + snippet;
    }

    function render() {
        var query = input.value.trim();
        results.innerHTML = "";
        selected = -1;
        if (!query) {
            results.innerHTML = '<li class="search-empty">Search the Vano documentation.</li>';
            return;
        }
        if (!index) {
            results.innerHTML = '<li class="search-empty">Loading the index…</li>';
            buildIndex().then(render);
            return;
        }
        var terms = query.toLowerCase().split(/\s+/).filter(Boolean);
        var hits = search(query);
        if (hits.length === 0) {
            var empty = document.createElement("li");
            empty.className = "search-empty";
            empty.textContent = "No results for \u201c" + query + "\u201d";
            results.appendChild(empty);
            return;
        }

        // Grouped by page, pages in the order of their best hit.
        var groups = [];
        var byPage = {};
        hits.forEach(function (hit) {
            if (!byPage[hit.page]) {
                byPage[hit.page] = [];
                groups.push(hit.page);
            }
            byPage[hit.page].push(hit);
        });
        groups.forEach(function (page) {
            var label = document.createElement("li");
            label.className = "group";
            label.setAttribute("role", "presentation");
            label.textContent = page;
            results.appendChild(label);
            byPage[page].forEach(function (hit) {
                var item = document.createElement("li");
                var link = document.createElement("a");
                link.href = hit.url;
                link.setAttribute("role", "option");
                link.innerHTML =
                    '<span class="heading">' + mark(hit.heading, terms) + "</span>" +
                    '<span class="excerpt">' + mark(excerpt(hit, terms), terms) + "</span>";
                link.addEventListener("click", function () {
                    dialog.close();
                });
                item.appendChild(link);
                results.appendChild(item);
            });
        });
        select(0);
    }

    function options() {
        return results.querySelectorAll("a[role=option]");
    }

    function select(i) {
        var links = options();
        if (links.length === 0) {
            return;
        }
        selected = (i + links.length) % links.length;
        links.forEach(function (link, n) {
            link.setAttribute("aria-selected", n === selected ? "true" : "false");
        });
        links[selected].scrollIntoView({ block: "nearest" });
    }

    function openSearch() {
        if (!dialog || dialog.open) {
            return;
        }
        dialog.showModal();
        input.select();
        render();
        buildIndex();
    }

    function setupSearch() {
        dialog = document.querySelector(".search-dialog");
        if (!dialog || typeof dialog.showModal !== "function") {
            return;
        }
        input = dialog.querySelector("input");
        results = dialog.querySelector(".search-results");

        document.querySelectorAll(".search-trigger").forEach(function (button) {
            button.addEventListener("click", openSearch);
        });
        // The shortcut label follows the platform.
        if (/Mac|iPhone|iPad/.test(navigator.platform)) {
            document.querySelectorAll(".search-trigger kbd").forEach(function (kbd) {
                kbd.textContent = "\u2318 K";
            });
        }

        input.addEventListener("input", render);
        input.addEventListener("keydown", function (event) {
            if (event.key === "ArrowDown") {
                event.preventDefault();
                select(selected + 1);
            } else if (event.key === "ArrowUp") {
                event.preventDefault();
                select(selected - 1);
            } else if (event.key === "Enter") {
                var links = options();
                if (links[selected]) {
                    event.preventDefault();
                    links[selected].click();
                }
            }
        });
        // A click on the backdrop closes the dialog.
        dialog.addEventListener("click", function (event) {
            if (event.target === dialog) {
                dialog.close();
            }
        });

        document.addEventListener("keydown", function (event) {
            var typing = /^(INPUT|TEXTAREA|SELECT)$/.test(document.activeElement.tagName);
            if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
                event.preventDefault();
                openSearch();
            } else if (event.key === "/" && !typing) {
                event.preventDefault();
                openSearch();
            }
        });
    }

    setupMenu();
    setupHighlighting();
    setupHeadings();
    setupCopy();
    setupSearch();
})();
