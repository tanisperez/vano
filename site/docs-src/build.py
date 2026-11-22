#!/usr/bin/env python3
"""Builds the user documentation: wraps every page of pages/ in template.html and writes the
result to site/docs/. Standard library only, so it runs anywhere Python 3 does.

    python3 site/docs-src/build.py [OUTPUT_DIR]

Page format: a header of `key: value` lines (`title`, `description`), a `---` line, then the
page body in HTML. Inside the body:

- A fenced block, ```lang title="file name" ... ```, becomes a code panel with its content
  escaped. `lang` is one of the languages js/docs.js highlights (js, json, env, toml, shell) or
  `text`. The fences must start at the beginning of a line.
- An h2, h3 or h4 without an id gets one from its text, so it can be linked and searched.

The build fails, listing every problem, when a page in NAV has no source file, a source file is
not in NAV, an internal link or anchor does not exist, or a page contains an em dash.

It also writes two files at the root of the site, from NAV and the page headers, so they never
fall behind the pages: sitemap.xml (the home page, the install page and every docs page) and
llms.txt (llms.txt of this folder, with the list of docs pages filled in).
"""
import html
import json
import re
import sys
from pathlib import Path
from string import Template

HERE = Path(__file__).resolve().parent
SITE = HERE.parent
BASE = "https://vano.tanis.codes/"
OUT = Path(sys.argv[1]) if len(sys.argv) > 1 else SITE / "docs"

# The sidebar, in order. It also sets the Previous and Next links and the breadcrumb.
NAV = [
    ("Getting started", [
        ("index.html", "Introduction"),
        ("installation.html", "Installation"),
        ("quick-start.html", "Quick start"),
        ("interface.html", "The interface"),
    ]),
    ("Guides", [
        ("scanning.html", "What to scan"),
        ("recommendations.html", "Recommendations"),
        ("removing.html", "Removing files"),
        ("wsl.html", "WSL 2 distributions"),
    ]),
    ("Reference", [
        ("command-line.html", "Command line"),
        ("keyboard-shortcuts.html", "Keyboard shortcuts"),
        ("settings.html", "Settings"),
        ("files-and-folders.html", "Files and folders"),
    ]),
    ("Help", [
        ("troubleshooting.html", "Troubleshooting"),
        ("faq.html", "FAQ"),
    ]),
]


def slugify(text):
    text = re.sub(r"<[^>]+>", "", text)
    text = html.unescape(text).lower()
    return re.sub(r"[^a-z0-9]+", "-", text).strip("-")


def code_panel(match):
    lang, title, code = match.group(1), match.group(2), match.group(3)
    attrs = f' data-lang="{lang}"' if lang and lang != "text" else ""
    head = ""
    if title:
        head = f'<div class="code-title"><span class="dot"></span>{html.escape(title)}</div>'
    return f'<div class="code">{head}<pre{attrs}>{html.escape(code, quote=False)}</pre></div>'


def render_body(body):
    body = re.sub(
        r'^```(\w*)(?: title="([^"]*)")?\n(.*?)\n```$', code_panel, body, flags=re.S | re.M
    )
    used = set(re.findall(r'id="([^"]+)"', body))

    def add_id(match):
        tag, attrs, inner = match.groups()
        if "id=" in attrs:
            return match.group(0)
        slug = base = slugify(inner)
        n = 2
        while slug in used:
            slug, n = f"{base}-{n}", n + 1
        used.add(slug)
        return f'<{tag}{attrs} id="{slug}">{inner}</{tag}>'

    # Not indented: indentation would end up inside <pre> blocks.
    return re.sub(r"<(h[234])([^>]*)>(.*?)</\1>", add_id, body, flags=re.S).strip()


def render_nav(current):
    lines = []
    for group, items in NAV:
        lines.append('            <div class="group">')
        lines.append(f"                <h2>{group}</h2>")
        lines.append("                <ul>")
        for file, label in items:
            mark = ' aria-current="page"' if file == current else ""
            lines.append(f'                    <li><a href="{file}"{mark}>{label}</a></li>')
        lines.append("                </ul>")
        lines.append("            </div>")
    return "\n".join(lines)


def render_pager(order, i):
    links = []
    if i > 0:
        file, label, _ = order[i - 1]
        links.append(f'                    <a class="prev" href="{file}"><small>Previous</small>{label}</a>')
    if i < len(order) - 1:
        file, label, _ = order[i + 1]
        links.append(f'                    <a class="next" href="{file}"><small>Next</small>{label}</a>')
    return "\n".join(links)


def check_links(pages, errors):
    """Every link to another docs page, and every #anchor, must exist."""
    ids = {name: set(re.findall(r'id="([^"]+)"', text)) for name, text in pages.items()}
    for name, text in pages.items():
        for href in re.findall(r'href="([^"]+)"', text):
            if href.startswith(("http:", "https:", "mailto:")):
                continue
            if href.startswith("../"):
                target = (SITE / href[3:].split("#")[0]).resolve()
                if not target.exists():
                    errors.append(f"{name}: broken link {href}")
                continue
            page, _, anchor = href.partition("#")
            if page == "":
                page = name
            elif page == "./":
                page = "index.html"
            if page not in pages:
                errors.append(f"{name}: broken link {href}")
            elif anchor and anchor not in ids[page]:
                errors.append(f"{name}: missing anchor {href}")
        for src in re.findall(r'src="\.\./([^"]+)"', text):
            if not (SITE / src).exists():
                errors.append(f"{name}: missing file ../{src}")


def page_url(file):
    return f"{BASE}docs/{'' if file == 'index.html' else file}"


def json_ld(title, description, file, group):
    """The page's structured data: a TechArticle and its breadcrumb."""
    crumbs = [("Vano", BASE), ("Docs", page_url("index.html"))]
    if file != "index.html":
        crumbs.append((title, page_url(file)))
    data = {
        "@context": "https://schema.org",
        "@graph": [
            {
                "@type": "TechArticle",
                "headline": title,
                "description": description,
                "url": page_url(file),
                "inLanguage": "en",
                "articleSection": group,
                "isPartOf": {"@type": "WebSite", "name": "Vano", "url": BASE},
                "about": {"@type": "SoftwareApplication", "name": "Vano", "url": BASE},
            },
            {
                "@type": "BreadcrumbList",
                "itemListElement": [
                    {"@type": "ListItem", "position": i + 1, "name": name, "item": url}
                    for i, (name, url) in enumerate(crumbs)
                ],
            },
        ],
    }
    return json.dumps(data, ensure_ascii=False, indent=4).replace("</", "<\\/")


def write_root_files(order, metas):
    urls = [BASE, f"{BASE}install.html"] + [page_url(file) for file, _, _ in order]
    sitemap = "".join(f"    <url>\n        <loc>{url}</loc>\n    </url>\n" for url in urls)
    (SITE / "sitemap.xml").write_text(
        '<?xml version="1.0" encoding="UTF-8"?>\n'
        '<urlset xmlns="http://www.sitemaps.org/schemas/sitemap/0.9">\n'
        f"{sitemap}</urlset>\n"
    )
    docs = [f"- [{metas[file]['title']}]({page_url(file)}): {metas[file]['description']}"
            for file, _, _ in order]
    llms = Template((HERE / "llms.txt").read_text()).substitute(docs="\n".join(docs))
    (SITE / "llms.txt").write_text(llms)


def main():
    template = Template((HERE / "template.html").read_text())
    order = [(file, label, group) for group, items in NAV for file, label in items]
    errors = []

    sources = {path.name for path in (HERE / "pages").glob("*.html")}
    listed = {file for file, _, _ in order}
    errors += [f"{file}: in NAV but there is no pages/{file}" for file in sorted(listed - sources)]
    errors += [f"pages/{file}: not listed in NAV" for file in sorted(sources - listed)]

    pages = {}
    metas = {}
    for i, (file, label, group) in enumerate(order):
        if file not in sources:
            continue
        head, body = (HERE / "pages" / file).read_text().split("\n---\n", 1)
        meta = dict(line.split(": ", 1) for line in head.strip().split("\n"))
        meta.setdefault("title", label)
        metas[file] = meta
        page = template.substitute(
            title=meta["title"],
            og_title=html.escape(f"{meta['title']} · Vano docs"),
            description=html.escape(meta["description"]),
            canonical="" if file == "index.html" else file,
            json_ld=json_ld(meta["title"], meta["description"], file, group),
            nav=render_nav(file),
            group=group,
            body=render_body(body),
            file=file,
            pager=render_pager(order, i),
        )
        if "—" in page:
            errors.append(f"pages/{file}: contains an em dash, use a comma or a period")
        pages[file] = page

    check_links(pages, errors)
    if errors:
        print("\n".join(errors), file=sys.stderr)
        sys.exit(1)

    OUT.mkdir(parents=True, exist_ok=True)
    for stale in OUT.glob("*.html"):
        if stale.name not in pages:
            stale.unlink()
    for file, page in pages.items():
        (OUT / file).write_text(page)
    write_root_files(order, metas)
    print(f"Built {len(pages)} pages into {OUT}, and sitemap.xml and llms.txt")


if __name__ == "__main__":
    main()
